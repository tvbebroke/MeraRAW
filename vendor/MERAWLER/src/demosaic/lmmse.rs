//! LMMSE — Linear Minimum Mean Square-Error demosaicing.
//!
//! L. Zhang and X. Wu, "Color demosaicking via directional linear minimum
//! mean square-error estimation", IEEE TIP 14(12), 2005.
//!
//! LMMSE is the best choice for **noisy** captures and complements RCD: instead
//! of picking a single interpolation direction, it forms a color-difference
//! estimate (`green - red/blue`) both horizontally and vertically, then fuses
//! the two by their statistically estimated reliability. Concretely, per
//! direction it splits the low-passed color difference into a "signal" variance
//! (`vx`) and a residual/"noise" variance (`vn`) over a 9-tap window; the MMSE
//! estimate leans on whichever direction has the cleaner signal, which is
//! exactly what suppresses noise-driven false color.
//!
//! Pipeline: gamma-compand → directional green (as color differences) →
//! Gaussian low-pass of the differences → per-direction LMMSE fusion → rebuild
//! green, then red/blue from the color differences → inverse gamma.
//!
//! This is an original Rust implementation written from the published algorithm
//! (constants/offsets are mathematical facts), not a port of the GPL reference.
//! It runs whole-image with a bilinear-seeded border (median-refinement
//! iterations, used only for the non-default `iterations > 1` mode, are omitted).

use crate::image::{CfaImage, RgbImage};
use super::bilinear::Bilinear;
use super::Demosaic;

pub struct Lmmse;

/// Neighbor reach: low-pass (±4) stacked on the LMMSE window (±4) plus the
/// two color-difference reconstruction passes.
const BA: usize = 12;

