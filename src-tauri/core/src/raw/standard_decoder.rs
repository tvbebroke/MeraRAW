//! Standard (already-rendered) image decode — JPEG/PNG/TIFF/WebP/BMP/GIF via
//! the `image` crate. Contract B1 sibling of the RAW decoder.
//!
//! These are display-referred (already white-balanced / tone-mapped). We
//! honor an embedded ICC when present (phase 11.2), otherwise assume sRGB,
//! then convert into linear Rec.2020 so edit nodes operate correctly. Tagged
//! `Rendered` so the pipeline skips camera stages (WB / DCP / filmic).

use super::{DecodedImage, Decoder, ImageKind, ImageMeta};
use crate::error::CoreError;
use crate::image::RgbF32Buf;
use crate::metadata;
use std::path::Path;

pub const STD_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "tif", "tiff", "webp", "bmp", "gif", "jxl", "heic", "heif", "hif", "psd",
];

fn is_psd(path: &Path) -> bool {
    path.extension()
        .map(|e| e.eq_ignore_ascii_case("psd"))
        .unwrap_or(false)
}

/// Decode a Photoshop PSD's flattened composite (fast path — spec §4). Layered
/// intelligence is ignored; only RGB color mode for now (CMYK/Lab need ICC).
fn decode_psd_srgb(path: &Path) -> Result<(Vec<f32>, usize, usize), CoreError> {
    use psd::{ColorMode, Psd};
    let bytes = std::fs::read(path).map_err(|e| CoreError::Decode(format!("psd read: {e}")))?;
    let doc =
        Psd::from_bytes(&bytes).map_err(|e| CoreError::Decode(format!("psd parse: {e:?}")))?;
    if doc.color_mode() != ColorMode::Rgb {
        return Err(CoreError::Decode(format!(
            "PSD color mode {:?} not supported yet — only RGB",
            doc.color_mode()
        )));
    }
    let (w, h) = (doc.width() as usize, doc.height() as usize);
    let rgba = doc.rgba(); // flattened composite, RGBA8, sRGB-encoded
    if rgba.len() < w * h * 4 {
        return Err(CoreError::Decode("psd: short composite buffer".into()));
    }
    let mut rgb = vec![0f32; w * h * 3];
    for i in 0..(w * h) {
        rgb[i * 3] = rgba[i * 4] as f32 / 255.0;
        rgb[i * 3 + 1] = rgba[i * 4 + 1] as f32 / 255.0;
        rgb[i * 3 + 2] = rgba[i * 4 + 2] as f32 / 255.0;
    }
    Ok((rgb, w, h))
}

/// Cheap PSD dimensions from the 26-byte header (big-endian). PSB ("8BPB")
/// is not supported by the `psd` crate, so it errors here.
fn psd_dimensions(path: &Path) -> Result<(u32, u32), CoreError> {
    use std::io::Read;
    let mut buf = [0u8; 26];
    let mut f =
        std::fs::File::open(path).map_err(|e| CoreError::Decode(format!("psd open: {e}")))?;
    f.read_exact(&mut buf)
        .map_err(|e| CoreError::Decode(format!("psd header: {e}")))?;
    if &buf[0..4] != b"8BPS" {
        return Err(CoreError::Decode(
            "not a PSD (PSB is not supported yet)".into(),
        ));
    }
    let h = u32::from_be_bytes([buf[14], buf[15], buf[16], buf[17]]);
    let w = u32::from_be_bytes([buf[18], buf[19], buf[20], buf[21]]);
    Ok((w, h))
}

fn is_jxl(path: &Path) -> bool {
    path.extension()
        .map(|e| e.eq_ignore_ascii_case("jxl"))
        .unwrap_or(false)
}

fn is_heic(path: &Path) -> bool {
    path.extension()
        .map(|e| {
            let e = e.to_string_lossy().to_lowercase();
            e == "heic" || e == "heif" || e == "hif"
        })
        .unwrap_or(false)
}

