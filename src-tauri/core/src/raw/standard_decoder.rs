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

pub const STD_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "tif", "tiff", "webp", "bmp", "gif"];

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
        other => other.to_uppercase(),
    }
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
        // Cheap: reads only the header for dimensions (depth refined on decode).
        let (w, h) = image::image_dimensions(path)
            .map_err(|e| CoreError::Decode(format!("image dims: {e}")))?;
        Ok(rendered_meta(path, w, h, 8))
    }

    fn embedded_preview(
        &self,
        path: &Path,
        max_dim: u32,
    ) -> Result<Option<(Vec<u8>, u32, u32)>, CoreError> {
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
        let dynimg =
            image::open(path).map_err(|e| CoreError::Decode(format!("image open: {e}")))?;
        let bit_depth = match dynimg.color() {
            image::ColorType::Rgb16
            | image::ColorType::Rgba16
            | image::ColorType::L16
            | image::ColorType::La16 => 16,
            _ => 8,
        };
        // to_rgb32f scales the encoded samples to [0,1] WITHOUT gamma decoding,
        // so these are sRGB-encoded values we then linearize ourselves.
        let rgb = dynimg.to_rgb32f();
        let (w, h) = (rgb.width() as usize, rgb.height() as usize);
        let src = rgb.as_raw(); // len w*h*3, interleaved, sRGB-encoded
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
