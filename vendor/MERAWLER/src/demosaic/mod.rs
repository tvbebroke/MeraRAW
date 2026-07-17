//! Demosaic algorithms and the trait they share.
//!
//! Every algorithm takes a [`CfaImage`] and returns a full-color [`RgbImage`].
//! Implementations live in submodules and are selected via [`Algorithm`].

use crate::image::{CfaImage, RgbImage};

pub mod amaze;
pub mod bilinear;
pub mod ddfapd;
pub mod igv;
pub mod lmmse;
pub mod malvar;
pub mod rcd;

/// Helpers shared by the per-algorithm unit tests: build synthetic mosaics
/// from a known continuous field and measure interior reconstruction error.
#[cfg(test)]
pub(crate) mod tests_common {
    use crate::image::{CfaImage, CfaPattern, RgbImage};

    /// Sample `f(x, y)` and pack it into a Bayer mosaic (single channel per
    /// sensel, as a real sensor would record a scene of that luminance).
    pub fn mosaic_from_fn(
        width: usize,
        height: usize,
        pattern: CfaPattern,
        f: impl Fn(usize, usize) -> f32,
    ) -> CfaImage {
        let mut data = vec![0.0f32; width * height];
        for y in 0..height {
            for x in 0..width {
                data[y * width + x] = f(x, y);
            }
        }
        CfaImage {
            width,
            height,
            data,
            pattern,
            wb: [1.0, 1.0, 1.0],
        }
    }

    /// Max absolute error between every reconstructed channel and the ground
    /// truth `f`, ignoring a `border`-pixel margin.
    pub fn max_interior_error(
        rgb: &RgbImage,
        border: usize,
        f: impl Fn(usize, usize) -> f32,
    ) -> f32 {
        let mut worst = 0.0f32;
        for y in border..rgb.height - border {
            for x in border..rgb.width - border {
                let truth = f(x, y);
                for c in rgb.at(x, y) {
                    worst = worst.max((c - truth).abs());
                }
            }
        }
        worst
    }
}

/// A demosaic (CFA interpolation) algorithm.
pub trait Demosaic {
    /// Reconstruct a full-color image from the Bayer mosaic.
    fn demosaic(&self, cfa: &CfaImage) -> RgbImage;

    /// Human-readable name, e.g. `"RCD"`.
    fn name(&self) -> &'static str;
}

/// Selectable demosaic algorithm.
///
/// `Bilinear` and `Malvar` are implemented today. The remaining variants are
/// the planned high-quality algorithms; they parse and describe themselves but
/// [`Algorithm::make`] returns `None` until they land.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algorithm {
    /// Fast separable interpolation. Baseline; visible zippering/false color.
    Bilinear,
    /// Malvar–He–Cutler gradient-corrected bilinear. Cheap quality baseline.
    Malvar,
    /// Ratio Corrected Demosaicing (Luis Sanz Rodríguez). *Planned.*
    Rcd,
    /// LMMSE demosaicing — best on noisy captures. *Planned.*
    Lmmse,
    /// AMaZE (Emil Martinec) — maximum detail. *Planned.*
    Amaze,
    /// Integrated Gradients. *Planned.*
    Igv,
    /// DDFAPD (Menon) directional demosaicing. *Planned.*
    Ddfapd,
}

impl Algorithm {
    /// Parse an algorithm by name (case-insensitive).
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "bilinear" | "bilin" => Some(Algorithm::Bilinear),
            "malvar" | "mhc" => Some(Algorithm::Malvar),
            "rcd" => Some(Algorithm::Rcd),
            "lmmse" => Some(Algorithm::Lmmse),
            "amaze" => Some(Algorithm::Amaze),
            "igv" => Some(Algorithm::Igv),
            "ddfapd" | "menon" => Some(Algorithm::Ddfapd),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Algorithm::Bilinear => "bilinear",
            Algorithm::Malvar => "malvar",
            Algorithm::Rcd => "rcd",
            Algorithm::Lmmse => "lmmse",
            Algorithm::Amaze => "amaze",
            Algorithm::Igv => "igv",
            Algorithm::Ddfapd => "ddfapd",
        }
    }

    /// Whether this algorithm is implemented yet.
    pub fn is_implemented(self) -> bool {
        matches!(
            self,
            Algorithm::Bilinear
                | Algorithm::Malvar
                | Algorithm::Rcd
                | Algorithm::Lmmse
                | Algorithm::Amaze
                | Algorithm::Igv
                | Algorithm::Ddfapd
        )
    }

    /// Construct the demosaicer, or `None` if it is not implemented yet.
    pub fn make(self) -> Option<Box<dyn Demosaic>> {
        match self {
            Algorithm::Bilinear => Some(Box::new(bilinear::Bilinear)),
            Algorithm::Malvar => Some(Box::new(malvar::Malvar)),
            Algorithm::Rcd => Some(Box::new(rcd::Rcd)),
            Algorithm::Lmmse => Some(Box::new(lmmse::Lmmse)),
            Algorithm::Amaze => Some(Box::new(amaze::Amaze)),
            Algorithm::Igv => Some(Box::new(igv::Igv)),
            Algorithm::Ddfapd => Some(Box::new(ddfapd::Ddfapd)),
        }
    }

    /// Every variant, in roadmap order — handy for CLI listings.
    pub fn all() -> &'static [Algorithm] {
        &[
            Algorithm::Bilinear,
            Algorithm::Malvar,
            Algorithm::Rcd,
            Algorithm::Lmmse,
            Algorithm::Amaze,
            Algorithm::Igv,
            Algorithm::Ddfapd,
        ]
    }
}

/// Convenience: demosaic `cfa` with `algo`, or `None` if `algo` is unimplemented.
pub fn demosaic(cfa: &CfaImage, algo: Algorithm) -> Option<RgbImage> {
    algo.make().map(|d| d.demosaic(cfa))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_parse_roundtrip() {
        for &a in Algorithm::all() {
            assert_eq!(Algorithm::from_name(a.name()), Some(a));
        }
    }

    #[test]
    fn implemented_set_matches_make() {
        for &a in Algorithm::all() {
            assert_eq!(a.is_implemented(), a.make().is_some(), "{a:?}");
        }
        assert!(Algorithm::Rcd.is_implemented());
        assert!(Algorithm::Amaze.is_implemented());
        assert!(Algorithm::Igv.is_implemented());
        // every roadmap algorithm is now implemented
        assert!(Algorithm::all().iter().all(|a| a.is_implemented()));
    }
}
