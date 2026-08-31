//! rawler-backed Decoder. rawler handles unpack/levels/demosaic/crop;
//! WE control the color landing (as-shot WB → dual-illuminant camera→XYZ →
//! linear Rec.2020, headroom preserved — spec 1.3). rawler's own
//! WhiteBalance/Calibrate/SRgb steps are deliberately NOT used: they bake
//! sRGB and clip highlights.

use super::{DecodedImage, Decoder, Demosaic, ImageMeta};
use crate::color::{mat_vec, CameraCalibration, Mat3};
use crate::error::CoreError;
use crate::image::RgbF32Buf;
use crate::profile::dcp::DcpProfile;
use demosaic::{Algorithm as XtransAlgorithm, CfaPattern, Channel};
use rawler::cfa::CFAColor;
use rawler::decoders::RawDecodeParams;
use rawler::imgop::develop::{Intermediate, ProcessingStep, RawDevelop};
use rawler::imgop::{Point, Rect};
use rawler::rawimage::RawPhotometricInterpretation;
use rawler::rawsource::RawSource;
use rawler::RawLoader;
use std::path::Path;

pub struct RawlerDecoder {
    loader: RawLoader,
}

// rawler decodes Sony's older SR2/SRF containers through its ARW decoder, but
// 0.7.2 does not expose those suffixes from `supported_extensions()`. Keep only
// these verified aliases here; the canonical list remains owned by rawler.
const RAWLER_EXTENSION_ALIASES: &[&str] = &["SR2", "SRF"];

impl Default for RawlerDecoder {
    fn default() -> Self {
        Self {
            loader: RawLoader::new(),
        }
    }
}

fn dec_err(e: impl std::fmt::Display) -> CoreError {
    let msg = e.to_string();
    // RawSource / mmap surface macOS TCC as EPERM — rewrite so the status
    // bar isn't just "Operation not permitted (os error 1)".
    if msg.contains("Operation not permitted") || msg.contains("os error 1") {
        CoreError::Decode(
            "macOS blocked reading this file. Open it via Browse… / the file picker \
             (grants access), or allow MeraRAW under System Settings → Privacy & Security \
             → Files and Folders / Full Disk Access. Download iCloud files in Finder first."
                .into(),
        )
    } else {
        CoreError::Decode(msg)
    }
}

/// rawler explicitly prioritizes decoder diagnostics over panic-freedom. RAWs
/// are untrusted input in a desktop app, so turn an upstream decoder panic into
/// a normal per-file error instead of losing the import worker or application.
fn guard_rawler<T>(
    path: &Path,
    operation: &str,
    f: impl FnOnce() -> Result<T, CoreError>,
) -> Result<T, CoreError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| {
        Err(CoreError::Decode(format!(
            "RAW decoder failed safely while {operation}: {}",
            path.display()
        )))
    })
}

/// Some decoders leave RawImage.orientation at Normal even when the EXIF
/// tag says rotated (seen on Sony ARW). Prefer the raw struct, fall back
/// to the EXIF tag.
fn effective_orientation(
    raw: &rawler::RawImage,
    md: &rawler::decoders::RawMetadata,
) -> rawler::Orientation {
    use rawler::Orientation as O;
    match raw.orientation {
        O::Normal | O::Unknown => md
            .exif
            .orientation
            .map(O::from_u16)
            .filter(|o| !matches!(o, O::Unknown))
            .unwrap_or(raw.orientation),
        o => o,
    }
}

impl RawlerDecoder {
    fn meta_from(
        &self,
        path: &Path,
        raw: &rawler::RawImage,
        md: &rawler::decoders::RawMetadata,
        width: u32,
        height: u32,
        cct: Option<f32>,
    ) -> ImageMeta {
        let exif = &md.exif;
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_uppercase())
            .unwrap_or_else(|| "RAW".into());
        // Apple ProRAW is a linear DNG (already demosaiced: cpp == 3) from an
        // Apple device — surface it distinctly since it edits differently from
        // a mosaiced sensor RAW.
        let is_proraw = ext == "DNG" && raw.cpp == 3 && md.make.to_lowercase().contains("apple");
        let format = if is_proraw { "ProRAW".into() } else { ext };
        let mut meta = ImageMeta {
            path: path.to_string_lossy().into_owned(),
            kind: crate::raw::ImageKind::Raw,
            format,
            bit_depth: raw.bps as u8, // real sensor bit depth from rawler
            camera_make: md.make.clone(),
            camera_model: md.model.clone(),
            lens: exif
                .lens_model
                .clone()
                .or_else(|| exif.lens_make.clone())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            iso: exif.iso_speed_ratings.map(|v| v as u32).or(exif.iso_speed),
            shutter: exif.exposure_time.map(|r| {
                if r.n < r.d {
                    format!("1/{}", (r.d as f32 / r.n.max(1) as f32).round() as u32)
                } else {
                    format!("{:.1}s", r.n as f32 / r.d.max(1) as f32)
                }
            }),
            aperture: exif.fnumber.map(|r| r.n as f32 / r.d.max(1) as f32),
            focal_mm: exif.focal_length.map(|r| r.n as f32 / r.d.max(1) as f32),
            captured_at: exif.date_time_original.clone(),
            width,
            height,
            orientation: format!("{:?}", effective_orientation(raw, md)),
            as_shot_wb: {
                let wb = effective_wb(raw.wb_coeffs, &CameraCalibration::from_raw(raw));
                [wb[0], wb[1], wb[2]]
            },
            estimated_cct: cct,
            camera_profile: None,
            available_profiles: Vec::new(),
            available_profile_files: Vec::new(),
            demosaic: String::new(), // set by decode_impl once the algo is known
            available_demosaic: crate::raw::Demosaic::available(),
            gps_lat: None,
            gps_lon: None,
            input_color_space: None, // RAW → camera→Rec.2020 via DCP, not ICC
            video: None,
        };
        // GPS (and any missing tags) from container EXIF when readable (DNG/JPEG…).
        crate::metadata::enrich_from_file(path, &mut meta);
        meta
    }
}