/// Decode HEIC/HEIF via macOS `sips` (converts to a temp PNG, which we then
/// read). No pure-Rust permissive HEVC decoder exists yet; the pure-Rust
/// `heic` crate is AGPL, so this mirrors the sips-based HEIC *export* path.
/// macOS-only for now (cross-platform HEIC needs the C libheif).
fn decode_heic_srgb(path: &Path) -> Result<(Vec<f32>, usize, usize), CoreError> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err(CoreError::Decode(
            "HEIC/HEIF import is only available on macOS in this build.".into(),
        ))
    }
    #[cfg(target_os = "macos")]
    {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "heic".into());
        let tmp = std::env::temp_dir().join(format!("meraraw-heic-{stem}-{nanos}.png"));
        let out = std::process::Command::new("/usr/bin/sips")
            .args(["-s", "format", "png"])
            .arg(path)
            .arg("--out")
            .arg(&tmp)
            .output()
            .map_err(|e| CoreError::Decode(format!("sips spawn: {e}")))?;
        if !out.status.success() {
            let _ = std::fs::remove_file(&tmp);
            return Err(CoreError::Decode(format!(
                "sips heic decode: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        let result = (|| {
            let dynimg =
                image::open(&tmp).map_err(|e| CoreError::Decode(format!("heic->png read: {e}")))?;
            let rgb = dynimg.to_rgb32f();
            let (w, h) = (rgb.width() as usize, rgb.height() as usize);
            Ok((rgb.into_raw(), w, h))
        })();
        let _ = std::fs::remove_file(&tmp);
        result
    }
}

/// Decode a JPEG XL to interleaved sRGB-encoded RGB f32 (pure-Rust jxl-oxide).
/// Output is in the file's color encoding (sRGB for the common case), which
/// the caller linearizes like any other rendered image.
fn decode_jxl_srgb(path: &Path) -> Result<(Vec<f32>, usize, usize, u8), CoreError> {
    use jxl_oxide::JxlImage;
    let image = JxlImage::builder()
        .open(path)
        .map_err(|e| CoreError::Decode(format!("jxl open: {e}")))?;
    // Real encoded bit depth from the header (8/10/12/16-bit int or float).
    let bit_depth = image
        .image_header()
        .metadata
        .bit_depth
        .bits_per_sample()
        .min(255) as u8;
    let render = image
        .render_frame(0)
        .map_err(|e| CoreError::Decode(format!("jxl render: {e}")))?;
    let fb = render.image_all_channels();
    let (w, h, ch) = (fb.width(), fb.height(), fb.channels());
    let buf = fb.buf();
    let mut rgb = vec![0f32; w * h * 3];
    for i in 0..(w * h) {
        let base = i * ch;
        if ch >= 3 {
            rgb[i * 3] = buf[base];
            rgb[i * 3 + 1] = buf[base + 1];
            rgb[i * 3 + 2] = buf[base + 2];
        } else {
            let v = buf[base]; // grayscale
            rgb[i * 3] = v;
            rgb[i * 3 + 1] = v;
            rgb[i * 3 + 2] = v;
        }
    }
    Ok((rgb, w, h, bit_depth))
}

fn jxl_dimensions(path: &Path) -> Result<(u32, u32), CoreError> {
    use jxl_oxide::JxlImage;
    let image = JxlImage::builder()
        .open(path)
        .map_err(|e| CoreError::Decode(format!("jxl open: {e}")))?;
    Ok((image.width(), image.height()))
}

/// Cheap HEIC dimensions via `sips -g` (no pixel decode).
fn heic_dimensions(path: &Path) -> Result<(u32, u32), CoreError> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err(CoreError::Decode(
            "HEIC/HEIF import is only available on macOS in this build.".into(),
        ))
    }
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("/usr/bin/sips")
            .args(["-g", "pixelWidth", "-g", "pixelHeight"])
            .arg(path)
            .output()
            .map_err(|e| CoreError::Decode(format!("sips spawn: {e}")))?;
        let text = String::from_utf8_lossy(&out.stdout);
        let (mut w, mut h) = (0u32, 0u32);
        for line in text.lines() {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("pixelWidth:") {
                w = v.trim().parse().unwrap_or(0);
            } else if let Some(v) = line.strip_prefix("pixelHeight:") {
                h = v.trim().parse().unwrap_or(0);
            }
        }
        if w == 0 || h == 0 {
            return Err(CoreError::Decode(
                "sips: could not read HEIC dimensions".into(),
            ));
        }
        Ok((w, h))
    }
}

fn format_tag(path: &Path) -> String {
    let e = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match e.as_str() {
        "jpg" | "jpeg" => "JPEG".into(),
        "tif" | "tiff" => "TIFF".into(),
        "jxl" => "JXL".into(),
        "heic" | "heif" | "hif" => "HEIC".into(),
        "psd" => "PSD".into(),
        other => other.to_uppercase(),
    }
}

