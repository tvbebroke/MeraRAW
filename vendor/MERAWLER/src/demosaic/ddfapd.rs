//! DDFAPD — Menon (2007) "Demosaicing With Directional Filtering and a
//! posteriori Decision".
//!
//! Green is first interpolated in *both* directions (horizontal and vertical)
//! with a Hamilton-Adams filter, giving two candidate color-difference planes.
//! A local classifier then decides, per pixel, which direction reconstructs
//! more smoothly (the "a posteriori decision", stored in the mask `M`) by
//! comparing accumulated color-difference gradients. Green is chosen from the
//! winning direction; red and blue are then filled from the color differences,
//! and an optional refining step cleans up the chroma.
//!
//! This is an original Rust implementation written from the algorithm as given
//! by the BSD-3-Clause `colour-demosaicing` reference (Menon 2007); constants
//! and filter taps are mathematical facts. Runs whole-image with a
//! bilinear-seeded border.

use crate::image::{CfaImage, RgbImage};
use super::bilinear::Bilinear;
use super::Demosaic;

pub struct Ddfapd;

/// Outer border filled by the bilinear seed (covers every stage's reach).
const BORDER: usize = 8;

impl Demosaic for Ddfapd {
    fn demosaic(&self, img: &CfaImage) -> RgbImage {
        let (w, h) = (img.width, img.height);
        if w < 2 * BORDER + 4 || h < 2 * BORDER + 4 {
            return Bilinear.demosaic(img);
        }
        let mut out = Bilinear.demosaic(img);
        let cfa = &img.data[..];
        let pattern = img.pattern;
        let n = w * h;

        let fc = |x: usize, y: usize| pattern.color_at(x, y);
        // Clamped-edge sample of a plane.
        let g = |a: &[f32], x: isize, y: isize| -> f32 {
            let cx = x.clamp(0, w as isize - 1) as usize;
            let cy = y.clamp(0, h as isize - 1) as usize;
            a[cy * w + cx]
        };

        // Row/column color membership (which lines carry red / blue).
        let red_row = |y: usize| fc(0, y) == 0 || fc(1, y) == 0;
        let blue_row = |y: usize| fc(0, y) == 2 || fc(1, y) == 2;
        let red_col = |x: usize| fc(x, 0) == 0 || fc(x, 1) == 0;
        let blue_col = |x: usize| fc(x, 0) == 2 || fc(x, 1) == 2;

        // --- directional green (Hamilton-Adams) + color differences ---
        // At R/B sites: G_H = 0.5*(g[-1]+g[+1]) - 0.25*(c[-2]+c[+2]) + 0.5*c.
        let mut gh = vec![0.0f32; n];
        let mut gv = vec![0.0f32; n];
        let mut ch = vec![0.0f32; n]; // horizontal color difference (C - G_H)
        let mut cv = vec![0.0f32; n];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let (xi, yi) = (x as isize, y as isize);
                if fc(x, y) == 1 {
                    gh[i] = cfa[i];
                    gv[i] = cfa[i];
                    // ch/cv stay 0 at green sites
                } else {
                    let hh = 0.5 * (g(cfa, xi - 1, yi) + g(cfa, xi + 1, yi))
                        - 0.25 * (g(cfa, xi - 2, yi) + g(cfa, xi + 2, yi))
                        + 0.5 * cfa[i];
                    let vv = 0.5 * (g(cfa, xi, yi - 1) + g(cfa, xi, yi + 1))
                        - 0.25 * (g(cfa, xi, yi - 2) + g(cfa, xi, yi + 2))
                        + 0.5 * cfa[i];
                    gh[i] = hh;
                    gv[i] = vv;
                    ch[i] = cfa[i] - hh;
                    cv[i] = cfa[i] - vv;
                }
            }
        }

        // Color-difference gradients (magnitude of the +2 difference).
        let mut dh = vec![0.0f32; n];
        let mut dv = vec![0.0f32; n];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                dh[i] = (ch[i] - g(&ch, x as isize + 2, y as isize)).abs();
                dv[i] = (cv[i] - g(&cv, x as isize, y as isize + 2)).abs();
            }
        }

        // Accumulate directional evidence with the Menon 5x5 kernel (the exact
        // flipped offsets of scipy's `convolve`, and its transpose for vertical).
        // Decide per pixel: mask = (vertical grad >= horizontal grad).
        let mut mask = vec![false; n];
        let mut green = vec![0.0f32; n];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let (xi, yi) = (x as isize, y as isize);
                let d_h = 3.0 * g(&dh, xi, yi)
                    + 3.0 * g(&dh, xi - 2, yi)
                    + g(&dh, xi, yi + 2)
                    + g(&dh, xi - 2, yi + 2)
                    + g(&dh, xi - 1, yi + 1)
                    + g(&dh, xi - 1, yi - 1)
                    + g(&dh, xi, yi - 2)
                    + g(&dh, xi - 2, yi - 2);
                let d_v = 3.0 * g(&dv, xi, yi)
                    + 3.0 * g(&dv, xi, yi - 2)
                    + g(&dv, xi + 2, yi)
                    + g(&dv, xi + 2, yi - 2)
                    + g(&dv, xi + 1, yi - 1)
                    + g(&dv, xi - 1, yi - 1)
                    + g(&dv, xi - 2, yi)
                    + g(&dv, xi - 2, yi - 2);
                let m = d_v >= d_h;
                mask[i] = m;
                green[i] = if fc(x, y) == 1 {
                    cfa[i]
                } else if m {
                    gh[i]
                } else {
                    gv[i]
                };
            }
        }
        drop((gh, gv, ch, cv, dh, dv));

        // Red/blue planes: measured at their own sensels, 0 elsewhere (yet).
        let mut r = vec![0.0f32; n];
        let mut b = vec![0.0f32; n];
        for i in 0..n {
            match pattern.color_at(i % w, i / w) {
                0 => r[i] = cfa[i],
                2 => b[i] = cfa[i],
                _ => {}
            }
        }

        let kb_h = |a: &[f32], x: isize, y: isize| 0.5 * (g(a, x - 1, y) + g(a, x + 1, y));
        let kb_v = |a: &[f32], x: isize, y: isize| 0.5 * (g(a, x, y - 1) + g(a, x, y + 1));

        // --- R/B at green sensels (color-difference interpolation) ---
        for y in 0..h {
            for x in 0..w {
                if fc(x, y) != 1 {
                    continue;
                }
                let i = y * w + x;
                let (xi, yi) = (x as isize, y as isize);
                // red: horizontal in red rows, vertical in blue rows
                if red_row(y) {
                    r[i] = green[i] + kb_h(&r, xi, yi) - kb_h(&green, xi, yi);
                } else if blue_row(y) {
                    r[i] = green[i] + kb_v(&r, xi, yi) - kb_v(&green, xi, yi);
                }
                // blue: horizontal in blue rows, vertical in red rows
                if blue_row(y) {
                    b[i] = green[i] + kb_h(&b, xi, yi) - kb_h(&green, xi, yi);
                } else if red_row(y) {
                    b[i] = green[i] + kb_v(&b, xi, yi) - kb_v(&green, xi, yi);
                }
            }
        }

        // --- R at blue sensels and B at red sensels (direction from mask) ---
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let (xi, yi) = (x as isize, y as isize);
                match fc(x, y) {
                    2 => {
                        // R at blue
                        r[i] = if mask[i] {
                            b[i] + kb_h(&r, xi, yi) - kb_h(&b, xi, yi)
                        } else {
                            b[i] + kb_v(&r, xi, yi) - kb_v(&b, xi, yi)
                        };
                    }
                    0 => {
                        // B at red
                        b[i] = if mask[i] {
                            r[i] + kb_h(&b, xi, yi) - kb_h(&r, xi, yi)
                        } else {
                            r[i] + kb_v(&b, xi, yi) - kb_v(&r, xi, yi)
                        };
                    }
                    _ => {}
                }
            }
        }

        // --- refining step ---
        refine(
            &mut r, &mut b, &mut green, &mask, cfa, w, h, &fc, &g, &red_row, &blue_row, &red_col,
            &blue_col,
        );

        // --- output interior; border stays bilinear ---
        for y in BORDER..h - BORDER {
            for x in BORDER..w - BORDER {
                let i = y * w + x;
                out.data[i] = [r[i].max(0.0), green[i].max(0.0), b[i].max(0.0)];
            }
        }
        out
    }

    fn name(&self) -> &'static str {
        "ddfapd"
    }
}

