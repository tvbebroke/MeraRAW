//! Export — contract F2 OUTPUT side (distinct from the screen view
//! transform): linear Rec.2020 → delivery space with constant-hue gamut
//! compression → OETF → resize-aware sharpen → encode + ICC embed.
//! The last thing the pipeline does, the first thing the client sees.

use crate::color::{self, mat_mul, mat_vec, Mat3};
use crate::error::CoreError;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetSpace {
    Srgb,
    DisplayP3,
    AdobeRgb,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportFormat {
    Jpeg,
    Png,
    Tiff16,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSettings {
    pub format: ExportFormat,
    pub target: TargetSpace,
    /// JPEG quality 1-100
    pub quality: u8,
    /// longest-edge resize; None = full resolution
    pub max_dim: Option<u32>,
    /// output sharpen 0-100, applied AFTER resize (spec 7.2)
    pub sharpen: f32,
    pub dest_dir: String,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            format: ExportFormat::Jpeg,
            target: TargetSpace::Srgb,
            quality: 90,
            max_dim: Some(2560),
            sharpen: 30.0,
            dest_dir: String::new(),
        }
    }
}

/// Display P3 (linear) → XYZ D65 (color-science-reference §2).
const P3_TO_XYZ: Mat3 = [
    [0.4865709, 0.2656677, 0.1982173],
    [0.2289746, 0.6917385, 0.0792869],
    [0.0000000, 0.0451134, 1.0439444],
];

/// Adobe RGB (1998) linear → XYZ D65 (derived from primaries; standard).
const ADOBE_TO_XYZ: Mat3 = [
    [0.5766690, 0.1855582, 0.1882286],
    [0.2973450, 0.6273636, 0.0752915],
    [0.0270314, 0.0706889, 0.9913375],
];

fn target_from_rec2020(target: TargetSpace) -> Mat3 {
    let to_xyz = match target {
        TargetSpace::Srgb => color::SRGB_TO_XYZ,
        TargetSpace::DisplayP3 => P3_TO_XYZ,
        TargetSpace::AdobeRgb => ADOBE_TO_XYZ,
    };
    let from_xyz = color::mat_inverse(&to_xyz).expect("target matrix invertible");
    mat_mul(&from_xyz, &color::REC2020_TO_XYZ)
}

fn oetf(target: TargetSpace, v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    match target {
        // sRGB + Display P3 share the sRGB curve (§3)
        TargetSpace::Srgb | TargetSpace::DisplayP3 => {
            if v <= 0.0031308 {
                12.92 * v
            } else {
                1.055 * v.powf(1.0 / 2.4) - 0.055
            }
        }
        TargetSpace::AdobeRgb => v.powf(1.0 / 2.19921875),
    }
}

/// Display look — MIRRORS present.wgsl `view_look` so export == preview.
/// Reinhard shoulder + gentle S-curve on luminance + saturation.
fn view_look(rgb: [f32; 3], camera: bool) -> [f32; 3] {
    let (gain, contrast, sat) = if camera {
        (1.6f32, 0.34f32, 1.22f32)
    } else {
        (1.15f32, 0.12f32, 1.0f32)
    };
    let lw = 4.0f32;
    let l = 0.2126 * rgb[0].max(0.0) + 0.7152 * rgb[1].max(0.0) + 0.0722 * rgb[2].max(0.0);
    if l <= 1e-8 {
        return [0.0; 3];
    }
    let x = l * gain;
    let r = x * (1.0 + x / (lw * lw)) / (1.0 + x);
    let s = 0.5 - 0.5 * (r.clamp(0.0, 1.0) * std::f32::consts::PI).cos();
    let ld = (r + (s - r) * contrast).clamp(0.0, 1.0);
    let k = ld / l;
    let mut out = [rgb[0] * k, rgb[1] * k, rgb[2] * k];
    let l2 = 0.2126 * out[0].max(0.0) + 0.7152 * out[1].max(0.0) + 0.0722 * out[2].max(0.0);
    for c in out.iter_mut() {
        *c = (l2 + (*c - l2) * sat).max(0.0);
    }
    out
}

/// Constant-hue gamut compression: mix toward luma until in-gamut (≥0).
fn gamut_compress(rgb: [f32; 3]) -> [f32; 3] {
    let luma = 0.2126 * rgb[0].max(0.0) + 0.7152 * rgb[1].max(0.0) + 0.0722 * rgb[2].max(0.0);
    let m = rgb[0].min(rgb[1]).min(rgb[2]);
    if m >= 0.0 || luma <= 0.0 {
        return rgb.map(|c| c.max(0.0));
    }
    let t = (-m / (luma - m).max(1e-6)).clamp(0.0, 1.0);
    rgb.map(|c| (c + (luma - c) * t).max(0.0))
}

/// linear Rec.2020 RGB f32 → encoded target-space bytes (8-bit) or u16.
pub struct EncodedImage {
    pub width: u32,
    pub height: u32,
    pub rgb8: Vec<u8>,
    pub rgb16: Vec<u16>,
}

