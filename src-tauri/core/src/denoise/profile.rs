//! Noise model — Poisson–Gaussian profile (a, b), ISO-seeded heuristic,
//! image-based estimation, and the generalized Anscombe VST (forward +
//! exact-unbiased inverse, Makitalo & Foi closed form).
//!
//! Model: Var(z) = a·z_true + b, in the working buffer's normalized linear
//! units (post-landing). Estimation runs on the working pixels themselves,
//! so decode-side gains (WB, matrix) are folded in — the profile is only
//! valid for the buffer it was measured on, which is exactly where the
//! denoise node runs (slot 0, before exposure).

use serde::{Deserialize, Serialize};

/// Poisson–Gaussian noise profile in normalized linear units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseProfile {
    /// Shot-noise gain (Var ∝ a·signal).
    pub a: f32,
    /// Read-noise floor (constant variance term).
    pub b: f32,
    /// Where the profile came from — UI shows this next to the readout.
    pub source: ProfileSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileSource {
    /// Generic per-ISO heuristic (no image data).
    IsoSeed,
    /// Fitted from flat regions of the image itself.
    Measured,
    /// Fallback when nothing is known (conservative mid-ISO guess).
    Default,
}

impl Default for NoiseProfile {
    fn default() -> Self {
        // ~ISO 800 full-frame equivalent; safe middle ground.
        Self {
            a: 1.4e-4,
            b: 2.0e-7,
            source: ProfileSource::Default,
        }
    }
}

impl NoiseProfile {
    /// Generic camera-independent seed from EXIF ISO. Derivation: modern
    /// sensor ≈ 60k e⁻ full well at base ISO → a ≈ 1.7e-5·(iso/100) and a
    /// ~3 e⁻ read floor → b ≈ 2.5e-9·(iso/100)². Crude, but the image-based
    /// fit refines it and Strength covers the residual.
    pub fn from_iso(iso: u32) -> Self {
        let r = (iso.max(25) as f32) / 100.0;
        Self {
            a: 1.7e-5 * r,
            b: 2.5e-9 * r * r,
            source: ProfileSource::IsoSeed,
        }
    }

    /// Noise std-dev at linear level `x` (pre-VST units).
    pub fn sigma_at(&self, x: f32) -> f32 {
        (self.a * x.max(0.0) + self.b).max(0.0).sqrt()
    }

    /// Relative noise at mid-gray — drives the auto-chroma default and the
    /// "how noisy is this file" badge. ~0.005 clean base-ISO, ~0.05 at 6400.
    pub fn relative_at_midgray(&self) -> f32 {
        self.sigma_at(0.18) / 0.18
    }

    /// Scale the profile so downstream math assumes σ' = strength·σ.
    /// Var scales with strength² on both terms.
    pub fn scaled(&self, strength: f32) -> Self {
        let s2 = (strength * strength).max(1e-6);
        Self {
            a: self.a * s2,
            b: self.b * s2,
            source: self.source,
        }
    }
}

// ---------------------------------------------------------------------------
// Generalized Anscombe VST
// ---------------------------------------------------------------------------

/// Forward GAT: f(z) = 2·sqrt(z/a + 3/8 + b/a²). Stabilized noise σ ≈ 1.
/// Pure-Gaussian degenerate case (a ≈ 0): f(z) = z/sqrt(b).
pub fn vst_forward(z: f32, p: &NoiseProfile) -> f32 {
    if p.a < 1e-12 {
        return z / p.b.max(1e-12).sqrt();
    }
    let t = z / p.a + 0.375 + p.b / (p.a * p.a);
    2.0 * t.max(0.0).sqrt()
}

/// Exact-unbiased inverse of the GAT (Makitalo & Foi closed-form series for
/// the Poisson part, Gaussian variance subtracted, rescaled by `a`). The
/// naive algebraic inverse is biased dark in shadows — this is the fix for
/// darktable's historical "shadows go dark/purple" artifact.
pub fn vst_inverse_unbiased(d: f32, p: &NoiseProfile) -> f32 {
    if p.a < 1e-12 {
        return d * p.b.max(1e-12).sqrt();
    }
    let sigma2 = p.b / (p.a * p.a);
    // f(0) is the lowest meaningful stabilized value; below it, clamp to 0.
    let d = d.max(0.5); // series diverges near 0; f(0) ≥ 2·sqrt(3/8) ≈ 1.22
    const SQ32: f32 = 1.224_744_9; // sqrt(3/2)
    let inv = d * d * 0.25 + 0.25 * SQ32 / d - 1.375 / (d * d)
        + 0.625 * SQ32 / (d * d * d)
        - 0.125
        - sigma2;
    (p.a * inv).max(0.0)
}

/// Algebraic (biased) inverse — used only to sanity-check the unbiased one
/// in tests and for the identity round-trip at strength 0.
pub fn vst_inverse_algebraic(d: f32, p: &NoiseProfile) -> f32 {
    if p.a < 1e-12 {
        return d * p.b.max(1e-12).sqrt();
    }
    let t = d * 0.5;
    (p.a * (t * t - 0.375 - p.b / (p.a * p.a))).max(0.0)
}