/// Menon's refining step: re-estimate green from a smoothed color difference,
/// then re-fill red/blue at green sensels and at the opposite-color sensels.
#[allow(clippy::too_many_arguments)]
fn refine(
    r: &mut [f32],
    b: &mut [f32],
    green: &mut [f32],
    mask: &[bool],
    _cfa: &[f32],
    w: usize,
    h: usize,
    fc: &impl Fn(usize, usize) -> u8,
    g: &impl Fn(&[f32], isize, isize) -> f32,
    red_row: &impl Fn(usize) -> bool,
    blue_row: &impl Fn(usize) -> bool,
    red_col: &impl Fn(usize) -> bool,
    blue_col: &impl Fn(usize) -> bool,
) {
    let n = w * h;
    let fir_h = |a: &[f32], x: isize, y: isize| (g(a, x - 1, y) + g(a, x, y) + g(a, x + 1, y)) / 3.0;
    let fir_v = |a: &[f32], x: isize, y: isize| (g(a, x, y - 1) + g(a, x, y) + g(a, x, y + 1)) / 3.0;
    let kb_h = |a: &[f32], x: isize, y: isize| 0.5 * (g(a, x - 1, y) + g(a, x + 1, y));
    let kb_v = |a: &[f32], x: isize, y: isize| 0.5 * (g(a, x, y - 1) + g(a, x, y + 1));

    // Update green from a 3-tap smoothed color difference (direction from mask).
    let rg: Vec<f32> = (0..n).map(|i| r[i] - green[i]).collect();
    let bg: Vec<f32> = (0..n).map(|i| b[i] - green[i]).collect();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let (xi, yi) = (x as isize, y as isize);
            match fc(x, y) {
                0 => {
                    let m = if mask[i] { fir_h(&rg, xi, yi) } else { fir_v(&rg, xi, yi) };
                    green[i] = r[i] - m;
                }
                2 => {
                    let m = if mask[i] { fir_h(&bg, xi, yi) } else { fir_v(&bg, xi, yi) };
                    green[i] = b[i] - m;
                }
                _ => {}
            }
        }
    }

    // Update red/blue at green sensels from the refreshed color differences.
    let rg: Vec<f32> = (0..n).map(|i| r[i] - green[i]).collect();
    let bg: Vec<f32> = (0..n).map(|i| b[i] - green[i]).collect();
    for y in 0..h {
        for x in 0..w {
            if fc(x, y) != 1 {
                continue;
            }
            let i = y * w + x;
            let (xi, yi) = (x as isize, y as isize);
            if blue_row(y) {
                r[i] = green[i] + kb_v(&rg, xi, yi);
            } else if blue_col(x) {
                r[i] = green[i] + kb_h(&rg, xi, yi);
            }
            if red_row(y) {
                b[i] = green[i] + kb_v(&bg, xi, yi);
            } else if red_col(x) {
                b[i] = green[i] + kb_h(&bg, xi, yi);
            }
        }
    }

    // Update R at blue and B at red from the R-B difference.
    let rb: Vec<f32> = (0..n).map(|i| r[i] - b[i]).collect();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let (xi, yi) = (x as isize, y as isize);
            match fc(x, y) {
                2 => {
                    let m = if mask[i] { fir_h(&rb, xi, yi) } else { fir_v(&rb, xi, yi) };
                    r[i] = b[i] + m;
                }
                0 => {
                    let m = if mask[i] { fir_h(&rb, xi, yi) } else { fir_v(&rb, xi, yi) };
                    b[i] = r[i] - m;
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::CfaPattern;
    use crate::demosaic::tests_common::{max_interior_error, mosaic_from_fn};

    #[test]
    fn constant_field_reconstructs() {
        let cfa = mosaic_from_fn(48, 40, CfaPattern::Rggb, |_, _| 0.5);
        let rgb = Ddfapd.demosaic(&cfa);
        for px in &rgb.data {
            for c in px {
                assert!((c - 0.5).abs() < 2e-3, "got {c}");
            }
        }
    }

    #[test]
    fn linear_gradient_low_error() {
        let grad = |x: usize, y: usize| 0.2 + 0.003 * x as f32 + 0.002 * y as f32;
        let cfa = mosaic_from_fn(64, 56, CfaPattern::Rggb, grad);
        let rgb = Ddfapd.demosaic(&cfa);
        let err = max_interior_error(&rgb, BORDER, grad);
        assert!(err < 2e-2, "max interior error {err}");
    }

    #[test]
    fn all_patterns_reconstruct_constant() {
        for p in [
            CfaPattern::Rggb,
            CfaPattern::Bggr,
            CfaPattern::Grbg,
            CfaPattern::Gbrg,
        ] {
            let cfa = mosaic_from_fn(40, 40, p, |_, _| 0.4);
            let rgb = Ddfapd.demosaic(&cfa);
            for px in &rgb.data {
                for c in px {
                    assert!((c - 0.4).abs() < 2e-3, "pattern {:?} got {c}", p);
                }
            }
        }
    }
}