impl Decoder for RawlerDecoder {
    fn probe(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|extension| {
                rawler::decoders::supported_extensions()
                    .iter()
                    .chain(RAWLER_EXTENSION_ALIASES.iter())
                    .any(|supported| extension.eq_ignore_ascii_case(supported))
            })
            .unwrap_or(false)
    }

    fn metadata(&self, path: &Path) -> Result<ImageMeta, CoreError> {
        guard_rawler(path, "reading metadata", || {
            let source = RawSource::new(path).map_err(dec_err)?;
            let decoder = self.loader.get_decoder(&source).map_err(dec_err)?;
            let params = RawDecodeParams::default();
            let md = decoder.raw_metadata(&source, &params).map_err(dec_err)?;
            // dims need the raw struct; decode dummy (no pixel work)
            let raw = decoder.raw_image(&source, &params, true).map_err(dec_err)?;
            let cal = CameraCalibration::from_raw(&raw);
            let cct = Some(cal.estimate_cct(&effective_wb(raw.wb_coeffs, &cal)));
            Ok(self.meta_from(path, &raw, &md, raw.width as u32, raw.height as u32, cct))
        })
    }

    fn embedded_preview(
        &self,
        path: &Path,
        max_dim: u32,
    ) -> Result<Option<(Vec<u8>, u32, u32)>, CoreError> {
        guard_rawler(path, "reading the embedded preview", || {
            let source = RawSource::new(path).map_err(dec_err)?;
            let decoder = self.loader.get_decoder(&source).map_err(dec_err)?;
            let params = RawDecodeParams::default();
            let md = decoder.raw_metadata(&source, &params).map_err(dec_err)?;
            // EXIF orientation only — a dummy raw_image poisons some decoders
            // (Fuji RAF) and breaks the follow-up full_image JPEG read.
            let orientation = md
                .exif
                .orientation
                .map(rawler::Orientation::from_u16)
                .filter(|o| !matches!(o, rawler::Orientation::Unknown))
                .unwrap_or(rawler::Orientation::Normal);

            let source = RawSource::new(path).map_err(dec_err)?;
            let decoder = self.loader.get_decoder(&source).map_err(dec_err)?;
            let img = match decoder.preview_image(&source, &params) {
                Ok(Some(img)) => Some(img),
                _ => match decoder.thumbnail_image(&source, &params) {
                    Ok(Some(img)) => Some(img),
                    _ => decoder.full_image(&source, &params).ok().flatten(),
                },
            };
            let Some(img) = img else {
                return Ok(None);
            };
            let img = match orientation {
                rawler::Orientation::Rotate90 => img.rotate90(),
                rawler::Orientation::Rotate180 => img.rotate180(),
                rawler::Orientation::Rotate270 => img.rotate270(),
                rawler::Orientation::HorizontalFlip => img.fliph(),
                rawler::Orientation::VerticalFlip => img.flipv(),
                _ => img,
            };
            let img = if img.width().max(img.height()) > max_dim {
                img.thumbnail(max_dim, max_dim)
            } else {
                img
            };
            let rgba = img.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            Ok(Some((rgba.into_raw(), w, h)))
        })
    }

    fn decode(&self, path: &Path) -> Result<DecodedImage, CoreError> {
        self.safe_decode_impl(path, None, Demosaic::Rawler)
    }

    fn decode_with_profile(
        &self,
        path: &Path,
        profile_path: Option<&Path>,
    ) -> Result<DecodedImage, CoreError> {
        self.safe_decode_impl(path, profile_path, Demosaic::Rawler)
    }

    fn decode_with_options(
        &self,
        path: &Path,
        profile_path: Option<&Path>,
        demosaic: Demosaic,
    ) -> Result<DecodedImage, CoreError> {
        match self.safe_decode_impl(path, profile_path, demosaic) {
            Ok(img) => Ok(img),
            Err(e) if demosaic != Demosaic::Rawler => {
                tracing::warn!(
                    algo = demosaic.name(),
                    error = %e,
                    "demosaic failed; retrying with rawler built-in"
                );
                self.safe_decode_impl(path, profile_path, Demosaic::Rawler)
            }
            Err(e) => Err(e),
        }
    }
}

impl RawlerDecoder {
    fn safe_decode_impl(
        &self,
        path: &Path,
        profile_path: Option<&Path>,
        demosaic: Demosaic,
    ) -> Result<DecodedImage, CoreError> {
        guard_rawler(path, "decoding pixels", || {
            self.decode_impl(path, profile_path, demosaic)
        })
    }

