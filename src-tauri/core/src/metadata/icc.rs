//! Input ICC for rendered images (phase 11.2): extract embedded profiles and
//! convert display-referred RGB into linear Rec.2020 working space.

use crate::color::{mat_mul, mat_vec, Mat3, SRGB_TO_XYZ, XYZ_TO_REC2020};
use crate::error::CoreError;
use lcms2::{
    CIExyY, CIExyYTRIPLE, InfoType, Intent, Locale, PixelFormat, Profile, ToneCurve, Transform,
};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// Result of probing a file for an input color space.
#[derive(Debug, Clone)]
pub struct InputColorInfo {
    /// Human label for UI / ImageMeta (e.g. "Display P3", "sRGB").
    pub label: String,
    /// Raw ICC bytes when present (not retained after convert).
    pub icc: Option<Vec<u8>>,
}

/// Probe embedded ICC (and fall back to sRGB when absent).
pub fn probe_input_color(path: &Path) -> InputColorInfo {
    match extract_icc(path) {
        Some(icc) => {
            let label = profile_label(&icc).unwrap_or_else(|| "Embedded ICC".into());
            InputColorInfo {
                label,
                icc: Some(icc),
            }
        }
        None => InputColorInfo {
            label: "sRGB".into(),
            icc: None,
        },
    }
}

/// Convert interleaved encoded RGB f32 [0,1] → linear Rec.2020.
/// Uses LCMS when `icc` is present; otherwise the pinned sRGB matrix path.
pub fn encoded_rgb_to_working(
    src: &[f32],
    width: usize,
    height: usize,
    icc: Option<&[u8]>,
) -> Result<Vec<f32>, CoreError> {
    let n = width * height;
    if src.len() < n * 3 {
        return Err(CoreError::Decode("rgb buffer short".into()));
    }
    match icc {
        None => Ok(srgb_encoded_to_working(&src[..n * 3], n)),
        Some(bytes) => lcms_to_rec2020(&src[..n * 3], n, bytes),
    }
}

fn srgb_to_rec2020_mat() -> Mat3 {
    mat_mul(&XYZ_TO_REC2020, &SRGB_TO_XYZ)
}

fn srgb_eotf(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_encoded_to_working(src: &[f32], n: usize) -> Vec<f32> {
    let m = srgb_to_rec2020_mat();
    let mut data = vec![0f32; n * 3];
    for i in 0..n {
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

fn rec2020_linear_profile() -> Result<Profile, CoreError> {
    let white = CIExyY {
        x: 0.3127,
        y: 0.3290,
        Y: 1.0,
    };
    let primaries = CIExyYTRIPLE {
        Red: CIExyY {
            x: 0.708,
            y: 0.292,
            Y: 1.0,
        },
        Green: CIExyY {
            x: 0.170,
            y: 0.797,
            Y: 1.0,
        },
        Blue: CIExyY {
            x: 0.131,
            y: 0.046,
            Y: 1.0,
        },
    };
    let linear = ToneCurve::new(1.0);
    Profile::new_rgb(&white, &primaries, &[&linear, &linear, &linear])
        .map_err(|e| CoreError::Decode(format!("Rec.2020 profile: {e:?}")))
}

fn lcms_to_rec2020(src: &[f32], n: usize, icc: &[u8]) -> Result<Vec<f32>, CoreError> {
    let src_prof =
        Profile::new_icc(icc).map_err(|e| CoreError::Decode(format!("ICC parse: {e:?}")))?;
    let dst_prof = rec2020_linear_profile()?;
    let transform = Transform::new(
        &src_prof,
        PixelFormat::RGB_FLT,
        &dst_prof,
        PixelFormat::RGB_FLT,
        Intent::RelativeColorimetric,
    )
    .map_err(|e| CoreError::Decode(format!("ICC transform: {e:?}")))?;

    let mut pixels: Vec<[f32; 3]> = (0..n)
        .map(|i| [src[i * 3], src[i * 3 + 1], src[i * 3 + 2]])
        .collect();
    transform.transform_in_place(&mut pixels);
    let mut out = vec![0f32; n * 3];
    for (i, p) in pixels.iter().enumerate() {
        out[i * 3] = p[0].max(0.0);
        out[i * 3 + 1] = p[1].max(0.0);
        out[i * 3 + 2] = p[2].max(0.0);
    }
    Ok(out)
}

fn profile_label(icc: &[u8]) -> Option<String> {
    let p = Profile::new_icc(icc).ok()?;
    let desc = p.info(InfoType::Description, Locale::none())?;
    let t = desc.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Extract an embedded ICC profile from JPEG / PNG / TIFF when present.
pub fn extract_icc(path: &Path) -> Option<Vec<u8>> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => extract_jpeg_icc(path).ok().flatten(),
        "png" => extract_png_icc(path).ok().flatten(),
        "tif" | "tiff" => extract_tiff_icc(path).ok().flatten(),
        _ => None,
    }
}

fn extract_jpeg_icc(path: &Path) -> Result<Option<Vec<u8>>, CoreError> {
    let mut f = File::open(path).map_err(|e| CoreError::Decode(e.to_string()))?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes)
        .map_err(|e| CoreError::Decode(e.to_string()))?;
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return Ok(None);
    }
    let mut i = 2usize;
    let mut chunks: Vec<(u8, Vec<u8>)> = Vec::new();
    let mut expected: Option<u8> = None;
    while i + 4 < bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = bytes[i + 1];
        if marker == 0xD9 || marker == 0xDA {
            break; // EOI / SOS
        }
        if marker == 0x00 || marker == 0xFF {
            i += 1;
            continue;
        }
        if i + 4 > bytes.len() {
            break;
        }
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if len < 2 || i + 2 + len > bytes.len() {
            break;
        }
        let payload = &bytes[i + 4..i + 2 + len];
        if marker == 0xE2 && payload.len() >= 14 && payload.starts_with(b"ICC_PROFILE\0") {
            let seq = payload[12];
            let count = payload[13];
            expected = Some(count);
            chunks.push((seq, payload[14..].to_vec()));
        }
        i += 2 + len;
    }
    if chunks.is_empty() {
        return Ok(None);
    }
    chunks.sort_by_key(|(s, _)| *s);
    if let Some(c) = expected {
        if chunks.len() != c as usize {
            tracing::warn!(
                found = chunks.len(),
                expected = c,
                "JPEG ICC chunk count mismatch"
            );
        }
    }
    let mut out = Vec::new();
    for (_, c) in chunks {
        out.extend_from_slice(&c);
    }
    Ok(if out.is_empty() { None } else { Some(out) })
}

