//! Standard (already-rendered) image decode — JPEG/PNG/TIFF/WebP/BMP/GIF via
//! the `image` crate. Contract B1 sibling of the RAW decoder.
//!
//! These are display-referred: sRGB-encoded, already white-balanced and
//! tone-mapped. We linearize (sRGB EOTF) and convert into the app's linear
//! Rec.2020 working space so the edit nodes operate correctly, and tag the
//! image `Rendered` so the pipeline skips the camera stages (WB / DCP look /
//! filmic view transform) that would double-process it.

use super::{DecodedImage, Decoder, ImageKind, ImageMeta};
use crate::color::{mat_mul, mat_vec, Mat3, SRGB_TO_XYZ, XYZ_TO_REC2020};
use crate::error::CoreError;
use crate::image::RgbF32Buf;
use std::path::Path;

pub const STD_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "tif", "tiff", "webp", "bmp", "gif", "jxl", "heic", "heif", "hif",
];

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
            let dynimg = image::open(&tmp)
                .map_err(|e| CoreError::Decode(format!("heic->png read: {e}")))?;
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
fn decode_jxl_srgb(path: &Path) -> Result<(Vec<f32>, usize, usize), CoreError> {
    use jxl_oxide::JxlImage;
    let image = JxlImage::builder()
        .open(path)
        .map_err(|e| CoreError::Decode(format!("jxl open: {e}")))?;
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
    Ok((rgb, w, h))
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
        return Err(CoreError::Decode(
            "HEIC/HEIF import is only available on macOS in this build.".into(),
        ));
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
            return Err(CoreError::Decode("sips: could not read HEIC dimensions".into()));
        }
        Ok((w, h))
    }
}

/// linear sRGB → linear Rec.2020, from the pinned color-science matrices.
fn srgb_to_rec2020() -> Mat3 {
    mat_mul(&XYZ_TO_REC2020, &SRGB_TO_XYZ)
}

/// sRGB EOTF: gamma-encoded [0,1] → linear light (color-science-reference §3).
fn srgb_eotf(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
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
        other => other.to_uppercase(),
    }
}

/// linear Rec.2020 working data from interleaved sRGB-encoded RGB f32.
fn srgb_rgb_to_working(src: &[f32], w: usize, h: usize) -> Vec<f32> {
    let m = srgb_to_rec2020();
    let mut data = vec![0f32; w * h * 3];
    for i in 0..(w * h) {
        let lin = [
            srgb_eotf(src[i * 3]),
            srgb_eotf(src[i * 3 + 1]),
            srgb_eotf(src[i * 3 + 2]),
        ];
        let rec = mat_vec(&m, lin);
        data[i * 3] = rec[0].max(0.0);
        data[i * 3 + 1] = rec[1].max(0.0);
        data[i * 3 + 2] = rec[2].max(0.0);
    }
    data
}

fn rendered_meta(path: &Path, w: u32, h: u32, bit_depth: u8) -> ImageMeta {
    ImageMeta {
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
    }
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
        } else {
            image::image_dimensions(path)
                .map_err(|e| CoreError::Decode(format!("image dims: {e}")))?
        };
        Ok(rendered_meta(path, w, h, 8))
    }

    fn embedded_preview(
        &self,
        path: &Path,
        max_dim: u32,
    ) -> Result<Option<(Vec<u8>, u32, u32)>, CoreError> {
        if is_jxl(path) || is_heic(path) {
            // No fast embedded preview for these — decode then downscale.
            let (rgb, w, h) = if is_jxl(path) {
                decode_jxl_srgb(path)?
            } else {
                decode_heic_srgb(path)?
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
            let (rgb, w, h) = decode_jxl_srgb(path)?;
            (rgb, w, h, 0u8) // JXL depth varies (8/16/f32) — omit rather than guess
        } else if is_heic(path) {
            let (rgb, w, h) = decode_heic_srgb(path)?;
            (rgb, w, h, 8u8) // sips → 8-bit PNG
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
            // to_rgb32f scales samples to [0,1] WITHOUT gamma decoding — sRGB-encoded.
            let rgb = dynimg.to_rgb32f();
            let (w, h) = (rgb.width() as usize, rgb.height() as usize);
            (rgb.into_raw(), w, h, bit_depth)
        };
        let data = srgb_rgb_to_working(&src, w, h);
        let working = RgbF32Buf {
            width: w,
            height: h,
            data,
        };
        let meta = rendered_meta(path, w as u32, h as u32, bit_depth);
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
        let (r, g, b) = (out.working.data[0], out.working.data[1], out.working.data[2]);
        assert!((r - 0.216).abs() < 0.02, "sRGB 128 → linear ~0.216, got {r}");
        assert!((r - g).abs() < 0.01 && (g - b).abs() < 0.01, "gray stays neutral");
        let _ = std::fs::remove_file(&path);
    }
}
