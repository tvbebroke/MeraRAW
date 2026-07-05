//! rawler-backed Decoder. rawler handles unpack/levels/demosaic/crop;
//! WE control the color landing (as-shot WB → dual-illuminant camera→XYZ →
//! linear Rec.2020, headroom preserved — spec 1.3). rawler's own
//! WhiteBalance/Calibrate/SRgb steps are deliberately NOT used: they bake
//! sRGB and clip highlights.

use super::{DecodedImage, Decoder, Demosaic, ImageMeta};
use crate::color::{mat_vec, CameraCalibration};
use crate::error::CoreError;
use crate::image::RgbF32Buf;
use crate::profile::dcp::DcpProfile;
use rawler::decoders::RawDecodeParams;
use rawler::imgop::develop::{Intermediate, ProcessingStep, RawDevelop};
use rawler::rawsource::RawSource;
use rawler::RawLoader;
use std::path::Path;

pub struct RawlerDecoder {
    loader: RawLoader,
}

impl Default for RawlerDecoder {
    fn default() -> Self {
        Self {
            loader: RawLoader::new(),
        }
    }
}

const RAW_EXTENSIONS: &[&str] = &[
    "arw", "nef", "nrw", "cr2", "cr3", "crw", "dng", "raf", "orf", "rw2", "pef", "srw", "erf",
    "kdc", "dcs", "dcr", "iiq", "3fr", "mef", "mos",
];

fn dec_err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Decode(e.to_string())
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
        let is_proraw = ext == "DNG"
            && raw.cpp == 3
            && md.make.to_lowercase().contains("apple");
        let format = if is_proraw { "ProRAW".into() } else { ext };
        ImageMeta {
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
            iso: exif
                .iso_speed_ratings
                .map(|v| v as u32)
                .or(exif.iso_speed),
            shutter: exif
                .exposure_time
                .map(|r| {
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
            as_shot_wb: [raw.wb_coeffs[0], raw.wb_coeffs[1], raw.wb_coeffs[2]],
            estimated_cct: cct,
            camera_profile: None,
            available_profiles: Vec::new(),
            available_profile_files: Vec::new(),
            demosaic: String::new(), // set by decode_impl once the algo is known
        }
    }
}

impl Decoder for RawlerDecoder {
    fn probe(&self, path: &Path) -> bool {
        path.extension()
            .map(|e| RAW_EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str()))
            .unwrap_or(false)
    }

    fn metadata(&self, path: &Path) -> Result<ImageMeta, CoreError> {
        let source = RawSource::new(path).map_err(dec_err)?;
        let decoder = self.loader.get_decoder(&source).map_err(dec_err)?;
        let params = RawDecodeParams::default();
        let md = decoder.raw_metadata(&source, &params).map_err(dec_err)?;
        // dims need the raw struct; decode dummy (no pixel work)
        let raw = decoder.raw_image(&source, &params, true).map_err(dec_err)?;
        let cal = CameraCalibration::from_rawler(&raw.color_matrix);
        let cct = Some(cal.estimate_cct(&raw.wb_coeffs));
        Ok(self.meta_from(path, &raw, &md, raw.width as u32, raw.height as u32, cct))
    }

    fn embedded_preview(
        &self,
        path: &Path,
        max_dim: u32,
    ) -> Result<Option<(Vec<u8>, u32, u32)>, CoreError> {
        let source = RawSource::new(path).map_err(dec_err)?;
        let decoder = self.loader.get_decoder(&source).map_err(dec_err)?;
        let params = RawDecodeParams::default();
        let img = match decoder.full_image(&source, &params) {
            Ok(Some(img)) => Some(img),
            _ => match decoder.preview_image(&source, &params) {
                Ok(Some(img)) => Some(img),
                _ => decoder.thumbnail_image(&source, &params).ok().flatten(),
            },
        };
        let Some(img) = img else {
            return Ok(None);
        };
        // camera previews carry the same EXIF orientation as the raw
        let raw = decoder.raw_image(&source, &params, true).map_err(dec_err)?;
        let md = decoder.raw_metadata(&source, &params).map_err(dec_err)?;
        let img = match effective_orientation(&raw, &md) {
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
    }

    fn decode(&self, path: &Path) -> Result<DecodedImage, CoreError> {
        self.decode_impl(path, None, Demosaic::Rawler)
    }

    fn decode_with_profile(
        &self,
        path: &Path,
        profile_path: Option<&Path>,
    ) -> Result<DecodedImage, CoreError> {
        self.decode_impl(path, profile_path, Demosaic::Rawler)
    }

    fn decode_with_options(
        &self,
        path: &Path,
        profile_path: Option<&Path>,
        demosaic: Demosaic,
    ) -> Result<DecodedImage, CoreError> {
        self.decode_impl(path, profile_path, demosaic)
    }
}

impl RawlerDecoder {
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
        let raw = decoder.raw_image(&source, &params, false).map_err(dec_err)?;