fn extract_png_icc(path: &Path) -> Result<Option<Vec<u8>>, CoreError> {
    let f = File::open(path).map_err(|e| CoreError::Decode(e.to_string()))?;
    let decoder = png::Decoder::new(BufReader::new(f));
    let reader = decoder
        .read_info()
        .map_err(|e| CoreError::Decode(format!("png: {e}")))?;
    Ok(reader.info().icc_profile.as_ref().map(|c| c.to_vec()))
}

fn extract_tiff_icc(path: &Path) -> Result<Option<Vec<u8>>, CoreError> {
    use tiff::decoder::Decoder;
    use tiff::tags::Tag;
    let f = File::open(path).map_err(|e| CoreError::Decode(e.to_string()))?;
    let mut dec =
        Decoder::new(BufReader::new(f)).map_err(|e| CoreError::Decode(format!("tiff: {e}")))?;
    match dec.get_tag_u8_vec(Tag::IccProfile) {
        Ok(v) if !v.is_empty() => Ok(Some(v)),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::mat_vec;

    #[test]
    fn srgb_path_matches_matrix() {
        // Mid gray encoded 0.5 → linear ~0.214 → Rec.2020 gray stays gray.
        let src = vec![0.5f32, 0.5, 0.5];
        let out = encoded_rgb_to_working(&src, 1, 1, None).unwrap();
        assert!((out[0] - out[1]).abs() < 1e-4);
        assert!((out[1] - out[2]).abs() < 1e-4);
        let lin = srgb_eotf(0.5);
        let expect = mat_vec(&srgb_to_rec2020_mat(), [lin, lin, lin]);
        assert!((out[0] - expect[0]).abs() < 1e-4);
    }

    #[test]
    fn srgb_icc_round_trip_close_to_matrix() {
        let srgb = Profile::new_srgb();
        let bytes = srgb.icc().expect("srgb profile serializes");
        let src = vec![0.8f32, 0.2, 0.1];
        let via_icc = encoded_rgb_to_working(&src, 1, 1, Some(&bytes)).unwrap();
        let via_mat = encoded_rgb_to_working(&src, 1, 1, None).unwrap();
        for i in 0..3 {
            assert!(
                (via_icc[i] - via_mat[i]).abs() < 0.03,
                "ch {i}: icc={} mat={}",
                via_icc[i],
                via_mat[i]
            );
        }
    }

    #[test]
    fn extract_missing_file_is_none() {
        assert!(extract_icc(Path::new("/no/such/file.jpg")).is_none());
    }

    #[test]
    fn display_p3_red_differs_from_srgb_assumption() {
        // Build a Display P3 ICC and convert a pure red — must not match the
        // sRGB-assumed path (the silent wrong-color bug this phase fixes).
        let white = CIExyY {
            x: 0.3127,
            y: 0.3290,
            Y: 1.0,
        };
        let p3 = CIExyYTRIPLE {
            Red: CIExyY {
                x: 0.680,
                y: 0.320,
                Y: 1.0,
            },
            Green: CIExyY {
                x: 0.265,
                y: 0.690,
                Y: 1.0,
            },
            Blue: CIExyY {
                x: 0.150,
                y: 0.060,
                Y: 1.0,
            },
        };
        // P3 shares the sRGB TRC — approximate with gamma 2.2 for the test profile.
        let trc = ToneCurve::new(2.2);
        let prof = Profile::new_rgb(&white, &p3, &[&trc, &trc, &trc]).unwrap();
        let icc = prof.icc().unwrap();
        let src = vec![1.0f32, 0.0, 0.0];
        let p3_out = encoded_rgb_to_working(&src, 1, 1, Some(&icc)).unwrap();
        let srgb_out = encoded_rgb_to_working(&src, 1, 1, None).unwrap();
        let delta = (p3_out[0] - srgb_out[0]).abs()
            + (p3_out[1] - srgb_out[1]).abs()
            + (p3_out[2] - srgb_out[2]).abs();
        assert!(
            delta > 0.02,
            "P3 red should diverge from sRGB assumption; delta={delta} p3={p3_out:?} srgb={srgb_out:?}"
        );
    }
}
