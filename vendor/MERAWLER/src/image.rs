//! Shared value types: the Bayer mosaic input and the RGB output.
//!
//! Color indices are the usual convention throughout the crate:
//! `0 = Red`, `1 = Green`, `2 = Blue`.

/// The four Bayer CFA layouts, named by the 2x2 tile read left-to-right,
/// top-to-bottom (e.g. [`CfaPattern::Rggb`] = R,G / G,B).
///
/// MERAWLER currently targets classic 2x2 Bayer sensors. X-Trans (6x6) needs a
/// different family of algorithms (Markesteijn) and is intentionally out of
/// scope here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CfaPattern {
    Rggb,
    Bggr,
    Grbg,
    Gbrg,
}

impl CfaPattern {
    /// The 2x2 color tile as `[top-left, top-right, bottom-left, bottom-right]`.
    #[inline]
    fn tile(self) -> [u8; 4] {
        match self {
            CfaPattern::Rggb => [0, 1, 1, 2],
            CfaPattern::Bggr => [2, 1, 1, 0],
            CfaPattern::Grbg => [1, 0, 2, 1],
            CfaPattern::Gbrg => [1, 2, 0, 1],
        }
    }

    /// Color index (0=R, 1=G, 2=B) of the sensel at absolute pixel `(x, y)`.
    #[inline]
    pub fn color_at(self, x: usize, y: usize) -> u8 {
        self.tile()[(y & 1) * 2 + (x & 1)]
    }

    /// Build a pattern from the four color indices of the top-left 2x2 tile.
    /// Green-2 (index 3, used by some 4-color decoders) is folded into green.
    pub fn from_tile(tl: u8, tr: u8, bl: u8, br: u8) -> Option<Self> {
        let g = |c: u8| if c == 3 { 1 } else { c };
        match [g(tl), g(tr), g(bl), g(br)] {
            [0, 1, 1, 2] => Some(CfaPattern::Rggb),
            [2, 1, 1, 0] => Some(CfaPattern::Bggr),
            [1, 0, 2, 1] => Some(CfaPattern::Grbg),
            [1, 2, 0, 1] => Some(CfaPattern::Gbrg),
            _ => None,
        }
    }

    /// Parse a pattern name such as `"RGGB"` (case-insensitive).
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "RGGB" => Some(CfaPattern::Rggb),
            "BGGR" => Some(CfaPattern::Bggr),
            "GRBG" => Some(CfaPattern::Grbg),
            "GBRG" => Some(CfaPattern::Gbrg),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            CfaPattern::Rggb => "RGGB",
            CfaPattern::Bggr => "BGGR",
            CfaPattern::Grbg => "GRBG",
            CfaPattern::Gbrg => "GBRG",
        }
    }
}

/// A normalized single-channel Bayer mosaic: one float per sensel, row-major.
///
/// `data` is normalized to roughly `[0, 1]` (black level maps to 0, white level
/// to 1) but values may exceed 1 where the decoder preserved highlight
/// headroom; algorithms should not assume a hard ceiling.
#[derive(Clone, Debug)]
pub struct CfaImage {
    pub width: usize,
    pub height: usize,
    /// `width * height` samples, row-major.
    pub data: Vec<f32>,
    pub pattern: CfaPattern,
    /// As-shot white balance multipliers, green-normalized `[r, 1, b]`.
    pub wb: [f32; 3],
}

impl CfaImage {
    /// Sensel value at `(x, y)`. Debug-checked bounds.
    #[inline]
    pub fn at(&self, x: usize, y: usize) -> f32 {
        debug_assert!(x < self.width && y < self.height);
        self.data[y * self.width + x]
    }

    /// Color index (0=R, 1=G, 2=B) at `(x, y)`.
    #[inline]
    pub fn color_at(&self, x: usize, y: usize) -> u8 {
        self.pattern.color_at(x, y)
    }
}

/// A 3-channel linear RGB image: `[r, g, b]` per pixel, row-major.
#[derive(Clone, Debug)]
pub struct RgbImage {
    pub width: usize,
    pub height: usize,
    /// `width * height` pixels, row-major.
    pub data: Vec<[f32; 3]>,
}

impl RgbImage {
    pub fn new(width: usize, height: usize) -> Self {
        RgbImage {
            width,
            height,
            data: vec![[0.0; 3]; width * height],
        }
    }

    #[inline]
    pub fn at(&self, x: usize, y: usize) -> [f32; 3] {
        self.data[y * self.width + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, px: [f32; 3]) {
        self.data[y * self.width + x] = px;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rggb_color_layout() {
        let p = CfaPattern::Rggb;
        assert_eq!(p.color_at(0, 0), 0); // R
        assert_eq!(p.color_at(1, 0), 1); // G
        assert_eq!(p.color_at(0, 1), 1); // G
        assert_eq!(p.color_at(1, 1), 2); // B
        // pattern is periodic
        assert_eq!(p.color_at(2, 2), 0);
        assert_eq!(p.color_at(3, 3), 2);
    }

    #[test]
    fn pattern_name_roundtrip() {
        for p in [
            CfaPattern::Rggb,
            CfaPattern::Bggr,
            CfaPattern::Grbg,
            CfaPattern::Gbrg,
        ] {
            assert_eq!(CfaPattern::from_name(p.name()), Some(p));
        }
    }

    #[test]
    fn from_tile_detects_and_folds_green2() {
        assert_eq!(CfaPattern::from_tile(0, 1, 1, 2), Some(CfaPattern::Rggb));
        // green-2 (index 3) folds into green
        assert_eq!(CfaPattern::from_tile(0, 1, 3, 2), Some(CfaPattern::Rggb));
        assert_eq!(CfaPattern::from_tile(2, 3, 1, 0), Some(CfaPattern::Bggr));
    }
}