fn rendered_meta(
    path: &Path,
    w: u32,
    h: u32,
    bit_depth: u8,
    color: &metadata::InputColorInfo,
) -> ImageMeta {
    let mut meta = ImageMeta {
        path: path.to_string_lossy().into_owned(),
        kind: ImageKind::Rendered,
        format: format_tag(path),
        bit_depth,
        camera_make: String::new(),
        camera_model: String::new(),
        lens: None,
        iso: None,
        shutter: None,
        aperture: None,
        focal_mm: None,
        captured_at: None,
        width: w,
        height: h,
        orientation: "Normal".into(),
        as_shot_wb: [1.0, 1.0, 1.0], // already balanced — WB starts neutral
        estimated_cct: Some(6500.0), // D65
        camera_profile: None,
        available_profiles: Vec::new(),
        available_profile_files: Vec::new(),
        demosaic: String::new(), // n/a for already-rendered images
        available_demosaic: Vec::new(),
        gps_lat: None,
        gps_lon: None,
        input_color_space: Some(color.label.clone()),
        video: None,
    };
    metadata::enrich_from_file(path, &mut meta);
    meta
}

#[derive(Default)]
pub struct StandardDecoder;

impl Decoder for StandardDecoder {
    fn probe(&self, path: &Path) -> bool {
        path.extension()
            .map(|e| STD_EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str()))
            .unwrap_or(false)
    }

    fn metadata(&self, path: &Path) -> Result<ImageMeta, CoreError> {
        // Cheap: header-only dimensions (depth refined on full decode).
        let (w, h) = if is_jxl(path) {
            jxl_dimensions(path)?
        } else if is_heic(path) {
            heic_dimensions(path)?
        } else if is_psd(path) {
            psd_dimensions(path)?
        } else {
            image::image_dimensions(path)
                .map_err(|e| CoreError::Decode(format!("image dims: {e}")))?
        };
        let color = metadata::probe_input_color(path);
        Ok(rendered_meta(path, w, h, 8, &color))
    }

    fn embedded_preview(
        &self,
        path: &Path,
        max_dim: u32,
    ) -> Result<Option<(Vec<u8>, u32, u32)>, CoreError> {
        if is_jxl(path) || is_heic(path) || is_psd(path) {
            // No fast embedded preview for these — decode then downscale.
            let (rgb, w, h) = if is_jxl(path) {
                let (rgb, w, h, _depth) = decode_jxl_srgb(path)?;
                (rgb, w, h)
            } else if is_heic(path) {
                decode_heic_srgb(path)?
            } else {
                decode_psd_srgb(path)?
            };
            let full = image::Rgb32FImage::from_raw(w as u32, h as u32, rgb)
                .ok_or_else(|| CoreError::Decode("decoded buffer".into()))?;
            let thumb = image::DynamicImage::ImageRgb32F(full).thumbnail(max_dim, max_dim);
            let rgba = thumb.to_rgba8();
            let (tw, th) = (rgba.width(), rgba.height());
            return Ok(Some((rgba.into_raw(), tw, th)));
        }
        let img = image::open(path).map_err(|e| CoreError::Decode(format!("image open: {e}")))?;
        let thumb = img.thumbnail(max_dim, max_dim); // aspect-preserving downscale
        let rgba = thumb.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());
        Ok(Some((rgba.into_raw(), w, h)))
    }

    fn decode_with_profile(
        &self,
        path: &Path,
        _profile_path: Option<&Path>,
    ) -> Result<DecodedImage, CoreError> {
        // Get interleaved sRGB-encoded RGB f32 + source bit depth.
        let (src, w, h, bit_depth) = if is_jxl(path) {
            let (rgb, w, h, depth) = decode_jxl_srgb(path)?;
            (rgb, w, h, depth) // real encoded depth from JXL header
        } else if is_heic(path) {
            let (rgb, w, h) = decode_heic_srgb(path)?;
            (rgb, w, h, 8u8) // sips → 8-bit PNG
        } else if is_psd(path) {
            let (rgb, w, h) = decode_psd_srgb(path)?;
            (rgb, w, h, 8u8) // 8-bit RGB composite
        } else {
            let dynimg =
                image::open(path).map_err(|e| CoreError::Decode(format!("image open: {e}")))?;
            let bit_depth = match dynimg.color() {
                image::ColorType::Rgb16
                | image::ColorType::Rgba16
                | image::ColorType::L16
                | image::ColorType::La16 => 16,
                _ => 8,
            };
            // to_rgb32f scales samples to [0,1] WITHOUT TRC decoding — encoded.
            let rgb = dynimg.to_rgb32f();
            let (w, h) = (rgb.width() as usize, rgb.height() as usize);
            (rgb.into_raw(), w, h, bit_depth)
        };
        // HEIC/JXL/PSD paths don't carry a reliable ICC yet → sRGB fallback.
        let color = if is_jxl(path) || is_heic(path) || is_psd(path) {
            metadata::InputColorInfo {
                label: "sRGB".into(),
                icc: None,
            }
        } else {
            metadata::probe_input_color(path)
        };
        let data = metadata::encoded_rgb_to_working(&src, w, h, color.icc.as_deref())?;
        let stats = crate::pipeline::RgbStats::from_rgb(&data, 0.995, 0.004);
        let working = RgbF32Buf {
            width: w,
            height: h,
            data,
        };
        let meta = rendered_meta(path, w as u32, h as u32, bit_depth, &color);
        crate::pipeline::log_import(
            "decoded-working",
            meta.kind,
            &meta.format,
            meta.bit_depth,
            color.label.as_str(),
            true,
            "",
            None,
            &stats,
        );
        Ok(DecodedImage { working, meta })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_png_as_rendered_linear() {
        // mid-gray 128 sRGB → linear ~0.216; neutral stays neutral in Rec.2020.
        let img = image::RgbImage::from_pixel(4, 2, image::Rgb([128, 128, 128]));
        let path = std::env::temp_dir().join("meraraw-std-decoder-test.png");
        img.save(&path).unwrap();

        let dec = StandardDecoder;
        assert!(dec.probe(&path));
        let out = dec.decode_with_profile(&path, None).unwrap();

        assert_eq!(out.meta.kind, ImageKind::Rendered);
        assert_eq!(out.meta.format, "PNG");
        assert_eq!(out.meta.bit_depth, 8);
        assert_eq!((out.working.width, out.working.height), (4, 2));
        assert_eq!(out.working.data.len(), 4 * 2 * 3);
        let (r, g, b) = (
            out.working.data[0],
            out.working.data[1],
            out.working.data[2],
        );
        assert!(
            (r - 0.216).abs() < 0.02,
            "sRGB 128 → linear ~0.216, got {r}"
        );
        assert!(
            (r - g).abs() < 0.01 && (g - b).abs() < 0.01,
            "gray stays neutral"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn round_trips_all_image_crate_formats() {
        let base = image::RgbImage::from_fn(8, 6, |x, y| {
            image::Rgb([((x * 30) % 256) as u8, ((y * 40) % 256) as u8, 100])
        });
        let dir = std::env::temp_dir();
        for (ext, fmt) in [
            ("jpg", "JPEG"),
            ("png", "PNG"),
            ("tiff", "TIFF"),
            ("webp", "WEBP"),
            ("bmp", "BMP"),
            ("gif", "GIF"),
        ] {
            let path = dir.join(format!("meraraw-fmt-{ext}.{ext}"));
            base.save(&path)
                .unwrap_or_else(|e| panic!("save {ext}: {e}"));
            let dec = crate::raw::decoder_for(&path);
            assert!(dec.probe(&path), "probe {ext}");
            let out = dec
                .decode_with_profile(&path, None)
                .unwrap_or_else(|e| panic!("decode {ext}: {e:?}"));
            assert_eq!(out.meta.kind, ImageKind::Rendered, "{ext} kind");
            assert_eq!(out.meta.format, fmt, "{ext} format");
            assert_eq!(
                (out.working.width, out.working.height),
                (8, 6),
                "{ext} dims"
            );
            assert_eq!(out.working.data.len(), 8 * 6 * 3, "{ext} buf len");
            let m = dec
                .metadata(&path)
                .unwrap_or_else(|e| panic!("meta {ext}: {e:?}"));
            assert_eq!((m.width, m.height), (8, 6), "{ext} meta dims");
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn decodes_heic_via_sips() {
        let base = image::RgbImage::from_pixel(16, 12, image::Rgb([120, 60, 200]));
        let dir = std::env::temp_dir();
        let png = dir.join("meraraw-heic-src.png");
        base.save(&png).unwrap();
        let heic = dir.join("meraraw-heic-test.heic");
        let out = std::process::Command::new("/usr/bin/sips")
            .args(["-s", "format", "heic"])
            .arg(&png)
            .arg("--out")
            .arg(&heic)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&png);
        if !out.status.success() || !heic.exists() {
            eprintln!("skip: sips heic encode unavailable");
            return;
        }
        let dec = crate::raw::decoder_for(&heic);
        assert!(dec.probe(&heic));
        let d = dec.decode_with_profile(&heic, None).unwrap();
        assert_eq!(d.meta.kind, ImageKind::Rendered);
        assert_eq!(d.meta.format, "HEIC");
        assert_eq!((d.working.width, d.working.height), (16, 12));
        assert_eq!(d.working.data.len(), 16 * 12 * 3);
        let _ = std::fs::remove_file(&heic);
    }
}
