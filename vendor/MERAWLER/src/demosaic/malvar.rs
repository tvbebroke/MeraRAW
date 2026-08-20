//! Malvar–He–Cutler (2004) gradient-corrected bilinear demosaicing.
//!
//! "High-quality linear interpolation for demosaicing of Bayer-patterned color
//! images", ICASSP 2004. Each missing channel is a bilinear estimate plus a
//! correction proportional to the Laplacian of a *measured* channel at that
//! site, implemented as five fixed 5x5 kernels (all coefficients over 8).
//!
//! Pixels within 2px of the border fall back to [`bilinear`](super::bilinear)
//! since the 5x5 support would read out of bounds.

use super::bilinear::demosaic_pixel as bilinear_pixel;
use super::Demosaic;
use crate::image::{CfaImage, RgbImage};

pub struct Malvar;

impl Demosaic for Malvar {
    fn demosaic(&self, cfa: &CfaImage) -> RgbImage {
        let (w, h) = (cfa.width, cfa.height);
        let mut out = RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let px = if x < 2 || y < 2 || x + 2 >= w || y + 2 >= h {
                    bilinear_pixel(cfa, x, y)
                } else {
                    malvar_pixel(cfa, x, y)
                };
                out.set(x, y, px);
            }
        }
        out
    }

    fn name(&self) -> &'static str {
        "malvar"
    }
}

// --- The five Malvar kernels (already divided by 8) ------------------------

/// Green at a red or blue sensel.
#[rustfmt::skip]
const G_AT_RB: [[f32; 5]; 5] = [
    [ 0.0,  0.0, -1.0/8.0,  0.0,  0.0],
    [ 0.0,  0.0,  2.0/8.0,  0.0,  0.0],
    [-1.0/8.0, 2.0/8.0, 4.0/8.0, 2.0/8.0, -1.0/8.0],
    [ 0.0,  0.0,  2.0/8.0,  0.0,  0.0],
    [ 0.0,  0.0, -1.0/8.0,  0.0,  0.0],
];

/// Red/blue at a green sensel whose same-color neighbors lie in the same ROW
/// (horizontal). Transpose gives the vertical case [`RB_AT_G_V`].
#[rustfmt::skip]
const RB_AT_G_H: [[f32; 5]; 5] = [
    [ 0.0,     0.0,  0.5/8.0,  0.0,     0.0    ],
    [ 0.0,    -1.0/8.0, 0.0, -1.0/8.0,  0.0    ],
    [-1.0/8.0, 4.0/8.0, 5.0/8.0, 4.0/8.0, -1.0/8.0],
    [ 0.0,    -1.0/8.0, 0.0, -1.0/8.0,  0.0    ],
    [ 0.0,     0.0,  0.5/8.0,  0.0,     0.0    ],
];

/// Red/blue at a green sensel whose same-color neighbors lie in the same COLUMN
/// (vertical) — the transpose of [`RB_AT_G_H`].
#[rustfmt::skip]
const RB_AT_G_V: [[f32; 5]; 5] = [
    [ 0.0,      0.0,    -1.0/8.0,  0.0,     0.0   ],
    [ 0.0,     -1.0/8.0, 4.0/8.0, -1.0/8.0, 0.0   ],
    [ 0.5/8.0,  0.0,     5.0/8.0,  0.0,     0.5/8.0],
    [ 0.0,     -1.0/8.0, 4.0/8.0, -1.0/8.0, 0.0   ],
    [ 0.0,      0.0,    -1.0/8.0,  0.0,     0.0   ],
];

/// Red at a blue sensel, or blue at a red sensel (diagonal neighbors).
#[rustfmt::skip]
const RB_AT_BR: [[f32; 5]; 5] = [
    [ 0.0,      0.0,    -1.5/8.0,  0.0,     0.0   ],
    [ 0.0,      2.0/8.0, 0.0,      2.0/8.0, 0.0   ],
    [-1.5/8.0,  0.0,     6.0/8.0,  0.0,    -1.5/8.0],
    [ 0.0,      2.0/8.0, 0.0,      2.0/8.0, 0.0   ],
    [ 0.0,      0.0,    -1.5/8.0,  0.0,     0.0   ],
];

