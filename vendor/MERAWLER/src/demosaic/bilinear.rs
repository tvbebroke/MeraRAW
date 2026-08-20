//! Bilinear demosaicing — the simplest real interpolator, used as a baseline
//! and as the edge fallback for kernel-based algorithms.
//!
//! For each pixel we keep the measured channel and fill the two missing
//! channels by averaging the same-color samples in the 3x3 neighborhood.
//! Out-of-bounds neighbors are skipped rather than clamped, so a clamped
//! coordinate can never contribute the wrong CFA color.

use super::Demosaic;
use crate::image::{CfaImage, RgbImage};

pub struct Bilinear;

impl Demosaic for Bilinear {
    fn demosaic(&self, cfa: &CfaImage) -> RgbImage {
        let (w, h) = (cfa.width, cfa.height);
        let mut out = RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                out.set(x, y, demosaic_pixel(cfa, x, y));
            }
        }
        out
    }

    fn name(&self) -> &'static str {
        "bilinear"
    }
}

/// Bilinear reconstruction for a single pixel. Shared with kernel-based
/// algorithms that fall back to bilinear near the image border.
#[inline]
pub(crate) fn demosaic_pixel(cfa: &CfaImage, x: usize, y: usize) -> [f32; 3] {
    let own = cfa.color_at(x, y) as usize;
    let mut px = [0.0f32; 3];
    for color in 0..3u8 {
        px[color as usize] = if color as usize == own {
            cfa.at(x, y)
        } else {
            avg_color_neighbors(cfa, x, y, color)
        };
    }
    px
}

/// Average of the `color`-colored sensels in the 3x3 neighborhood of `(x, y)`,
/// skipping out-of-bounds positions.
#[inline]
fn avg_color_neighbors(cfa: &CfaImage, x: usize, y: usize, color: u8) -> f32 {
    let (w, h) = (cfa.width as isize, cfa.height as isize);
    let (xi, yi) = (x as isize, y as isize);
    let mut sum = 0.0f32;
    let mut n = 0u32;
    for dy in -1..=1isize {
        for dx in -1..=1isize {
            let (nx, ny) = (xi + dx, yi + dy);
            if nx < 0 || ny < 0 || nx >= w || ny >= h {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if cfa.color_at(nx, ny) == color {
                sum += cfa.at(nx, ny);
                n += 1;
            }
        }
    }
    if n > 0 {
        sum / n as f32
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
        // A flat gray field: every channel equals the same constant, so a
        // correct demosaicer must return that constant everywhere.
        let cfa = mosaic_from_fn(16, 12, CfaPattern::Rggb, |_, _| 0.42);
        let rgb = Bilinear.demosaic(&cfa);
        for px in &rgb.data {
            for c in px {
                assert!((c - 0.42).abs() < 1e-6, "got {c}");
            }
        }
    }

    #[test]
    fn linear_gradient_reconstructs_in_interior() {
        // Bilinear is exact for a linear ramp in the interior.
        let grad = |x: usize, _y: usize| 0.1 + 0.02 * x as f32;
        let cfa = mosaic_from_fn(32, 24, CfaPattern::Rggb, grad);
        let rgb = Bilinear.demosaic(&cfa);
        let err = max_interior_error(&rgb, 1, grad);
        assert!(err < 1e-4, "max interior error {err}");
    }
}