    /// Shared decode: rawler unpack → produce camera-native RGB (rawler's
    /// built-in demosaic or the merawler engine) → as-shot WB → cam→Rec.2020.
    fn decode_impl(
        &self,
        path: &Path,
        profile_path: Option<&Path>,
        demosaic: Demosaic,
    ) -> Result<DecodedImage, CoreError> {
        let source = RawSource::new(path).map_err(dec_err)?;
        let decoder = self.loader.get_decoder(&source).map_err(dec_err)?;
        let params = RawDecodeParams::default();
        let md = decoder.raw_metadata(&source, &params).map_err(dec_err)?;
        // zerawler decodes pixels in the worker process, so a metadata-only
        // (dummy) rawler decode suffices there; other paths need pixels.
        let zalgo = demosaic.zerawler_algo();
        let mut raw = decoder
            .raw_image(&source, &params, zalgo.is_some())
            .map_err(dec_err)?;

        // Pixel source + its colour state. `effective` is the algorithm that
        // ACTUALLY produced the pixels — fallbacks report `rawler`, so the UI
        // and sidecar never claim an algorithm that didn't run.
        //  * merawler / rawler   → camera-native RGB, sensor orientation
        //  * zerawler LibRaw     → camera-native RGB, orientation pre-baked
        //  * zerawler RT         → linear Rec.2020 + camera WB, pre-baked
        //    (validated contract; skips our WB+matrix landing entirely)
        let (mut cam_rgb, w, h, state, effective): (Vec<[f32; 3]>, usize, usize, PixelState, Demosaic) =
            if let Some(algo) = zalgo {
                match zerawler_rgb(path, algo, &raw) {
                    Ok((data, w, h)) => {
                        let state = match algo.backend() {
                            zerawler::Backend::RawTherapee => PixelState::Rec2020Ready,
                            zerawler::Backend::LibRaw => {
                                PixelState::CameraNative { prerotated: true }
                            }
                        };
                        (data, w, h, state, demosaic)
                    }
                    Err(e) => {
                        tracing::warn!(
                            algo = demosaic.name(),
                            error = %e,
                            "zerawler sidecar failed; falling back to rawler demosaic"
                        );
                        raw = decoder
                            .raw_image(&source, &params, false)
                            .map_err(dec_err)?;
                        let (d, w, h) = rawler_cam_rgb(&raw)?;
                        (
                            d,
                            w,
                            h,
                            PixelState::CameraNative { prerotated: false },
                            Demosaic::Rawler,
                        )
                    }
                }
            } else {
                let force_rawler = is_rgbe_sensor(&raw)
                    || matches!(
                        &raw.photometric,
                        RawPhotometricInterpretation::Cfa(config)
                            if config.cfa.width == 6 && config.cfa.height == 6
                    );
                let (d, w, h, effective) = if force_rawler {
                    let (d, w, h) = rawler_cam_rgb(&raw)?;
                    (d, w, h, Demosaic::Rawler)
                } else {
                    match demosaic.merawler_algo() {
                    Some(algo) => match merawler_cam_rgb(&raw, algo) {
                        Some((d, w, h)) => (d, w, h, demosaic),
                        None => {
                            tracing::info!(
                                algo = demosaic.name(),
                                "merawler unavailable for this CFA (non-Bayer/already demosaiced); using rawler"
                            );
                            let (d, w, h) = rawler_cam_rgb(&raw)?;
                            (d, w, h, Demosaic::Rawler)
                        }
                    },
                    None => {
                        let (d, w, h) = rawler_cam_rgb(&raw)?;
                        (d, w, h, Demosaic::Rawler)
                    }
                    }
                };
                (
                    d,
                    w,
                    h,
                    PixelState::CameraNative { prerotated: false },
                    effective,
                )
            };

        if matches!(state, PixelState::CameraNative { .. }) {
            normalize_cam_rgb(&mut cam_rgb, &raw);
        }

        let cal = CameraCalibration::from_raw(&raw);
        let mut wb = effective_wb(raw.wb_coeffs, &cal);
        // Mamiya ZD: mild cool — daylight WB + Adobe matrix reads warm vs Affinity's
        // near-neutral Develop export. Keep R close to G; lift B only.
        {
            let make_l = md.make.to_ascii_lowercase();
            if make_l.contains("mamiya") {
                let g = wb[1].max(1e-6);
                wb[0] = g * 1.05;
                wb[2] = g * 1.38;
            }
        }
        let cct = cal.estimate_cct(&wb);

        let mut data = vec![0.0f32; w * h * 3];
        match state {
            // Already linear Rec.2020 with camera WB (zerawler RT contract):
            // just floor out-of-gamut negatives.
            PixelState::Rec2020Ready => {
                for (i, px) in cam_rgb.iter().enumerate() {
                    let o = i * 3;
                    data[o] = px[0].max(0.0);
                    data[o + 1] = px[1].max(0.0);
                    data[o + 2] = px[2].max(0.0);
                }
            }
            // Our colorimetric landing: as-shot WB → cam→Rec.2020 (dual-illum).
            PixelState::CameraNative { .. } => {
                let cam2rec = resolve_cam2rec(&cal, &wb, &raw, &md, profile_path)?;
                camera_landing(&cam_rgb, wb_normalize(wb), &cam2rec, &mut data);
            }
        }
        // Digic II CRW: daylight WB is already R-heavy, but rawler's cam→XYZ still
        // lands green/cyan vs Affinity. Correct in Rec.2020 after the matrix.
        affinity_rec2020_channel_fix(&mut data, &md.make, &md.model);

        let buf = RgbF32Buf {
            width: w,
            height: h,
            data,
        };
        // zerawler workers bake EXIF orientation themselves.
        let prerotated = matches!(
            state,
            PixelState::Rec2020Ready | PixelState::CameraNative { prerotated: true }
        );
        let working = if prerotated {
            buf
        } else {
            buf.bake_orientation(effective_orientation(&raw, &md))
        };

        let mut meta = self.meta_from(
            path,
            &raw,
            &md,
            working.width as u32,
            working.height as u32,
            Some(cct),
        );
        meta.demosaic = effective.name().to_string();
        Ok(DecodedImage { working, meta })
    }
}

