//! RCD — Ratio Corrected Demosaicing (algorithm by Luis Sanz Rodríguez).
//!
//! RCD is darktable's default: excellent color, few artifacts, and cheaper than
//! AMaZE. It rests on three ideas:
//!
//! 1. **Directional discrimination.** A squared high-pass response measures how
//!    much detail runs vertically vs. horizontally (`vh_dir`) and along the two
//!    diagonals (`pq_dir`). These steer every later interpolation.
//! 2. **Ratio-corrected green.** At red/blue sensels green is interpolated from
//!    the four cardinal neighbors, but each neighbor is scaled by the ratio of a
//!    smooth low-pass luma (`lpf`) at the two sites, then blended by `vh_dir`.
//!    Interpolating a *ratio* instead of a raw value suppresses the color
//!    fringing bilinear/Malvar produce on edges.
//! 3. **Color-difference red/blue.** Red and blue are filled by interpolating
//!    the `color - green` difference: first at the opposite-color sensels along
//!    the diagonals (steered by `pq_dir`), then at green sensels along the
//!    cardinals (steered by `vh_dir`).
//!
//! This is an original Rust implementation written from the published algorithm
//! (the constants and neighbor offsets are mathematical facts); it is not a port
//! of the GPL reference code. It runs over the whole image with a small border;
//! the outer [`RCD_BORDER`] frame falls back to bilinear. (A tiled variant to cap
//! peak memory is a future optimization.)

use super::bilinear::Bilinear;
use super::Demosaic;
use crate::image::{CfaImage, RgbImage};

pub struct Rcd;

const EPS: f32 = 1e-5;
const EPSSQ: f32 = 1e-10;
/// Pixels closer than this to the edge fall back to bilinear (largest neighbor
/// reach is 4 for the green gradients plus 1 for the direction neighborhood).
const RCD_BORDER: usize = 6;

#[inline(always)]
fn sq(x: f32) -> f32 {
    x * x
}