        // Camera-native RGB, either from the merawler engine or rawler's built-in
        // demosaic. merawler falls back to rawler for non-Bayer sources.
        let (cam_rgb, w, h): (Vec<[f32; 3]>, usize, usize) = match demosaic.merawler_algo() {
            Some(algo) => match merawler_cam_rgb(&raw, algo) {
                Some(v) => v,
                None => {
                    tracing::info!(
                        algo = demosaic.name(),
                        "merawler unavailable for this CFA (non-Bayer/already demosaiced); using rawler"
                    );
                    rawler_cam_rgb(&raw)?
                }
            },
            None => rawler_cam_rgb(&raw)?,
        };

        // Our colorimetric landing: as-shot WB → cam→Rec.2020 (dual-illum).
        let mut wb = raw.wb_coeffs;
        if wb[0].is_nan() || wb[1] <= 0.0 {
            wb = [1.0, 1.0, 1.0, 1.0];
        }
        // normalize to green = 1
        let g = wb[1];
        let wbn = [wb[0] / g, 1.0, wb[2] / g];

        let cal = CameraCalibration::from_rawler(&raw.color_matrix);
        let cct = cal.estimate_cct(&raw.wb_coeffs);
        let dcp = profile_path.and_then(|p| DcpProfile::load(p).ok());
        let cam2rec = if let Some(ref dcp) = dcp {
            if dcp.matches_camera(&md.make, &md.model) {
                dcp.cam_to_rec2020(&raw.wb_coeffs, &cal).unwrap_or_else(|| {
                    tracing::warn!(
                        file = ?profile_path,
                        "DCP matrix failed; falling back to rawler calibration"
                    );
                    cal.cam_to_rec2020(&raw.wb_coeffs).unwrap_or([
                        [1.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [0.0, 0.0, 1.0],
                    ])
                })
            } else {
                tracing::warn!(
                    profile = %dcp.unique_camera_model,
                    make = %md.make,
                    model = %md.model,
                    "DCP camera mismatch; using rawler calibration"
                );
                cal.cam_to_rec2020(&raw.wb_coeffs).ok_or_else(|| {
                    CoreError::Decode(format!(
                        "no usable color matrix for {} {}",
                        md.make, md.model
                    ))
                })?
            }
        } else if profile_path.is_some() {
            tracing::warn!(file = ?profile_path, "DCP load failed; using rawler calibration");
            cal.cam_to_rec2020(&raw.wb_coeffs).ok_or_else(|| {
                CoreError::Decode(format!(
                    "no usable color matrix for {} {}",
                    md.make, md.model
                ))
            })?
        } else {
            cal.cam_to_rec2020(&raw.wb_coeffs).ok_or_else(|| {
                CoreError::Decode(format!(
                    "no usable color matrix for {} {}",
                    md.make, md.model
                ))
            })?
        };

        let mut data = vec![0.0f32; w * h * 3];
        for (i, px) in cam_rgb.iter().enumerate() {
            let wbd = [px[0] * wbn[0], px[1], px[2] * wbn[2]];
            let rgb = mat_vec(&cam2rec, wbd);
            let o = i * 3;
            // keep headroom; only clamp negatives (out-of-gamut sensor noise)
            data[o] = rgb[0].max(0.0);
            data[o + 1] = rgb[1].max(0.0);
            data[o + 2] = rgb[2].max(0.0);
        }

        let working = RgbF32Buf {
            width: w,
            height: h,
            data,
        }
        .bake_orientation(effective_orientation(&raw, &md));

        let mut meta = self.meta_from(
            path,
            &raw,
            &md,
            working.width as u32,
            working.height as u32,
            Some(cct),
        );
        meta.demosaic = demosaic.name().to_string();
        Ok(DecodedImage { working, meta })
    }
}

/// rawler's built-in path: rescale → demosaic → crops → camera-native RGB.
fn rawler_cam_rgb(raw: &rawler::RawImage) -> Result<(Vec<[f32; 3]>, usize, usize), CoreError> {
    let dev = RawDevelop {
        steps: vec![
            ProcessingStep::Rescale,
            ProcessingStep::Demosaic,
            ProcessingStep::CropActiveArea,
            ProcessingStep::CropDefault,
        ],
    };
    let intermediate = dev.develop_intermediate(raw).map_err(dec_err)?;
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
        Intermediate::Monochrome(px) => {
            (px.data.iter().map(|v| [*v, *v, *v]).collect(), px.width, px.height)
        }
    })
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
    let cfa = merawler::CfaImage {
        width: px.width,
        height: px.height,
        data: px.data,
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
fn crop_to_output(rgb: merawler::RgbImage, raw: &rawler::RawImage) -> (Vec<[f32; 3]>, usize, usize) {
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

    /// End-to-end: the merawler path must produce the same cropped dimensions as
    /// rawler (crop correctness) and a genuinely different image (algo applied),
    /// with finite values. Skips when the sample RAW isn't present.
    #[test]
    fn merawler_path_matches_dims_and_differs() {
        let Ok(home) = std::env::var("HOME") else { return };
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
            assert!(diff > 0.0, "{}: identical to rawler — not applied", algo.name());
        }
    }
}