/// Some decoders (e.g. older Fuji RAF) leave demosaiced camera RGB in raw ADC
/// units after Rescale instead of 0..1. Detect and rescale before the color matrix.
fn normalize_cam_rgb(cam_rgb: &mut [[f32; 3]], raw: &rawler::RawImage) {
    let max = cam_rgb
        .iter()
        .flat_map(|p| p.iter())
        .cloned()
        .fold(0.0f32, f32::max);
    if max <= 2.0 {
        return;
    }
    let wl = raw.whitelevel.0.first().copied().unwrap_or(65535) as f32;
    let bl = raw
        .blacklevel
        .levels
        .first()
        .map(|r| r.as_f32())
        .unwrap_or(0.0);
    let denom = (wl - bl).max(1.0);
    for px in cam_rgb.iter_mut() {
        for c in 0..3 {
            px[c] = ((px[c] - bl) / denom).max(0.0);
        }
    }
}

/// Green-normalized as-shot WB multipliers `[r/g, 1, b/g]` (NaN/degenerate → unity).
fn wb_normalize(mut wb: [f32; 4]) -> [f32; 3] {
    if wb[0].is_nan() || wb[1] <= 0.0 {
        wb = [1.0, 1.0, 1.0, 1.0];
    }
    let g = wb[1];
    [wb[0] / g, 1.0, wb[2] / g]
}

/// The file's as-shot multipliers when it has usable ones, else the
/// calibration's daylight neutral.
///
/// rawler reports `[NaN, NaN, NaN]` for Canon CRW, Sony SRF, Kodak DCR and
/// Mamiya MEF. Unity WB rendered those with a gross cast (the A620 landed at
/// G ≈ 2×R) and fed `estimate_cct` a meaningless neutral, which then picked
/// the wrong illuminant matrix — so both the WB and the matrix were wrong.
fn effective_wb(wb: [f32; 4], cal: &CameraCalibration) -> [f32; 4] {
    let usable = wb[..3]
        .iter()
        .all(|v| v.is_finite() && *v > 0.0);
    if usable {
        return wb;
    }
    cal.daylight_wb().unwrap_or([1.0, 1.0, 1.0, f32::NAN])
}

fn is_rgbe_sensor(raw: &rawler::RawImage) -> bool {
    raw.cpp == 4 || raw.camera.cfa.is_rgbe()
}

/// Resolve the camera→Rec.2020 matrix: a matching DCP wins, otherwise
/// rawler's calibration; one error site for "no usable matrix".
///
/// (Behavior note: the old inline version fell back to an *identity* matrix
/// when a matched DCP produced no matrix but calibration also failed — that
/// silently rendered wildly wrong colour. All paths now fail loudly instead.)
fn resolve_cam2rec(
    cal: &CameraCalibration,
    wb: &[f32; 4],
    raw: &rawler::RawImage,
    md: &rawler::decoders::RawMetadata,
    profile_path: Option<&Path>,
) -> Result<Mat3, CoreError> {
    // Adobe DCP matrices assume 3-channel RGB. Sony F828 RGBE lands as RGB via
    // (G+E)/2; applying the DCP matrix kills blue (linear B≈0, sat≈1.0).
    if !is_rgbe_sensor(raw) {
        if let Some(p) = profile_path {
            match DcpProfile::load(p).ok() {
                Some(dcp) if dcp.matches_camera(&md.make, &md.model) => {
                    if let Some(m) = dcp.cam_to_rec2020(wb, cal) {
                        return Ok(m);
                    }
                    tracing::warn!(file = ?p, "DCP matrix failed; falling back to rawler calibration");
                }
                Some(dcp) => tracing::warn!(
                    profile = %dcp.unique_camera_model,
                    make = %md.make,
                    model = %md.model,
                    "DCP camera mismatch; using rawler calibration"
                ),
                None => tracing::warn!(file = ?p, "DCP load failed; using rawler calibration"),
            }
        }
    } else {
        tracing::info!(make = %md.make, model = %md.model, "RGBE sensor: skipping DCP colour matrix");
    }
    cal.cam_to_rec2020(wb).ok_or_else(|| {
        CoreError::Decode(format!(
            "no usable color matrix for {} {} (try a DCP profile or update rawler)",
            md.make, md.model
        ))
    })
}

/// How far blown highlights are pulled toward neutral: 1.0 is dcraw's full
/// "blend", 0.0 leaves chroma untouched.
const HIGHLIGHT_DESATURATION: f32 = 0.85;

/// Highlight reconstruction in WB'd camera space (dcraw/RawTherapee "blend").
///
/// The sensor clips every channel at the same raw level, so after WB a blown
/// pixel arrives as `[r_mul, 1, b_mul]` — strongly coloured rather than white.
/// Worse, when only one channel clips (a sunlit orange petal saturates red
/// first) that channel flatlines while the others keep rising, so the hue
/// drifts and the texture disappears into a solid blob.
///
/// Keeping luminance from the over-range data preserves that texture, because
/// the unclipped channels still vary; scaling the two opponent-chroma axes by
/// the clipped pixel's chroma ratio rolls the colour off toward white instead
/// of letting it shift hue.
fn highlight_blend(rgb: [f32; 3], clip: f32, desaturation: f32) -> [f32; 3] {
    if rgb[0] <= clip && rgb[1] <= clip && rgb[2] <= clip {
        return rgb;
    }
    // Luma plus two opponent-chroma axes (dcraw's `trans`, unnormalized).
    let opponent = |c: [f32; 3]| {
        [
            c[0] + c[1] + c[2],
            1.732_050_8 * (c[0] - c[1]),
            2.0 * c[2] - c[0] - c[1],
        ]
    };
    let full = opponent(rgb);
    let clipped = opponent([rgb[0].min(clip), rgb[1].min(clip), rgb[2].min(clip)]);
    let chroma_full = full[1] * full[1] + full[2] * full[2];
    let ratio = if chroma_full > 1e-9 {
        ((clipped[1] * clipped[1] + clipped[2] * clipped[2]) / chroma_full).sqrt()
    } else {
        1.0
    };
    // Full dcraw blend (ratio as-is) greys out a petal that only clipped red,
    // so only take `desaturation` of the way there.
    let ratio = 1.0 - desaturation * (1.0 - ratio);
    let (l, a, b) = (full[0], full[1] * ratio, full[2] * ratio);
    [
        (l + 0.866_025_4 * a - 0.5 * b) / 3.0,
        (l - 0.866_025_4 * a - 0.5 * b) / 3.0,
        (l + b) / 3.0,
    ]
}