pub fn output_transform(
    linear: &[f32],
    width: u32,
    height: u32,
    target: TargetSpace,
    want16: bool,
    camera_look: bool,
) -> EncodedImage {
    let m = target_from_rec2020(target);
    let px = (width * height) as usize;
    let mut rgb8 = if want16 { Vec::new() } else { Vec::with_capacity(px * 3) };
    let mut rgb16 = if want16 { Vec::with_capacity(px * 3) } else { Vec::new() };
    for i in 0..px {
        let lin = [linear[i * 3], linear[i * 3 + 1], linear[i * 3 + 2]];
        let looked = view_look(lin, camera_look);
        let target_lin = gamut_compress(mat_vec(&m, looked));
        for c in target_lin {
            let e = oetf(target, c);
            if want16 {
                rgb16.push((e * 65535.0).round() as u16);
            } else {
                rgb8.push((e * 255.0).round() as u8);
            }
        }
    }
    EncodedImage {
        width,
        height,
        rgb8,
        rgb16,
    }
}

/// Lanczos resize in LINEAR space (before the output transform — quality).
pub fn resize_linear(
    linear: Vec<f32>,
    width: u32,
    height: u32,
    max_dim: u32,
) -> (Vec<f32>, u32, u32) {
    if width.max(height) <= max_dim {
        return (linear, width, height);
    }
    let img = image::Rgb32FImage::from_raw(width, height, linear).expect("linear buffer");
    let scale = max_dim as f32 / width.max(height) as f32;
    let (nw, nh) = (
        ((width as f32 * scale) as u32).max(1),
        ((height as f32 * scale) as u32).max(1),
    );
    let resized = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Lanczos3);
    (resized.into_raw(), nw, nh)
}

/// Output sharpen (USM on 8-bit luma, after resize — spec 7.2).
pub fn output_sharpen8(rgb: &mut [u8], width: u32, height: u32, amount: f32) {
    if amount <= 0.0 || width < 4 || height < 4 {
        return;
    }
    let k = amount / 100.0 * 0.8;
    let (w, h) = (width as usize, height as usize);
    let luma: Vec<f32> = (0..w * h)
        .map(|i| {
            0.2126 * rgb[i * 3] as f32 + 0.7152 * rgb[i * 3 + 1] as f32 + 0.0722 * rgb[i * 3 + 2] as f32
        })
        .collect();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            let blur = (luma[i - 1] + luma[i + 1] + luma[i - w] + luma[i + w] + luma[i] * 4.0) / 8.0;
            let high = luma[i] - blur;
            let gain = 1.0 + k * high / luma[i].max(8.0);
            for c in 0..3 {
                let v = rgb[i * 3 + c] as f32 * gain;
                rgb[i * 3 + c] = v.clamp(0.0, 255.0) as u8;
            }
        }
    }
}

fn icc_bytes(target: TargetSpace) -> Option<Vec<u8>> {
    let path = match target {
        TargetSpace::Srgb => "/System/Library/ColorSync/Profiles/sRGB Profile.icc",
        TargetSpace::DisplayP3 => "/System/Library/ColorSync/Profiles/Display P3.icc",
        TargetSpace::AdobeRgb => "/System/Library/ColorSync/Profiles/AdobeRGB1998.icc",
    };
    std::fs::read(path).ok()
}