impl Demosaic for Rcd {
    fn demosaic(&self, img: &CfaImage) -> RgbImage {
        let (w, h) = (img.width, img.height);
        let pattern = img.pattern;

        // Seed every channel with bilinear so borders and not-yet-filled
        // neighbors always hold a sane value, then overwrite the interior.
        let base = Bilinear.demosaic(img);
        if w <= 2 * RCD_BORDER || h <= 2 * RCD_BORDER {
            return base; // too small for RCD's support
        }
        let mut r = vec![0.0f32; w * h];
        let mut g = vec![0.0f32; w * h];
        let mut b = vec![0.0f32; w * h];
        for (i, px) in base.data.iter().enumerate() {
            r[i] = px[0];
            g[i] = px[1];
            b[i] = px[2];
        }

        let cfa = &img.data[..];
        let (w1, w2, w3, w4) = (w, 2 * w, 3 * w, 4 * w);

        // Vertical / horizontal high-pass at a single site.
        let vhpf = |x: usize, y: usize| -> f32 {
            let i = y * w + x;
            (cfa[i - w3] - cfa[i - w1] - cfa[i + w1] + cfa[i + w3])
                - 3.0 * (cfa[i - w2] + cfa[i + w2])
                + 6.0 * cfa[i]
        };
        let hhpf = |x: usize, y: usize| -> f32 {
            let i = y * w + x;
            (cfa[i - 3] - cfa[i - 1] - cfa[i + 1] + cfa[i + 3]) - 3.0 * (cfa[i - 2] + cfa[i + 2])
                + 6.0 * cfa[i]
        };

        // --- Step 1: V/H direction map (→1 = vertical detail, →0 = horizontal) ---
        let mut vh_dir = vec![0.5f32; w * h];
        for y in 4..h - 4 {
            for x in 4..w - 4 {
                let v = (sq(vhpf(x, y - 1)) + sq(vhpf(x, y)) + sq(vhpf(x, y + 1))).max(EPSSQ);
                let hs = (sq(hhpf(x - 1, y)) + sq(hhpf(x, y)) + sq(hhpf(x + 1, y))).max(EPSSQ);
                vh_dir[y * w + x] = v / (v + hs);
            }
        }

        // --- Step 2: low-pass luma proxy for the ratio correction ---
        let mut lpf = cfa.to_vec(); // border default = raw value
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let i = y * w + x;
                lpf[i] = cfa[i]
                    + 0.5 * (cfa[i - w1] + cfa[i + w1] + cfa[i - 1] + cfa[i + 1])
                    + 0.25
                        * (cfa[i - w1 - 1] + cfa[i - w1 + 1] + cfa[i + w1 - 1] + cfa[i + w1 + 1]);
            }
        }

        // Diagonal P ("\") and Q ("/") high-pass at a single site.
        let phpf = |x: usize, y: usize| -> f32 {
            let i = y * w + x;
            (cfa[i - w3 - 3] - cfa[i - w1 - 1] - cfa[i + w1 + 1] + cfa[i + w3 + 3])
                - 3.0 * (cfa[i - w2 - 2] + cfa[i + w2 + 2])
                + 6.0 * cfa[i]
        };
        let qhpf = |x: usize, y: usize| -> f32 {
            let i = y * w + x;
            (cfa[i - w3 + 3] - cfa[i - w1 + 1] - cfa[i + w1 - 1] + cfa[i + w3 - 3])
                - 3.0 * (cfa[i - w2 + 2] + cfa[i + w2 - 2])
                + 6.0 * cfa[i]
        };

        // --- Step 4 (computed up-front): P/Q diagonal direction map ---
        let mut pq_dir = vec![0.5f32; w * h];
        for y in 4..h - 4 {
            for x in 4..w - 4 {
                let p =
                    (sq(phpf(x - 1, y - 1)) + sq(phpf(x, y)) + sq(phpf(x + 1, y + 1))).max(EPSSQ);
                let q =
                    (sq(qhpf(x + 1, y - 1)) + sq(qhpf(x, y)) + sq(qhpf(x - 1, y + 1))).max(EPSSQ);
                pq_dir[y * w + x] = p / (p + q);
            }
        }

        // Blend weight: prefer the local direction, but if it is close to
        // undecided (0.5) trust the 4-diagonal neighborhood average instead.
        let disc = |dir: &[f32], i: usize| -> f32 {
            let central = dir[i];
            let nb = 0.25 * (dir[i - w1 - 1] + dir[i - w1 + 1] + dir[i + w1 - 1] + dir[i + w1 + 1]);
            if (0.5 - central).abs() < (0.5 - nb).abs() {
                nb
            } else {
                central
            }
        };

        // --- Step 3: green at red/blue sensels (ratio-corrected) ---
        for y in RCD_BORDER..h - RCD_BORDER {
            for x in RCD_BORDER..w - RCD_BORDER {
                if pattern.color_at(x, y) == 1 {
                    continue; // green sensels keep their measured value
                }
                let i = y * w + x;
                let n_grad = EPS
                    + (cfa[i - w1] - cfa[i + w1]).abs()
                    + (cfa[i] - cfa[i - w2]).abs()
                    + (cfa[i - w1] - cfa[i - w3]).abs()
                    + (cfa[i - w2] - cfa[i - w4]).abs();
                let s_grad = EPS
                    + (cfa[i - w1] - cfa[i + w1]).abs()
                    + (cfa[i] - cfa[i + w2]).abs()
                    + (cfa[i + w1] - cfa[i + w3]).abs()
                    + (cfa[i + w2] - cfa[i + w4]).abs();
                let w_grad = EPS
                    + (cfa[i - 1] - cfa[i + 1]).abs()
                    + (cfa[i] - cfa[i - 2]).abs()
                    + (cfa[i - 1] - cfa[i - 3]).abs()
                    + (cfa[i - 2] - cfa[i - 4]).abs();
                let e_grad = EPS
                    + (cfa[i - 1] - cfa[i + 1]).abs()
                    + (cfa[i] - cfa[i + 2]).abs()
                    + (cfa[i + 1] - cfa[i + 3]).abs()
                    + (cfa[i + 2] - cfa[i + 4]).abs();

                // lpf is a low-pass at R/B sites; the ratio uses the same-color
                // neighbor 2 rows/cols away (reference lpf is half-res, so its
                // `lpindx±w1`/`±1` map to full-res `±w2`/`±2`).
                let li = lpf[i];
                let n_est = cfa[i - w1] * (2.0 * li) / (EPS + li + lpf[i - w2]);
                let s_est = cfa[i + w1] * (2.0 * li) / (EPS + li + lpf[i + w2]);
                let w_est = cfa[i - 1] * (2.0 * li) / (EPS + li + lpf[i - 2]);
                let e_est = cfa[i + 1] * (2.0 * li) / (EPS + li + lpf[i + 2]);

                let v_est = (s_grad * n_est + n_grad * s_est) / (n_grad + s_grad);
                let h_est = (w_grad * e_est + e_grad * w_est) / (e_grad + w_grad);

                // intp(VH_Disc, H_Est, V_Est) = VH_Disc*H_Est + (1-VH_Disc)*V_Est
                let vh = disc(&vh_dir, i);
                g[i] = vh * h_est + (1.0 - vh) * v_est;
            }
        }

        // --- Step 5a: red & blue at the opposite-color sensels (diagonals) ---
        for y in RCD_BORDER..h - RCD_BORDER {
            for x in RCD_BORDER..w - RCD_BORDER {
                // the diagonal color to fill: red at blue sensels, blue at red.
                let cc = match pattern.color_at(x, y) {
                    0 => &mut b, // red sensel → interpolate blue
                    2 => &mut r, // blue sensel → interpolate red
                    _ => continue,
                };
                let i = y * w + x;
                let nw_grad = EPS
                    + (cc[i - w1 - 1] - cc[i + w1 + 1]).abs()
                    + (cc[i - w1 - 1] - cc[i - w3 - 3]).abs()
                    + (g[i] - g[i - w2 - 2]).abs();
                let ne_grad = EPS
                    + (cc[i - w1 + 1] - cc[i + w1 - 1]).abs()
                    + (cc[i - w1 + 1] - cc[i - w3 + 3]).abs()
                    + (g[i] - g[i - w2 + 2]).abs();
                let sw_grad = EPS
                    + (cc[i - w1 + 1] - cc[i + w1 - 1]).abs()
                    + (cc[i + w1 - 1] - cc[i + w3 - 3]).abs()
                    + (g[i] - g[i + w2 - 2]).abs();
                let se_grad = EPS
                    + (cc[i - w1 - 1] - cc[i + w1 + 1]).abs()
                    + (cc[i + w1 + 1] - cc[i + w3 + 3]).abs()
                    + (g[i] - g[i + w2 + 2]).abs();

                let nw_est = cc[i - w1 - 1] - g[i - w1 - 1];
                let ne_est = cc[i - w1 + 1] - g[i - w1 + 1];
                let sw_est = cc[i + w1 - 1] - g[i + w1 - 1];
                let se_est = cc[i + w1 + 1] - g[i + w1 + 1];

                let p_est = (nw_grad * se_est + se_grad * nw_est) / (nw_grad + se_grad);
                let q_est = (ne_grad * sw_est + sw_grad * ne_est) / (ne_grad + sw_grad);

                // intp(PQ_Disc, Q_Est, P_Est) = PQ_Disc*Q_Est + (1-PQ_Disc)*P_Est
                let pq = disc(&pq_dir, i);
                cc[i] = g[i] + (pq * q_est + (1.0 - pq) * p_est);
            }
        }

        // --- Step 5b: red & blue at the green sensels (cardinals) ---
        for y in RCD_BORDER..h - RCD_BORDER {
            for x in RCD_BORDER..w - RCD_BORDER {
                if pattern.color_at(x, y) != 1 {
                    continue;
                }
                let i = y * w + x;
                let vh = disc(&vh_dir, i);
                let n1 = EPS + (g[i] - g[i - w2]).abs();
                let s1 = EPS + (g[i] - g[i + w2]).abs();
                let w1g = EPS + (g[i] - g[i - 2]).abs();
                let e1 = EPS + (g[i] - g[i + 2]).abs();

                for cc in [&mut r, &mut b] {
                    let n_grad =
                        n1 + (cc[i - w1] - cc[i + w1]).abs() + (cc[i - w1] - cc[i - w3]).abs();
                    let s_grad =
                        s1 + (cc[i - w1] - cc[i + w1]).abs() + (cc[i + w1] - cc[i + w3]).abs();
                    let w_grad =
                        w1g + (cc[i - 1] - cc[i + 1]).abs() + (cc[i - 1] - cc[i - 3]).abs();
                    let e_grad = e1 + (cc[i - 1] - cc[i + 1]).abs() + (cc[i + 1] - cc[i + 3]).abs();

                    let n_est = cc[i - w1] - g[i - w1];
                    let s_est = cc[i + w1] - g[i + w1];
                    let w_est = cc[i - 1] - g[i - 1];
                    let e_est = cc[i + 1] - g[i + 1];

                    let v_est = (n_grad * s_est + s_grad * n_est) / (n_grad + s_grad);
                    let h_est = (e_grad * w_est + w_grad * e_est) / (e_grad + w_grad);
                    // intp(VH_Disc, H_Est, V_Est)
                    cc[i] = g[i] + (vh * h_est + (1.0 - vh) * v_est);
                }
            }
        }

        // Pack planes back, flooring out-of-gamut negatives (headroom kept).
        let mut out = RgbImage::new(w, h);
        for i in 0..w * h {
            out.data[i] = [r[i].max(0.0), g[i].max(0.0), b[i].max(0.0)];
        }
        out
    }

    fn name(&self) -> &'static str {
        "rcd"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demosaic::tests_common::{max_interior_error, mosaic_from_fn};
    use crate::image::CfaPattern;

    #[test]
    fn constant_field_reconstructs_exactly() {
        let cfa = mosaic_from_fn(48, 40, CfaPattern::Rggb, |_, _| 0.5);
        let rgb = Rcd.demosaic(&cfa);
        for px in &rgb.data {
            for c in px {
                assert!((c - 0.5).abs() < 1e-4, "got {c}");
            }
        }
    }

    #[test]
    fn linear_gradient_low_error_in_interior() {
        // Ratio correction is mildly nonlinear, so tolerance is looser than the
        // linear methods but still tight on a gentle ramp.
        let grad = |x: usize, y: usize| 0.2 + 0.004 * x as f32 + 0.003 * y as f32;
        let cfa = mosaic_from_fn(64, 56, CfaPattern::Rggb, grad);
        let rgb = Rcd.demosaic(&cfa);
        let err = max_interior_error(&rgb, RCD_BORDER, grad);
        assert!(err < 5e-3, "max interior error {err}");
    }

    #[test]
    fn all_patterns_reconstruct_constant() {
        for p in [
            CfaPattern::Rggb,
            CfaPattern::Bggr,
            CfaPattern::Grbg,
            CfaPattern::Gbrg,
        ] {
            let cfa = mosaic_from_fn(40, 40, p, |_, _| 0.35);
            let rgb = Rcd.demosaic(&cfa);
            for px in &rgb.data {
                for c in px {
                    assert!((c - 0.35).abs() < 1e-4, "pattern {:?} got {c}", p);
                }
            }
        }
    }
}