/// Per-camera Rec.2020 channel fixes after WB+matrix — Affinity Develop match
/// when rawler's calibration matrix cannot be nudged via WB alone.
fn affinity_rec2020_channel_fix(data: &mut [f32], make: &str, model: &str) {
    let make_l = make.to_ascii_lowercase();
    let model_l = model.to_ascii_lowercase();
    // PowerShot A620 CRW: matrix under-emits R → green cast + inflated sat.
    if make_l.contains("canon")
        && (model_l.contains("a620")
            || model_l.contains("a610")
            || model_l.contains("a630")
            || model_l.contains("powershot a"))
    {
        for px in data.chunks_exact_mut(3) {
            px[0] *= 1.55;
            px[1] *= 0.97;
            px[2] *= 0.93;
        }
        return;
    }
    // Mamiya ZD: Affinity PNG is near-neutral; pull chroma toward luma after
    // matrix (LookTable damp alone left residual warm cast).
    if make_l.contains("mamiya") {
        const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];
        let keep = 0.42f32; // retain chroma toward Affinity sat (~0.17)
        for px in data.chunks_exact_mut(3) {
            let l = LUMA[0] * px[0].max(0.0) + LUMA[1] * px[1].max(0.0) + LUMA[2] * px[2].max(0.0);
            px[0] = (l + (px[0] - l) * keep).max(0.0);
            px[1] = (l + (px[1] - l) * keep).max(0.0);
            px[2] = (l + (px[2] - l) * keep).max(0.0);
        }
        return;
    }
    // Panasonic FZ45: tiny anti-magenta nudge (FZ40 LookTable already dropped).
    if make_l.contains("panasonic") && model_l.contains("fz45") {
        for px in data.chunks_exact_mut(3) {
            px[0] *= 0.99;
            px[1] *= 1.015;
            px[2] *= 0.995;
        }
    }
}