impl Demosaic for Lmmse {
    fn demosaic(&self, img: &CfaImage) -> RgbImage {
        let (w, h) = (img.width, img.height);
        let pattern = img.pattern;

        let mut out = Bilinear.demosaic(img); // seed borders + fallback
        if w <= 2 * BA || h <= 2 * BA {
            return out;
        }
        let cfa = &img.data[..];
        let (w1, w2, w3, w4) = (w, 2 * w, 3 * w, 4 * w);

        // Gaussian low-pass weights (exp(-k^2/8), normalized) for the 9-tap FIR.
        let (mut h0, mut h1, mut h2, mut h3, mut h4) = (
            1.0f32,
            (-1.0f32 / 8.0).exp(),
            (-4.0f32 / 8.0).exp(),
            (-9.0f32 / 8.0).exp(),
            (-16.0f32 / 8.0).exp(),
        );
        let hs = h0 + 2.0 * (h1 + h2 + h3 + h4);
        h0 /= hs;
        h1 /= hs;
        h2 /= hs;
        h3 /= hs;
        h4 /= hs;

        // Work in a mild gamma space (matches the reference's default), so the
        // [0,1] clips and variance weighting behave perceptually.
        let n = w * h;
        let mut gam = vec![0.0f32; n];
        for i in 0..n {
            gam[i] = gamma(cfa[i]);
        }

        // Horizontal / vertical color differences (G - R/B) at every sensel.
        let mut hd = vec![0.0f32; n];
        let mut vd = vec![0.0f32; n];
        for y in 2..h - 2 {
            for x in 2..w - 2 {
                let i = y * w + x;
                if pattern.color_at(x, y) == 1 {
                    // green sensel: difference to the interpolated opposite color
                    let a = 0.25 * (gam[i - 2] + gam[i + 2])
                        - 0.5 * (gam[i - 1] + gam[i] + gam[i + 1]);
                    let b = 0.25 * (gam[i - w2] + gam[i + w2])
                        - 0.5 * (gam[i - w1] + gam[i] + gam[i + w1]);
                    hd[i] = clamp(a, -1.0, 0.0) + gam[i];
                    vd[i] = clamp(b, -1.0, 0.0) + gam[i];
                } else {
                    // red/blue sensel: interpolate green, keep it edge-safe
                    let v0 = 0.0625
                        * (gam[i - w1 - 1] + gam[i - w1 + 1] + gam[i + w1 - 1] + gam[i + w1 + 1])
                        + 0.25 * gam[i];
                    let mut gh =
                        -0.25 * (gam[i - 2] + gam[i + 2]) + 0.5 * (gam[i - 1] + gam[i] + gam[i + 1]);
                    let yh = v0 + 0.5 * gh;
                    gh = if gam[i] > 1.75 * yh {
                        median3(gh, gam[i - 1], gam[i + 1])
                    } else {
                        clamp(gh, 0.0, 1.0)
                    };
                    hd[i] = gh - gam[i];

                    let mut gv = -0.25 * (gam[i - w2] + gam[i + w2])
                        + 0.5 * (gam[i - w1] + gam[i] + gam[i + w1]);
                    let yv = v0 + 0.5 * gv;
                    gv = if gam[i] > 1.75 * yv {
                        median3(gv, gam[i - w1], gam[i + w1])
                    } else {
                        clamp(gv, 0.0, 1.0)
                    };
                    vd[i] = gv - gam[i];
                }
            }
        }

        // Low-pass the color differences (horizontal taps for hd, vertical for vd).
        let mut hlp = vec![0.0f32; n];
        let mut vlp = vec![0.0f32; n];
        for y in 6..h - 6 {
            for x in 6..w - 6 {
                let i = y * w + x;
                hlp[i] = h0 * hd[i]
                    + h1 * (hd[i - 1] + hd[i + 1])
                    + h2 * (hd[i - 2] + hd[i + 2])
                    + h3 * (hd[i - 3] + hd[i + 3])
                    + h4 * (hd[i - 4] + hd[i + 4]);
                vlp[i] = h0 * vd[i]
                    + h1 * (vd[i - w1] + vd[i + w1])
                    + h2 * (vd[i - w2] + vd[i + w2])
                    + h3 * (vd[i - w3] + vd[i + w3])
                    + h4 * (vd[i - w4] + vd[i + w4]);
            }
        }

        // Gamma-space channel planes. Measured color seeded everywhere; the rest
        // filled by LMMSE (green) and color-difference interpolation (red/blue).
        let mut gr = vec![0.0f32; n];
        let mut gg = vec![0.0f32; n];
        let mut gb = vec![0.0f32; n];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                match pattern.color_at(x, y) {
                    0 => gr[i] = gam[i],
                    2 => gb[i] = gam[i],
                    _ => gg[i] = gam[i],
                }
            }
        }

        // Each pass feeds the next at ±1, so widen the earlier passes: LMMSE
        // green runs 2px past the output ring, green-site R/B 1px past.
        // LMMSE fusion: refined G - R/B at red/blue sensels → green.
        for y in BA - 2..h - (BA - 2) {
            for x in BA - 2..w - (BA - 2) {
                if pattern.color_at(x, y) == 1 {
                    continue;
                }
                let i = y * w + x;
                let (xh, vh) = lmmse_dir(&hlp, &hd, i, 1);
                let (xv, vv) = lmmse_dir(&vlp, &vd, i, w1);
                gg[i] = gam[i] + (xh * vv + xv * vh) / (vh + vv);
            }
        }

        // Red/blue at green sensels (color-difference bilinear, cardinal).
        for y in BA - 1..h - (BA - 1) {
            for x in BA - 1..w - (BA - 1) {
                if pattern.color_at(x, y) != 1 {
                    continue;
                }
                let i = y * w + x;
                let ch = pattern.color_at(x + 1, y) as usize; // horizontal neighbor color
                let cv = 2 - ch; // vertical neighbor color
                let p_h = plane(&mut gr, &mut gb, ch);
                p_h[i] = gg[i] + 0.5 * (p_h[i - 1] - gg[i - 1] + p_h[i + 1] - gg[i + 1]);
                let p_v = plane(&mut gr, &mut gb, cv);
                p_v[i] = gg[i] + 0.5 * (p_v[i - w1] - gg[i - w1] + p_v[i + w1] - gg[i + w1]);
            }
        }

        // Red/blue at the opposite-color sensels (from the just-filled greens).
        for y in BA..h - BA {
            for x in BA..w - BA {
                let opp = match pattern.color_at(x, y) {
                    0 => 2usize, // red sensel → fill blue
                    2 => 0usize, // blue sensel → fill red
                    _ => continue,
                };
                let i = y * w + x;
                let p = plane(&mut gr, &mut gb, opp);
                p[i] = gg[i]
                    + 0.25
                        * (p[i - w1] - gg[i - w1] + p[i - 1] - gg[i - 1] + p[i + 1] - gg[i + 1]
                            + p[i + w1] - gg[i + w1]);
            }
        }

        // Emit linear RGB: measured channel stays linear, interpolated channels
        // are un-gamma'd. (Green at green sensels un-gammas back to the measured
        // value, so the whole frame is consistent.)
        for y in BA..h - BA {
            for x in BA..w - BA {
                let i = y * w + x;
                let c = pattern.color_at(x, y);
                let r = if c == 0 { cfa[i] } else { inv_gamma(gr[i]) };
                let g = if c == 1 { cfa[i] } else { inv_gamma(gg[i]) };
                let b = if c == 2 { cfa[i] } else { inv_gamma(gb[i]) };
                out.data[i] = [r.max(0.0), g.max(0.0), b.max(0.0)];
            }
        }
        out
    }

    fn name(&self) -> &'static str {
        "lmmse"
    }
}