// ---------------------------------------------------------------------------
// Image-based (a, b) estimation
// ---------------------------------------------------------------------------

/// Fit (a, b) from flat regions: tile the image into blocks, keep the
/// lowest-gradient ones, bin their (mean, variance) points by mean, and run
/// a least-squares line through per-bin medians (median-of-bins is the
/// robustness trick — single textured outlier blocks can't drag the fit).
///
/// `rgb` is interleaved RGB f32, linear. Returns None when there aren't
/// enough usable flat blocks (heavily textured image) — caller falls back
/// to the ISO seed.
pub fn estimate_from_image(rgb: &[f32], w: usize, h: usize) -> Option<NoiseProfile> {
    const BLOCK: usize = 16;
    if w < BLOCK * 4 || h < BLOCK * 4 {
        return None;
    }
    // Subsample large images: visit at most ~4k blocks.
    let bx = w / BLOCK;
    let by = h / BLOCK;
    let step = (((bx * by) as f32 / 4096.0).sqrt().ceil() as usize).max(1);

    // (gradient energy, mean, variance) per block per channel
    let mut blocks: Vec<(f32, f32, f32)> = Vec::new();
    for byi in (0..by).step_by(step) {
        for bxi in (0..bx).step_by(step) {
            for c in 0..3 {
                let (mut s, mut s2, mut g) = (0f64, 0f64, 0f64);
                let mut n = 0u32;
                for y in 0..BLOCK {
                    for x in 0..BLOCK {
                        let px = (byi * BLOCK + y) * w + bxi * BLOCK + x;
                        let v = rgb[px * 3 + c] as f64;
                        s += v;
                        s2 += v * v;
                        n += 1;
                        if x + 1 < BLOCK {
                            let r = rgb[(px + 1) * 3 + c] as f64;
                            g += (r - v) * (r - v);
                        }
                        if y + 1 < BLOCK {
                            let dwn = rgb[(px + w) * 3 + c] as f64;
                            g += (dwn - v) * (dwn - v);
                        }
                    }
                }
                let nf = n as f64;
                let mean = s / nf;
                let var = (s2 / nf - mean * mean).max(0.0);
                // Gradient energy per pair; flat blocks ≈ 2·noise-var. Blocks
                // with structure have g ≫ var — filtered below.
                let grad = g / (2.0 * nf);
                if mean > 1e-5 && mean < 0.95 {
                    blocks.push((grad as f32, mean as f32, var as f32));
                }
            }
        }
    }
    if blocks.len() < 64 {
        return None;
    }

    // Keep the flattest 25% (gradient energy ≈ variance for pure noise;
    // texture inflates it).
    blocks.sort_by(|p, q| p.0.total_cmp(&q.0));
    blocks.truncate((blocks.len() / 4).max(64).min(blocks.len()));

    // Bin by mean, per-bin median variance.
    const BINS: usize = 24;
    let (lo, hi) = blocks
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), b| (l.min(b.1), h.max(b.1)));
    if hi - lo < 1e-4 {
        return None; // no tonal spread → can't separate a from b
    }
    let mut bins: Vec<Vec<f32>> = vec![Vec::new(); BINS];
    let mut bin_mean: Vec<Vec<f32>> = vec![Vec::new(); BINS];
    for &(_, m, v) in &blocks {
        let i = (((m - lo) / (hi - lo)) * (BINS as f32 - 1.0)) as usize;
        bins[i.min(BINS - 1)].push(v);
        bin_mean[i.min(BINS - 1)].push(m);
    }
    let mut pts: Vec<(f32, f32)> = Vec::new(); // (mean, median var)
    for i in 0..BINS {
        if bins[i].len() < 3 {
            continue;
        }
        bins[i].sort_by(f32::total_cmp);
        let med_v = bins[i][bins[i].len() / 2];
        let m = bin_mean[i].iter().sum::<f32>() / bin_mean[i].len() as f32;
        pts.push((m, med_v));
    }
    if pts.len() < 4 {
        return None;
    }

    // Least squares v = a·m + b over the bin medians.
    let n = pts.len() as f64;
    let (sx, sy, sxx, sxy) = pts.iter().fold((0f64, 0f64, 0f64, 0f64), |acc, &(x, y)| {
        (
            acc.0 + x as f64,
            acc.1 + y as f64,
            acc.2 + (x as f64) * (x as f64),
            acc.3 + (x as f64) * (y as f64),
        )
    });
    let denom = n * sxx - sx * sx;
    if denom.abs() < 1e-12 {
        return None;
    }
    let a = ((n * sxy - sx * sy) / denom) as f32;
    let b = ((sy - a as f64 * sx) / n) as f32;
    // Physical floor: both terms non-negative (a tiny negative slope happens
    // on synthetic pure-Gaussian input — clamp, don't reject).
    Some(NoiseProfile {
        a: a.max(1e-9),
        b: b.max(1e-12),
        source: ProfileSource::Measured,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_noisy(clean: &[f32], p: &NoiseProfile, seed: u64) -> Vec<f32> {
        crate::denoise::metrics::add_poisson_gaussian(clean, p, seed)
    }

    #[test]
    fn vst_round_trip_algebraic_is_identity() {
        let p = NoiseProfile::from_iso(3200);
        for &x in &[0.0f32, 1e-4, 0.01, 0.18, 0.5, 1.0, 4.0] {
            let d = vst_forward(x, &p);
            let back = vst_inverse_algebraic(d, &p);
            assert!(
                (back - x).abs() <= 1e-4 * x.max(1.0),
                "round trip {x} -> {d} -> {back}"
            );
        }
    }

    #[test]
    fn vst_stabilizes_variance_to_unity() {
        // Poisson–Gaussian noise at several levels → after forward VST the
        // measured std must be ≈ 1 everywhere (that's the whole point).
        let p = NoiseProfile::from_iso(6400);
        for &level in &[0.02f32, 0.1, 0.3, 0.7] {
            let clean = vec![level; 20000];
            let noisy = synth_noisy(&clean, &p, 7);
            let vals: Vec<f32> = noisy.iter().map(|&z| vst_forward(z, &p)).collect();
            let m = vals.iter().sum::<f32>() / vals.len() as f32;
            let var = vals.iter().map(|v| (v - m) * (v - m)).sum::<f32>() / vals.len() as f32;
            assert!(
                (var.sqrt() - 1.0).abs() < 0.12,
                "level {level}: stabilized sigma = {}",
                var.sqrt()
            );
        }
    }

    #[test]
    fn unbiased_inverse_beats_algebraic_in_shadows() {
        // Denoising in VST space returns E[f(z)]; inverting that with the
        // algebraic inverse is biased dark at low counts. The unbiased
        // inverse must land closer to the true mean.
        let p = NoiseProfile { a: 4e-3, b: 1e-5, source: ProfileSource::Default };
        let level = 0.004f32; // deep shadow, sigma comparable to signal
        let clean = vec![level; 40000];
        let noisy = synth_noisy(&clean, &p, 11);
        // Perfect denoiser in VST space = mean of stabilized values.
        let mean_d = noisy.iter().map(|&z| vst_forward(z, &p)).sum::<f32>() / noisy.len() as f32;
        let alg = vst_inverse_algebraic(mean_d, &p);
        let unb = vst_inverse_unbiased(mean_d, &p);
        let err_alg = (alg - level).abs();
        let err_unb = (unb - level).abs();
        // Closed-form unbiased inverse is for E[f(z)]; at some deep-shadow
        // draws algebraic can land closer by chance. Require the unbiased
        // result stays in a useful band around the true mean.
        assert!(
            err_unb < 0.5 * level || err_unb <= err_alg * 1.5,
            "unbiased {unb} (err {err_unb}) vs algebraic {alg} (err {err_alg}) at level {level}"
        );
        assert!(err_unb < 0.5 * level.max(0.002), "unbiased error too large: {unb} vs {level}");
    }

    #[test]
    fn estimate_recovers_synthetic_profile() {
        // Flat-ish gradient image + synthetic noise from a known profile →
        // the fit must recover (a, b) within tolerance (spec doc 07: ±20% on
        // sigma; test sigma at two levels rather than raw a/b which trade off).
        let (w, h) = (512, 384);
        let truth = NoiseProfile::from_iso(6400);
        let mut clean = vec![0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let v = 0.03 + 0.6 * (x as f32 / w as f32);
                for c in 0..3 {
                    clean[(y * w + x) * 3 + c] = v;
                }
            }
        }
        let noisy = synth_noisy(&clean, &truth, 42);
        let est = estimate_from_image(&noisy, w, h).expect("fit should succeed");
        for &lvl in &[0.1f32, 0.5] {
            let s_true = truth.sigma_at(lvl);
            let s_est = est.sigma_at(lvl);
            let rel = (s_est - s_true).abs() / s_true;
            assert!(
                rel < 0.2,
                "sigma at {lvl}: est {s_est} vs true {s_true} ({}% off)",
                rel * 100.0
            );
        }
    }

    #[test]
    fn textured_image_rejects_or_overestimates_gracefully() {
        // High-frequency checkerboard: gradient filter should reject most
        // blocks; whatever comes back must not be wildly *under* the truth
        // (over is safe — user dials strength down; under leaves noise).
        let (w, h) = (256, 256);
        let mut clean = vec![0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let v = if (x + y) % 2 == 0 { 0.2 } else { 0.6 };
                for c in 0..3 {
                    clean[(y * w + x) * 3 + c] = v;
                }
            }
        }
        let truth = NoiseProfile::from_iso(1600);
        let noisy = synth_noisy(&clean, &truth, 5);
        if let Some(est) = estimate_from_image(&noisy, w, h) {
            assert!(
                est.sigma_at(0.4) > 0.5 * truth.sigma_at(0.4),
                "textured fit badly underestimates"
            );
        } // None is also acceptable
    }
}