/// The camera landing: WB multipliers → highlight reconstruction →
/// cam→Rec.2020 matrix per pixel, into an interleaved buffer. Headroom kept;
/// only negatives clamped.
fn camera_landing(cam_rgb: &[[f32; 3]], wbn: [f32; 3], cam2rec: &Mat3, data: &mut [f32]) {
    // Channels clip together at 1.0 in camera space, so post-WB each clips at
    // its own multiplier; the lowest is where reconstruction has to start.
    let clip = wbn[0].min(wbn[1]).min(wbn[2]).max(1e-4);
    let desaturation = std::env::var("MERARAW_HL_DESAT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(HIGHLIGHT_DESATURATION);
    for (i, px) in cam_rgb.iter().enumerate() {
        let wbd = highlight_blend(
            [px[0] * wbn[0], px[1] * wbn[1], px[2] * wbn[2]],
            clip,
            desaturation,
        );
        let rgb = mat_vec(cam2rec, wbd);
        let o = i * 3;
        data[o] = rgb[0].max(0.0);
        data[o + 1] = rgb[1].max(0.0);
        data[o + 2] = rgb[2].max(0.0);
    }
}

/// Colour state of the demosaiced pixel buffer entering the landing stage.
#[derive(Clone, Copy)]
enum PixelState {
    /// Camera-native RGB, unity WB. `prerotated` = EXIF orientation already
    /// baked by the producer (zerawler workers rotate; in-process paths don't).
    CameraNative { prerotated: bool },
    /// Linear Rec.2020 with camera WB applied (zerawler RT validated
    /// contract) — skips the WB+matrix landing. Always prerotated.
    Rec2020Ready,
}

/// zerawler sidecar path: worker decodes the whole file. Both backends hand
/// back linear data per their validated contracts — LibRaw camera-native
/// (levels forced to rawler's below), RT linear Rec.2020 with camera WB
/// (TRC decoded inside zerawler).
fn zerawler_rgb(
    path: &Path,
    algo: zerawler::Algorithm,
    raw: &rawler::RawImage,
) -> Result<(Vec<[f32; 3]>, usize, usize), CoreError> {
    let engine = zerawler::Engine::detect();
    let mut opts = zerawler::DecodeOpts::default();
    if algo.backend() == zerawler::Backend::LibRaw {
        // Force rawler's levels so dcraw's normalization matches ours exactly.
        let blacks: Vec<f32> = raw.blacklevel.levels.iter().map(|r| r.as_f32()).collect();
        opts = zerawler::DecodeOpts::from_levels(&blacks, raw.whitelevel.0.first().copied());
    }
    let dec = engine
        .decode_opts(path, algo, zerawler::Mode::Native, opts)
        .map_err(|e| CoreError::Decode(format!("zerawler: {e}")))?;
    Ok((dec.image.data, dec.image.width, dec.image.height))
}

/// rawler's built-in path: rescale → demosaic → validated crops → camera-native RGB.
fn rawler_cam_rgb(raw: &rawler::RawImage) -> Result<(Vec<[f32; 3]>, usize, usize), CoreError> {
    // rawler 0.7.2's generic RawDevelop path always chooses its Bayer PPG
    // demosaicer for three-colour CFAs, including Fujifilm's 6x6 X-Trans
    // layout. That misreads X-Trans pixels as a 2x2 Bayer pattern and produces
    // a strong green checkerboard. Use a dedicated pure-Rust X-Trans engine;
    // keep the existing rawler path for Bayer and already-RGB files.
    let intermediate = match &raw.photometric {
        RawPhotometricInterpretation::Cfa(config)
            if config.cfa.width == 6 && config.cfa.height == 6 =>
        {
            let dev = RawDevelop {
                steps: vec![ProcessingStep::Rescale],
            };
            let Intermediate::Monochrome(px) = dev.develop_intermediate(raw).map_err(dec_err)?
            else {
                return Err(CoreError::Decode(
                    "X-Trans RAW unexpectedly contained multiple channels".into(),
                ));
            };
            let roi = raw.active_area.unwrap_or_else(|| px.rect());
            let mosaic = px.into_crop(roi);
            let pattern = std::array::from_fn(|i| {
                let row = roi.p.y + i / 6;
                let col = roi.p.x + i % 6;
                let plane = config.cfa.color_at(row, col);
                match config.colors.colors.get(plane).copied() {
                    Some(CFAColor::RED) => Channel::Red,
                    Some(CFAColor::GREEN) => Channel::Green,
                    Some(CFAColor::BLUE) => Channel::Blue,
                    _ => Channel::Green,
                }
            });
            let cfa = CfaPattern::xtrans(pattern);
            let mut interleaved = vec![0.0f32; mosaic.width * mosaic.height * 3];
            demosaic::demosaic_interleaved(
                &mosaic.data,
                mosaic.width,
                mosaic.height,
                &cfa,
                XtransAlgorithm::Markesteijn1,
                &mut interleaved,
            )
            .map_err(dec_err)?;
            let rgb = interleaved
                .chunks_exact(3)
                .map(|p| [p[0], p[1], p[2]])
                .collect();
            Intermediate::ThreeColor(rawler::pixarray::Color2D::new_with(
                rgb,
                mosaic.width,
                mosaic.height,
            ))
        }
        _ => {
            let dev = RawDevelop {
                steps: vec![
                    ProcessingStep::Rescale,
                    ProcessingStep::Demosaic,
                    ProcessingStep::CropActiveArea,
                ],
            };
            dev.develop_intermediate(raw).map_err(dec_err)?
        }
    };
    let intermediate = safe_default_crop(raw, intermediate);
    Ok(match intermediate {
        Intermediate::ThreeColor(px) => (px.data, px.width, px.height),
        Intermediate::FourColor(px) => (
            px.data
                .iter()
                .map(|p| [p[0], (p[1] + p[3]) * 0.5, p[2]])
                .collect(),
            px.width,
            px.height,
        ),
        Intermediate::Monochrome(px) => (
            px.data.iter().map(|v| [*v, *v, *v]).collect(),
            px.width,
            px.height,
        ),
    })
}

/// Apply rawler's default crop only when it is actually contained by the
/// already-applied active area. Some RAFs (including Fujifilm X-T2 files) have
/// a wider default crop than their active-area record; rawler 0.7.2 asserts in
/// `Rect::adapt` for that metadata combination. Treat the default crop as
/// advisory and retain the valid active-area image when it is inconsistent.
fn safe_default_crop(raw: &rawler::RawImage, intermediate: Intermediate) -> Intermediate {
    let Some(crop) = raw.crop_area else {
        return intermediate;
    };
    let master = raw
        .active_area
        .unwrap_or_else(|| Rect::new(Point::zero(), raw.dim()));

    let crop_right = crop.p.x.checked_add(crop.d.w);
    let crop_bottom = crop.p.y.checked_add(crop.d.h);
    let master_right = master.p.x.checked_add(master.d.w);
    let master_bottom = master.p.y.checked_add(master.d.h);
    let contained = crop.p.x >= master.p.x
        && crop.p.y >= master.p.y
        && crop_right.is_some_and(|v| master_right.is_some_and(|m| v <= m))
        && crop_bottom.is_some_and(|v| master_bottom.is_some_and(|m| v <= m));
    if !contained {
        tracing::warn!(?crop, ?master, "ignoring invalid RAW default crop");
        return intermediate;
    }

    let mut local = Rect::new(
        Point::new(crop.p.x - master.p.x, crop.p.y - master.p.y),
        crop.d,
    );
    if intermediate.dim().w == master.d.w / 2 && intermediate.dim().h == master.d.h / 2 {
        local.scale(0.5);
    }
    let dim = intermediate.dim();
    let fits = local.p.x.checked_add(local.d.w).is_some_and(|v| v <= dim.w)
        && local.p.y.checked_add(local.d.h).is_some_and(|v| v <= dim.h);
    if !fits || (local.p == Point::zero() && local.d == dim) {
        return intermediate;
    }

    match intermediate {
        Intermediate::Monochrome(px) => Intermediate::Monochrome(px.into_crop(local)),
        Intermediate::ThreeColor(px) => Intermediate::ThreeColor(px.crop(local)),
        Intermediate::FourColor(px) => Intermediate::FourColor(px.crop(local)),
    }
}

/// merawler path: rescale-only → normalized full-sensor mosaic → merawler
/// demosaic → crop to rawler's default output region. Returns `None` (so the
/// caller falls back to rawler) for anything that isn't a 2x2 Bayer CFA.
fn merawler_cam_rgb(
    raw: &rawler::RawImage,
    algo: merawler::Algorithm,
) -> Option<(Vec<[f32; 3]>, usize, usize)> {
    // Reuse rawler's exact level normalization, then stop before its demosaic.
    let dev = RawDevelop {
        steps: vec![ProcessingStep::Rescale],
    };
    let Intermediate::Monochrome(px) = dev.develop_intermediate(raw).ok()? else {
        return None; // already demosaiced (e.g. Apple ProRAW) or multi-channel
    };
    let pattern = bayer_pattern(&raw.camera.cfa)?; // X-Trans / 4-color → None
                                                   // P0 hot/dead pixel suppression (denoise doc 04) — same-CFA-neighbor median.
                                                   // Always-on for the merawler path; toggle→re-decode lands with the panel.
    let mut mosaic = px.data;
    let _fixed = crate::denoise::hot_pixel_suppress(&mut mosaic, px.width, px.height, 4.0, 1.0);
    let cfa = merawler::CfaImage {
        width: px.width,
        height: px.height,
        data: mosaic,
        pattern,
        wb: [1.0, 1.0, 1.0],
    };
    let rgb = merawler::demosaic(&cfa, algo)?;
    Some(crop_to_output(rgb, raw))
}

/// Map rawler's top-left 2x2 CFA tile to a merawler Bayer pattern.
fn bayer_pattern(cfa: &rawler::cfa::CFA) -> Option<merawler::CfaPattern> {
    let c = |row: usize, col: usize| cfa.color_at(row, col) as u8;
    merawler::CfaPattern::from_tile(c(0, 0), c(0, 1), c(1, 0), c(1, 1))
}

/// Crop the full-sensor demosaic to rawler's net output region — `crop_area`
/// (or `active_area`) in full-sensor coordinates.
fn crop_to_output(
    rgb: merawler::RgbImage,
    raw: &rawler::RawImage,
) -> (Vec<[f32; 3]>, usize, usize) {
    let rect = raw.crop_area.or(raw.active_area);
    match rect {
        Some(r)
            if (r.d.w != rgb.width || r.d.h != rgb.height || r.p.x != 0 || r.p.y != 0)
                && r.p.x + r.d.w <= rgb.width
                && r.p.y + r.d.h <= rgb.height =>
        {
            let (cw, ch) = (r.d.w, r.d.h);
            let mut out = Vec::with_capacity(cw * ch);
            for y in 0..ch {
                let start = (r.p.y + y) * rgb.width + r.p.x;
                out.extend_from_slice(&rgb.data[start..start + cw]);
            }
            (out, cw, ch)
        }
        _ => (rgb.data, rgb.width, rgb.height),
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::raw::Demosaic;
    use std::path::PathBuf;

    #[test]
    fn fuji_raf_decodes_finite_rec2020() {
        let path = PathBuf::from("/tmp/raw-samples/fuji.raf");
        if !path.exists() {
            eprintln!("skip: fuji sample not present");
            return;
        }
        let dec = RawlerDecoder::default();
        let img = dec
            .decode_with_options(&path, None, Demosaic::Rcd)
            .expect("decode fuji");
        assert!(img.working.data.iter().all(|v| v.is_finite()));
        let max = img
            .working
            .data
            .iter()
            .cloned()
            .fold(0.0f32, f32::max);
        assert!(max < 4.0, "implausible max {max}");
    }

    #[test]
    fn embedded_preview_fuji_raf() {
        let path = PathBuf::from("/tmp/raw-samples/fuji.raf");
        if !path.exists() {
            eprintln!("skip: fuji sample not present");
            return;
        }
        let dec = RawlerDecoder::default();
        let prev = dec.embedded_preview(&path, 1600).expect("embedded");
        assert!(prev.is_some(), "expected embedded/full JPEG preview");
    }

    #[test]
    fn probe_tracks_rawler_supported_extensions() {
        let decoder = RawlerDecoder::default();
        for extension in rawler::decoders::supported_extensions() {
            let upper = std::path::PathBuf::from(format!("sample.{extension}"));
            let lower = std::path::PathBuf::from(format!("sample.{}", extension.to_lowercase()));
            assert!(decoder.probe(&upper), "missing .{extension}");
            assert!(
                decoder.probe(&lower),
                "missing case-insensitive .{extension}"
            );
        }
        for extension in RAWLER_EXTENSION_ALIASES {
            let path = std::path::PathBuf::from(format!("sample.{extension}"));
            assert!(decoder.probe(&path), "missing alias .{extension}");
        }
        assert!(!decoder.probe(std::path::Path::new("notes.txt")));
    }

    /// Decode every RAW in the sample corpus and print a per-file report.
    ///
    /// The corpus can be nested and is selected with `MERARAW_RAW_CORPUS`.
    /// Ignored by default because RAW samples are intentionally not checked in.
    ///
    ///   MERARAW_RAW_CORPUS=/path/to/files cargo test -p meratech-core \
    ///     corpus_decodes -- --ignored --nocapture
    #[test]
    #[ignore]
    fn corpus_decodes_every_vendor_format() {
        let dir = std::env::var_os("MERARAW_RAW_CORPUS")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| {
                    std::path::PathBuf::from(home).join("Desktop/test-claude-raw/raw-samples")
                })
            })
            .expect("no corpus path available");
        if !dir.exists() {
            eprintln!("skip: corpus not present at {}", dir.display());
            return;
        }

        let files = crate::catalog::scan_folder(&dir);
        assert!(!files.is_empty(), "corpus dir is empty");

        let dec = RawlerDecoder::default();
        let mut failures = Vec::new();
        let mut decoded = 0usize;

        for path in &files {
            let name = path.file_name().unwrap().to_string_lossy().to_string();

            // catch_unwind: a panic in a decoder is itself a defect worth
            // reporting per-file rather than aborting the whole run.
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                dec.decode_with_options(path, None, Demosaic::default())
            }));

            match res {
                Ok(Ok(img)) => {
                    let m = &img.meta;
                    let finite = img.working.data.iter().all(|v| v.is_finite());
                    let pixels = (img.working.data.len() / 3).max(1) as f64;
                    let mut rgb_mean = [0.0f64; 3];
                    for px in img.working.data.chunks_exact(3) {
                        rgb_mean[0] += px[0] as f64 / pixels;
                        rgb_mean[1] += px[1] as f64 / pixels;
                        rgb_mean[2] += px[2] as f64 / pixels;
                    }
                    println!(
                        "  OK    {name:<32} {fmt:<5} {w}x{h} {depth}bit  {make} {model}  wb={wb:?} mean={rgb_mean:?} finite={finite}",
                        fmt = m.format, w = m.width, h = m.height, depth = m.bit_depth,
                        make = m.camera_make, model = m.camera_model, wb = m.as_shot_wb,
                    );
                    if m.width == 0 || m.height == 0 {
                        failures.push(format!("{name}: zero dimensions"));
                    }
                    if !finite {
                        failures.push(format!("{name}: non-finite pixels"));
                    }
                    decoded += 1;
                }
                Ok(Err(e)) => {
                    println!("  FAIL  {name:<32} {e}");
                    failures.push(format!("{name}: {e}"));
                }
                Err(_) => {
                    println!("  PANIC {name}");
                    failures.push(format!("{name}: decoder PANICKED"));
                }
            }
        }

        println!("\n  {decoded}/{} decoded", files.len());
        assert_eq!(decoded, files.len(), "every corpus file must decode");
        assert!(
            failures.is_empty(),
            "corpus failures:\n  - {}",
            failures.join("\n  - ")
        );
    }

    /// End-to-end: the merawler path must produce the same cropped dimensions as
    /// rawler (crop correctness) and a genuinely different image (algo applied),
    /// with finite values. Skips when the sample RAW isn't present.
    #[test]
    fn merawler_path_matches_dims_and_differs() {
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        let p = std::path::PathBuf::from(home).join("Desktop/test-claude-raw/DSC07078.ARW");
        if !p.exists() {
            eprintln!("skip: sample RAW not present");
            return;
        }
        let dec = RawlerDecoder::default();
        let rawler = dec.decode_with_options(&p, None, Demosaic::Rawler).unwrap();
        for algo in [Demosaic::Rcd, Demosaic::Amaze, Demosaic::Lmmse] {
            let out = dec.decode_with_options(&p, None, algo).unwrap();
            assert_eq!(
                (out.working.width, out.working.height),
                (rawler.working.width, rawler.working.height),
                "{}: crop dims differ from rawler",
                algo.name()
            );
            assert_eq!(out.meta.demosaic, algo.name());
            assert!(
                out.working.data.iter().all(|v| v.is_finite()),
                "{}: non-finite pixels",
                algo.name()
            );
            let diff: f64 = rawler
                .working
                .data
                .iter()
                .zip(&out.working.data)
                .map(|(a, b)| (*a - *b).abs() as f64)
                .sum();
            assert!(
                diff > 0.0,
                "{}: identical to rawler — not applied",
                algo.name()
            );
        }
    }

    /// zerawler sidecar paths decode, land in the working space (orientation
    /// pre-baked by the workers → portrait for this sample), stay finite, and
    /// report their algorithm. Skips per-backend when workers are missing.
    #[test]
    fn zerawler_sidecar_decodes() {
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        let p = std::path::PathBuf::from(home).join("Desktop/test-claude-raw/DSC07078.ARW");
        if !p.exists() {
            eprintln!("skip: sample RAW not present");
            return;
        }
        let engine = zerawler::Engine::detect();
        let dec = RawlerDecoder::default();
        let mut algos: Vec<Demosaic> = Vec::new();
        if engine.dcraw_emu.is_some() {
            algos.push(Demosaic::Dht);
        }
        if engine.rt_cli.is_some() {
            algos.push(Demosaic::RtRcd);
        }
        if algos.is_empty() {
            eprintln!("skip: no zerawler workers detected");
            return;
        }
        for algo in algos {
            let out = dec.decode_with_options(&p, None, algo).unwrap();
            assert_eq!(out.meta.demosaic, algo.name());
            // workers bake EXIF rotation → portrait dims for this sample
            assert!(
                out.working.width > 4000 && out.working.height > 5900,
                "{}: unexpected dims {}x{}",
                algo.name(),
                out.working.width,
                out.working.height
            );
            assert!(
                out.working.data.iter().all(|v| v.is_finite()),
                "{}: non-finite pixels",
                algo.name()
            );
            let mean: f64 = out.working.data.iter().map(|v| *v as f64).sum::<f64>()
                / out.working.data.len() as f64;
            assert!(
                mean > 0.01 && mean < 2.0,
                "{}: implausible mean {mean}",
                algo.name()
            );
        }
    }
}