/// Malvar reconstruction of one interior pixel (2px border guaranteed by caller).
#[inline]
fn malvar_pixel(cfa: &CfaImage, x: usize, y: usize) -> [f32; 3] {
    let v = cfa.at(x, y);
    match cfa.color_at(x, y) {
        0 => {
            // Red sensel: measured R; estimate G and B.
            let g = conv5(cfa, x, y, &G_AT_RB);
            let b = conv5(cfa, x, y, &RB_AT_BR);
            [v, clamp0(g), clamp0(b)]
        }
        2 => {
            // Blue sensel: measured B; estimate G and R.
            let g = conv5(cfa, x, y, &G_AT_RB);
            let r = conv5(cfa, x, y, &RB_AT_BR);
            [clamp0(r), clamp0(g), v]
        }
        _ => {
            // Green sensel: one of R/B is a horizontal neighbor, the other
            // vertical. Pick the kernel per channel from the horizontal color.
            let red_is_horizontal = cfa.color_at(x + 1, y) == 0;
            let (r, b) = if red_is_horizontal {
                (conv5(cfa, x, y, &RB_AT_G_H), conv5(cfa, x, y, &RB_AT_G_V))
            } else {
                (conv5(cfa, x, y, &RB_AT_G_V), conv5(cfa, x, y, &RB_AT_G_H))
            };
            [clamp0(r), v, clamp0(b)]
        }
    }
}

/// 5x5 convolution of the raw mosaic centered at `(x, y)`. Caller guarantees
/// the 2px border, so no bounds handling is needed.
#[inline]
fn conv5(cfa: &CfaImage, x: usize, y: usize, k: &[[f32; 5]; 5]) -> f32 {
    let mut acc = 0.0f32;
    for (ky, row) in k.iter().enumerate() {
        let sy = y + ky - 2;
        for (kx, &w) in row.iter().enumerate() {
            if w != 0.0 {
                acc += w * cfa.at(x + kx - 2, sy);
            }
        }
    }
    acc
}

/// Clamp negative estimates (out-of-gamut noise) to zero while preserving
/// highlight headroom above 1.
#[inline]
fn clamp0(v: f32) -> f32 {
    if v > 0.0 {
        v
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demosaic::tests_common::{max_interior_error, mosaic_from_fn};
    use crate::image::CfaPattern;

    #[test]
    fn constant_field_reconstructs_exactly() {
        let cfa = mosaic_from_fn(16, 12, CfaPattern::Rggb, |_, _| 0.6);
        let rgb = Malvar.demosaic(&cfa);
        for px in &rgb.data {
            for c in px {
                assert!((c - 0.6).abs() < 1e-5, "got {c}");
            }
        }
    }

    #[test]
    fn linear_gradient_low_error_in_interior() {
        // On a linear ramp the Laplacian correction is ~0, so Malvar matches
        // the exact value closely across the interior.
        let grad = |x: usize, y: usize| 0.05 + 0.015 * x as f32 + 0.01 * y as f32;
        let cfa = mosaic_from_fn(40, 32, CfaPattern::Rggb, grad);
        let rgb = Malvar.demosaic(&cfa);
        let err = max_interior_error(&rgb, 2, grad);
        assert!(err < 1e-3, "max interior error {err}");
    }

    #[test]
    fn works_for_all_patterns() {
        for p in [
            CfaPattern::Rggb,
            CfaPattern::Bggr,
            CfaPattern::Grbg,
            CfaPattern::Gbrg,
        ] {
            let cfa = mosaic_from_fn(24, 24, p, |_, _| 0.3);
            let rgb = Malvar.demosaic(&cfa);
            for px in &rgb.data {
                for c in px {
                    assert!((c - 0.3).abs() < 1e-5, "pattern {:?} got {c}", p);
                }
            }
        }
    }
}