/// One direction of the LMMSE estimate over a 9-tap window with stride `s`
/// (1 = horizontal, `w` = vertical). Returns `(estimate, variance)`:
/// `vx` is the low-passed signal variance, `vn` the residual variance, and the
/// estimate leans toward whichever dominates.
#[inline]
fn lmmse_dir(lp: &[f32], diff: &[f32], i: usize, s: usize) -> (f32, f32) {
    let p = [
        lp[i - 4 * s],
        lp[i - 3 * s],
        lp[i - 2 * s],
        lp[i - s],
        lp[i],
        lp[i + s],
        lp[i + 2 * s],
        lp[i + 3 * s],
        lp[i + 4 * s],
    ];
    let mu = (p[0] + p[1] + p[2] + p[3] + p[4] + p[5] + p[6] + p[7] + p[8]) / 9.0;
    let mut vx = 1e-7f32;
    for &v in &p {
        vx += (v - mu) * (v - mu);
    }
    let off = [-4isize, -3, -2, -1, 0, 1, 2, 3, 4];
    let mut vn = 1e-7f32;
    for (k, &o) in off.iter().enumerate() {
        let j = (i as isize + o * s as isize) as usize;
        let e = p[k] - diff[j];
        vn += e * e;
    }
    let est = (diff[i] * vx + lp[i] * vn) / (vx + vn);
    let var = vx * vn / (vx + vn);
    (est, var)
}

/// Mutable borrow of the red (0) or blue (2) plane; green (1) is never asked.
#[inline]
fn plane<'a>(gr: &'a mut [f32], gb: &'a mut [f32], c: usize) -> &'a mut [f32] {
    match c {
        0 => gr,
        2 => gb,
        _ => unreachable!("green plane not requested here"),
    }
}

#[inline]
fn median3(a: f32, b: f32, c: f32) -> f32 {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    hi.min(lo.max(c))
}

#[inline]
fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}

/// Forward gamma companding (reference's default curve).
#[inline]
fn gamma(x: f32) -> f32 {
    if x <= 0.001867 {
        x * 17.0
    } else {
        1.044445 * x.powf(1.0 / 2.4) - 0.044445
    }
}

/// Inverse of [`gamma`].
#[inline]
fn inv_gamma(y: f32) -> f32 {
    if y <= 0.031746 {
        y / 17.0
    } else {
        ((y + 0.044445) / 1.044445).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::CfaPattern;
    use crate::demosaic::tests_common::{max_interior_error, mosaic_from_fn};

    #[test]
    fn gamma_roundtrips() {
        for &x in &[0.0f32, 0.0005, 0.05, 0.2, 0.5, 0.9, 1.0] {
            assert!((inv_gamma(gamma(x)) - x).abs() < 1e-4, "x={x}");
        }
    }

    #[test]
    fn constant_field_reconstructs_exactly() {
        let cfa = mosaic_from_fn(64, 56, CfaPattern::Rggb, |_, _| 0.5);
        let rgb = Lmmse.demosaic(&cfa);
        for px in &rgb.data {
            for c in px {
                assert!((c - 0.5).abs() < 1e-3, "got {c}");
            }
        }
    }

    #[test]
    fn linear_gradient_low_error_in_interior() {
        let grad = |x: usize, y: usize| 0.25 + 0.003 * x as f32 + 0.002 * y as f32;
        let cfa = mosaic_from_fn(72, 64, CfaPattern::Rggb, grad);
        let rgb = Lmmse.demosaic(&cfa);
        let err = max_interior_error(&rgb, BA, grad);
        assert!(err < 1.5e-2, "max interior error {err}");
    }

    #[test]
    fn all_patterns_reconstruct_constant() {
        for p in [
            CfaPattern::Rggb,
            CfaPattern::Bggr,
            CfaPattern::Grbg,
            CfaPattern::Gbrg,
        ] {
            let cfa = mosaic_from_fn(48, 48, p, |_, _| 0.4);
            let rgb = Lmmse.demosaic(&cfa);
            for px in &rgb.data {
                for c in px {
                    assert!((c - 0.4).abs() < 1e-3, "pattern {:?} got {c}", p);
                }
            }
        }
    }
}