/// Splice ICC APP2 segments into a JPEG right after SOI (the standard
/// ICC_PROFILE marker chunking).
fn embed_icc_jpeg(jpeg: Vec<u8>, icc: &[u8]) -> Vec<u8> {
    const CHUNK: usize = 65519 - 14; // marker payload minus ICC header
    let chunks: Vec<&[u8]> = icc.chunks(CHUNK).collect();
    let total = chunks.len() as u8;
    let mut out = Vec::with_capacity(jpeg.len() + icc.len() + 64);
    out.extend_from_slice(&jpeg[..2]); // SOI
    for (i, chunk) in chunks.iter().enumerate() {
        let payload_len = 2 + 12 + 2 + chunk.len(); // len + "ICC_PROFILE\0" + seq/total + data
        out.extend_from_slice(&[0xFF, 0xE2]);
        out.extend_from_slice(&((payload_len) as u16).to_be_bytes());
        out.extend_from_slice(b"ICC_PROFILE\0");
        out.push(i as u8 + 1);
        out.push(total);
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(&jpeg[2..]);
    out
}

/// Encode + write. Returns the output path.
pub fn encode_and_write(
    enc: &EncodedImage,
    settings: &ExportSettings,
    source_path: &str,
) -> Result<PathBuf, CoreError> {
    let stem = Path::new(source_path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "export".into());
    let dir = if settings.dest_dir.is_empty() {
        Path::new(source_path)
            .parent()
            .unwrap_or(Path::new("."))
            .join("exports")
    } else {
        PathBuf::from(&settings.dest_dir)
    };
    std::fs::create_dir_all(&dir)?;
    let err = |e: image::ImageError| CoreError::Io(format!("encode: {e}"));

    let path = match settings.format {
        ExportFormat::Jpeg => {
            let p = dir.join(format!("{stem}.jpg"));
            let img = image::RgbImage::from_raw(enc.width, enc.height, enc.rgb8.clone())
                .ok_or_else(|| CoreError::Io("buffer".into()))?;
            let mut bytes = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(
                &mut bytes,
                settings.quality.clamp(1, 100),
            )
            .encode_image(&img)
            .map_err(err)?;
            let bytes = match icc_bytes(settings.target) {
                Some(icc) => embed_icc_jpeg(bytes, &icc),
                None => bytes,
            };
            std::fs::write(&p, bytes)?;
            p
        }
        ExportFormat::Png => {
            // note: ICC embed for PNG (iCCP) not supported by the encoder —
            // color is correct in the target space; profile tag deferred
            let p = dir.join(format!("{stem}.png"));
            let img = image::RgbImage::from_raw(enc.width, enc.height, enc.rgb8.clone())
                .ok_or_else(|| CoreError::Io("buffer".into()))?;
            img.save(&p).map_err(err)?;
            p
        }
        ExportFormat::Tiff16 => {
            let p = dir.join(format!("{stem}.tif"));
            let img: image::ImageBuffer<image::Rgb<u16>, Vec<u16>> =
                image::ImageBuffer::from_raw(enc.width, enc.height, enc.rgb16.clone())
                    .ok_or_else(|| CoreError::Io("buffer".into()))?;
            img.save(&p).map_err(err)?;
            p
        }
    };
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_stays_neutral_in_every_target() {
        for target in [TargetSpace::Srgb, TargetSpace::DisplayP3, TargetSpace::AdobeRgb] {
            let lin = [0.18f32, 0.18, 0.18];
            let e = output_transform(&lin, 1, 1, target, false, false);
            let px = &e.rgb8;
            assert!(
                (px[0] as i32 - px[1] as i32).abs() <= 1
                    && (px[1] as i32 - px[2] as i32).abs() <= 1,
                "{target:?}: {px:?}"
            );
        }
    }

    #[test]
    fn highlights_roll_not_clip() {
        // scene 2.0 (over white) must encode below 255 (rolloff) not slam
        let e = output_transform(&[2.0, 2.0, 2.0], 1, 1, TargetSpace::Srgb, false, false);
        assert!(e.rgb8[0] < 255, "got {}", e.rgb8[0]);
        // but well above mid-gray
        assert!(e.rgb8[0] > 200);
    }

    #[test]
    fn out_of_gamut_compresses_at_positive_values() {
        // saturated Rec.2020 green is outside sRGB → must come back ≥0
        let e = output_transform(&[0.0, 0.5, 0.0], 1, 1, TargetSpace::Srgb, false, false);
        assert!(e.rgb8.iter().all(|v| *v <= 255));
        // green channel dominates, others not zero-slammed by clipping math
        assert!(e.rgb8[1] > e.rgb8[0] && e.rgb8[1] > e.rgb8[2]);
    }

    #[test]
    fn icc_embed_keeps_valid_jpeg_structure() {
        let img = image::RgbImage::from_pixel(8, 8, image::Rgb([128, 128, 128]));
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90)
            .encode_image(&img)
            .unwrap();
        let icc = vec![0xAAu8; 100_000]; // forces multi-chunk
        let out = embed_icc_jpeg(jpeg, &icc);
        assert_eq!(&out[..2], &[0xFF, 0xD8], "SOI intact");
        assert_eq!(&out[2..4], &[0xFF, 0xE2], "APP2 follows SOI");
        let needle = b"ICC_PROFILE\0";
        let count = out.windows(needle.len()).filter(|w| w == needle).count();
        assert!(count >= 2, "expected multiple ICC chunks, got {count}");
        // decoder still reads it
        let decoded = image::load_from_memory(&out).expect("jpeg decodes");
        assert_eq!(decoded.width(), 8);
    }

    #[test]
    fn resize_caps_long_edge() {
        let lin = vec![0.2f32; 100 * 50 * 3];
        let (out, w, h) = resize_linear(lin, 100, 50, 40);
        assert_eq!((w, h), (40, 20));
        assert_eq!(out.len(), (40 * 20 * 3) as usize);
    }

    #[test]
    fn adobe_matrix_white_is_d65() {
        let xyz = mat_vec(&ADOBE_TO_XYZ, [1.0, 1.0, 1.0]);
        assert!((xyz[0] - 0.9505).abs() < 0.01, "{xyz:?}");
        assert!((xyz[1] - 1.0).abs() < 0.01);
        assert!((xyz[2] - 1.0889).abs() < 0.01);
    }
}
