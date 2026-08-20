//! Bridge from a decoded `rawler::RawImage` to our [`CfaImage`].
//!
//! rawler handles container parsing and unpacking; here we do only what a
//! demosaic engine needs: pull the raw Bayer plane, subtract the per-position
//! black level, normalize by the white level, and detect the CFA pattern.
//! We deliberately do *not* run rawler's own demosaic/white-balance/sRGB steps.
//!
//! Only available with the `decode` feature.

use crate::image::{CfaImage, CfaPattern};
use rawler::rawimage::RawImageData;
use rawler::RawImage;
use std::path::Path;

/// Errors converting a decoded RAW into a mosaic we can demosaic.
#[derive(Debug)]
pub enum AdaptError {
    /// Image is already demosaiced (e.g. Apple ProRAW linear DNG): `cpp != 1`.
    NotMosaiced { cpp: usize },
    /// Floating-point raw planes aren't handled yet.
    FloatData,
    /// CFA layout isn't one of the four 2x2 Bayer patterns (e.g. X-Trans).
    UnsupportedPattern,
    /// rawler failed to open or decode the file.
    Decode(String),
}

impl std::fmt::Display for AdaptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdaptError::NotMosaiced { cpp } => {
                write!(
                    f,
                    "image is already demosaiced (cpp={cpp}); no CFA to interpolate"
                )
            }
            AdaptError::FloatData => write!(f, "floating-point raw data is not supported yet"),
            AdaptError::UnsupportedPattern => {
                write!(
                    f,
                    "CFA is not a 2x2 Bayer pattern (X-Trans is out of scope)"
                )
            }
            AdaptError::Decode(e) => write!(f, "decode failed: {e}"),
        }
    }
}

impl std::error::Error for AdaptError {}

/// Decode `path` with rawler and produce a normalized [`CfaImage`].
pub fn cfa_from_path(path: &Path) -> Result<CfaImage, AdaptError> {
    use rawler::decoders::RawDecodeParams;
    use rawler::rawsource::RawSource;
    use rawler::RawLoader;

    let loader = RawLoader::new();
    let source = RawSource::new(path).map_err(|e| AdaptError::Decode(e.to_string()))?;
    let decoder = loader
        .get_decoder(&source)
        .map_err(|e| AdaptError::Decode(e.to_string()))?;
    let params = RawDecodeParams::default();
    let raw = decoder
        .raw_image(&source, &params, false)
        .map_err(|e| AdaptError::Decode(e.to_string()))?;
    cfa_from_rawimage(&raw)
}

/// Convert an already-decoded [`RawImage`] into a normalized [`CfaImage`].
pub fn cfa_from_rawimage(raw: &RawImage) -> Result<CfaImage, AdaptError> {
    if raw.cpp != 1 {
        return Err(AdaptError::NotMosaiced { cpp: raw.cpp });
    }
    let samples = match &raw.data {
        RawImageData::Integer(v) => v,
        RawImageData::Float(_) => return Err(AdaptError::FloatData),
    };

    let (w, h) = (raw.width, raw.height);
    let cfa = &raw.camera.cfa;
    let pattern = detect_pattern(cfa).ok_or(AdaptError::UnsupportedPattern)?;

    // Black level: a repeating `bw x bh` tile of Rationals (usually 1x1 or 2x2).
    let bl = &raw.blacklevel;
    let black: Vec<f32> = bl.levels.iter().map(|r| r.as_f32()).collect();
    let (bw, bh) = (bl.width.max(1), bl.height.max(1));

    // White level: one value, or a per-Bayer-position [f32; 4].
    let white_arr = raw.whitelevel.as_bayer_array();
    let white_uniform = raw.whitelevel.0.len() < 4;
    let white0 = white_arr[0];

    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let bpos = if black.len() >= bw * bh {
                black[(y % bh) * bw + (x % bw)]
            } else {
                black.first().copied().unwrap_or(0.0)
            };
            let white = if white_uniform {
                white0
            } else {
                white_arr[(y & 1) * 2 + (x & 1)]
            };
            let range = (white - bpos).max(1.0);
            let v = samples[y * w + x] as f32;
            // Preserve highlight headroom above 1.0; only floor at black.
            data[y * w + x] = ((v - bpos) / range).max(0.0);
        }
    }

    // Green-normalized white balance from the as-shot coefficients.
    let c = raw.wb_coeffs;
    let g = if c[1] > 0.0 { c[1] } else { 1.0 };
    let wb = if c[0].is_finite() && c[2].is_finite() && c[0] > 0.0 {
        [c[0] / g, 1.0, c[2] / g]
    } else {
        [1.0, 1.0, 1.0]
    };

    Ok(CfaImage {
        width: w,
        height: h,
        data,
        pattern,
        wb,
    })
}

/// Read the top-left 2x2 tile of a rawler `CFA` and map it to our pattern.
/// rawler's `color_at(row, col)` returns 0=R, 1=G, 2=B (3 = green-2).
fn detect_pattern(cfa: &rawler::cfa::CFA) -> Option<CfaPattern> {
    let c = |row: usize, col: usize| cfa.color_at(row, col) as u8;
    CfaPattern::from_tile(c(0, 0), c(0, 1), c(1, 0), c(1, 1))
}
