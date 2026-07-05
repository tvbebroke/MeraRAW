//! Bayer/CFA pattern description. Real decode uses `rawler`
//! (`core/src/raw/rawler_decoder.rs`), which also demosaics.
#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CfaPattern {
    Rggb,
    Bggr,
    Grbg,
    Gbrg,
}

impl CfaPattern {
    /// Color index (0=R, 1=G, 2=B) at sensor position (x, y).
    pub fn color_at(&self, x: usize, y: usize) -> u8 {
        let p = (y & 1) * 2 + (x & 1);
        match self {
            CfaPattern::Rggb => [0u8, 1, 1, 2][p],
            CfaPattern::Bggr => [2u8, 1, 1, 0][p],
            CfaPattern::Grbg => [1u8, 0, 2, 1][p],
            CfaPattern::Gbrg => [1u8, 2, 0, 1][p],
        }
    }
}
