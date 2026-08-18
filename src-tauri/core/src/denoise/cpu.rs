//! CPU reference implementation of the classical denoise chain. Three jobs:
//! numerical ground truth for GPU parity tests, the no-GPU export fallback,
//! and the engine the bench harness measures. Mirrors the WGSL passes
//! stage-for-stage — any change here must ship with the shader change.
//!
//! Chain (doc 04): impulse median → VST forward (per RGB channel) →
//! RGB→Y0U0V0 → {à-trous wavelet | NLM luma + wavelet chroma} → detail
//! recovery → texture-mask blend → Y0U0V0→RGB → unbiased inverse VST.
//!
//! Pinned deviation from doc 04 §P3.a: VST runs BEFORE the Y0U0V0 rotation
//! (chroma axes go negative, sqrt can't take them; rotating stabilized
//! channels keeps σ analytic: σ_Y0 = 1/√3, σ_U0 = 1/√2, σ_V0 = √6/4).

use super::profile::{vst_forward, vst_inverse_unbiased, NoiseProfile};

/// Per-channel noise σ in Y0U0V0 after unit-variance RGB stabilization.
/// Literals are pinned to `graph/noise.wgsl` — do not "simplify" to std consts
/// (1 ulp of drift breaks GPU↔CPU parity).
pub const SIGMA_Y0: f32 = 0.577_350_3; // sqrt(3)/3
#[allow(clippy::approx_constant)]
pub const SIGMA_U0: f32 = 0.707_106_8; // 1/√2, matches noise.wgsl
pub const SIGMA_V0: f32 = 0.612_372_4; // sqrt(6)/4

/// À-trous B3-spline per-level noise attenuation for white input noise —
/// measured by the monte-carlo test below (clamp-to-edge + finite sample).
pub const ATROUS_SIGMA: [f32; 6] = [0.8907, 0.2007, 0.0855, 0.0531, 0.0265, 0.0133];

pub const LUMA_LEVELS: usize = 5;
pub const CHROMA_LEVELS: usize = 6;

/// Simple-slider → per-level Wiener aggressiveness (multiplies σ_level).
/// Fine levels lead for luma; coarse levels boosted for chroma (blotches).
pub const LUMA_CURVE: [f32; 6] = [1.0, 0.95, 0.85, 0.70, 0.50, 0.30];
pub const CHROMA_CURVE: [f32; 6] = [0.85, 0.95, 1.0, 1.0, 1.0, 1.0];
/// Slider 100 → this many σ subtracted at curve weight 1.
pub const LUMA_MAX_S: f32 = 2.5;
pub const CHROMA_MAX_S: f32 = 3.5;

/// Fully-resolved parameters for one chain run (all curves flattened; this
/// is also exactly what gets packed into the GPU uniform).
#[derive(Debug, Clone)]
pub struct ChainParams {
    pub profile: NoiseProfile, // already strength-scaled
    /// Per-level σ·s thresholds, luma channel (0 disables a level).
    pub luma_s: [f32; 6],
    /// Per-level σ·s thresholds, chroma channels.
    pub chroma_s: [f32; 6],
    pub luma_levels: usize,
    pub chroma_levels: usize,
    /// 0..1 — detail recovery (residual add-back gain).
    pub detail: f32,
    /// Impulse median threshold in σ units; 0 = pass disabled.
    pub impulse_k: f32,
    /// Texture-mask protection: Conservative = 3.0, Aggressive = 1.5.
    pub texture_k: f32,
    /// true → NLM for luma (chroma stays wavelet).
    pub use_nlm: bool,
    pub nlm_patch: i32,  // radius (1 = 3×3)
    pub nlm_search: i32, // radius
    pub nlm_h: f32,      // filtering strength (σ units)
    pub nlm_center: f32, // central-pixel weight 0..1
    /// σ scale for preview-resolution processing (≤1; downsampling averages
    /// noise away, σ_preview ≈ σ · scale).
    pub sigma_scale: f32,
}

impl ChainParams {
    /// Map the user-facing sliders onto a resolved parameter set.
    #[allow(clippy::too_many_arguments)]
    pub fn from_sliders(
        profile: &NoiseProfile,
        strength: f32,
        luminance: f32,   // 0..100
        chrominance: f32, // 0..100
        detail: f32,      // 0..100
        impulse: f32,     // 0..100
        luma_curve_mul: &[f32; 6],
        chroma_curve_mul: &[f32; 6],
        engine_nlm: bool,
        nlm_patch: i32,
        nlm_search: i32,
        nlm_center: f32, // 0..100
        aggressive: bool,
        sigma_scale: f32,
    ) -> Self {
        let l = (luminance / 100.0).clamp(0.0, 1.0);
        let c = (chrominance / 100.0).clamp(0.0, 1.0);
        let mut luma_s = [0f32; 6];
        let mut chroma_s = [0f32; 6];
        for j in 0..6 {
            luma_s[j] = LUMA_MAX_S * l * LUMA_CURVE[j] * luma_curve_mul[j].max(0.0);
            chroma_s[j] = CHROMA_MAX_S * c * CHROMA_CURVE[j] * chroma_curve_mul[j].max(0.0);
        }
        Self {
            profile: profile.scaled(strength),
            luma_s,
            chroma_s,
            luma_levels: LUMA_LEVELS,
            chroma_levels: CHROMA_LEVELS,
            detail: (detail / 100.0).clamp(0.0, 1.0),
            impulse_k: if impulse <= 0.0 {
                0.0
            } else {
                // slider 0..100 → threshold 6σ (timid) down to 1.5σ (eager)
                6.0 - 4.5 * (impulse / 100.0).clamp(0.0, 1.0)
            },
            texture_k: if aggressive { 1.5 } else { 3.0 },
            use_nlm: engine_nlm,
            nlm_patch: nlm_patch.clamp(1, 3),
            nlm_search: nlm_search.clamp(2, 10),
            nlm_h: 0.6 + 1.2 * l,
            nlm_center: 1.0 - 0.85 * (nlm_center / 100.0).clamp(0.0, 1.0),
            sigma_scale: sigma_scale.clamp(0.05, 1.0),
        }
    }

    pub fn is_identity(&self) -> bool {
        self.luma_s.iter().all(|&s| s == 0.0)
            && self.chroma_s.iter().all(|&s| s == 0.0)
            && self.impulse_k == 0.0
    }
}

// ---------------------------------------------------------------------------
// Color rotation
// ---------------------------------------------------------------------------

#[inline]
pub fn rgb_to_y0u0v0(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    ((r + g + b) / 3.0, (r - b) * 0.5, (r - 2.0 * g + b) * 0.25)
}

#[inline]
pub fn y0u0v0_to_rgb(y: f32, u: f32, v: f32) -> (f32, f32, f32) {
    let g = y - (4.0 / 3.0) * v;
    let r = y + (2.0 / 3.0) * v + u;
    let b = y + (2.0 / 3.0) * v - u;
    (r, g, b)
}

// ---------------------------------------------------------------------------
// Stage implementations (single channel planes / interleaved trios)
// ---------------------------------------------------------------------------

#[inline]
fn at(buf: &[f32], w: usize, h: usize, x: i32, y: i32, c: usize, nc: usize) -> f32 {
    // clamp-to-edge, matching WGSL textureLoad clamping in the shaders
    let xc = x.clamp(0, w as i32 - 1) as usize;
    let yc = y.clamp(0, h as i32 - 1) as usize;
    buf[(yc * w + xc) * nc + c]
}

/// Conditional 3×3 median (per channel): replace only where the center is
/// an outlier vs the neighborhood median by > k·σ(level). Plain medians
/// blur; the condition is what preserves texture (doc 04 §P2).
pub fn impulse_median(rgb: &[f32], w: usize, h: usize, p: &NoiseProfile, k: f32) -> Vec<f32> {
    if k <= 0.0 {
        return rgb.to_vec();
    }
    let mut out = rgb.to_vec();
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            for c in 0..3 {
                let mut v = [0f32; 9];
                let mut i = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        v[i] = at(rgb, w, h, x + dx, y + dy, c, 3);
                        i += 1;
                    }
                }
                v.sort_by(f32::total_cmp);
                let med = v[4];
                let center = rgb[(y as usize * w + x as usize) * 3 + c];
                if (center - med).abs() > k * p.sigma_at(med).max(1e-6) {
                    out[(y as usize * w + x as usize) * 3 + c] = med;
                }
            }
        }
    }
    out
}

/// One à-trous B3 smoothing level: separable [1,4,6,4,1]/16 with taps
/// spaced 2^level apart, clamp-to-edge. Interleaved `nc` channels.
pub fn atrous_smooth(src: &[f32], w: usize, h: usize, nc: usize, level: usize) -> Vec<f32> {
    const K: [f32; 5] = [1.0 / 16.0, 4.0 / 16.0, 6.0 / 16.0, 4.0 / 16.0, 1.0 / 16.0];
    let step = 1i32 << level;
    let mut tmp = vec![0f32; src.len()];
    // horizontal
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            for c in 0..nc {
                let mut acc = 0f32;
                for (i, k) in K.iter().enumerate() {
                    acc += k * at(src, w, h, x + (i as i32 - 2) * step, y, c, nc);
                }
                tmp[(y as usize * w + x as usize) * nc + c] = acc;
            }
        }
    }
    // vertical
    let mut out = vec![0f32; src.len()];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            for c in 0..nc {
                let mut acc = 0f32;
                for (i, k) in K.iter().enumerate() {
                    acc += k * at(&tmp, w, h, x, y + (i as i32 - 2) * step, c, nc);
                }
                out[(y as usize * w + x as usize) * nc + c] = acc;
            }
        }
    }
    out
}

/// Wiener-style shrinkage gain for a detail coefficient.
#[inline]
pub fn wiener_gain(d: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 1.0;
    }
    let d2 = d * d;
    (d2 - t * t).max(0.0) / (d2 + 1e-9)
}

/// Full à-trous shrink on a 3-channel Y0U0V0 buffer. Per level j:
/// detail_j = smooth_j − smooth_{j+1}, Wiener-shrunk with threshold
/// s_ch[j]·σ_ch·ATROUS_SIGMA[j]·sigma_scale, accumulated; output =
/// deepest smooth + Σ shrunk details. Perfect reconstruction at s = 0.
pub fn atrous_denoise(yuv: &[f32], w: usize, h: usize, p: &ChainParams) -> Vec<f32> {
    let levels = p.luma_levels.max(p.chroma_levels).min(6);
    let ch_sigma = [SIGMA_Y0, SIGMA_U0, SIGMA_V0];
    let mut accum = vec![0f32; yuv.len()];
    let mut smooth = yuv.to_vec();
    for j in 0..levels {
        let next = atrous_smooth(&smooth, w, h, 3, j);
        for i in 0..yuv.len() {
            let c = i % 3;
            let d = smooth[i] - next[i];
            let s = if c == 0 {
                if j < p.luma_levels { p.luma_s[j] } else { 0.0 }
            } else if j < p.chroma_levels {
                p.chroma_s[j]
            } else {
                0.0
            };
            let t = s * ch_sigma[c] * ATROUS_SIGMA[j] * p.sigma_scale;
            accum[i] += d * wiener_gain(d, t);
        }
        smooth = next;
    }
    let mut out = smooth;
    for i in 0..out.len() {
        out[i] += accum[i];
    }
    out
}

/// Non-local means on the Y0 plane only (stabilized units). Noise-compensated
/// patch distance, scattered sampling beyond radius 3, central-pixel weight.
pub fn nlm_luma(yuv: &[f32], w: usize, h: usize, p: &ChainParams) -> Vec<f32> {
    let sigma = SIGMA_Y0 * p.sigma_scale;
    let pr = p.nlm_patch;
    let sr = p.nlm_search;
    let patch_n = ((2 * pr + 1) * (2 * pr + 1)) as f32;
    let h2 = (p.nlm_h * sigma) * (p.nlm_h * sigma) * patch_n;
    let mut out = yuv.to_vec();
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut acc = 0f32;
            let mut wsum = 0f32;
            for qy in -sr..=sr {
                for qx in -sr..=sr {
                    if qx == 0 && qy == 0 {
                        continue;
                    }
                    // scattering: skip 3 of 4 samples beyond radius 3
                    let far = qx.abs().max(qy.abs()) > 3;
                    if far && ((qx & 1) != 0 || (qy & 1) != 0) {
                        continue;
                    }
                    let mut dist = 0f32;
                    for py in -pr..=pr {
                        for px in -pr..=pr {
                            let a = at(yuv, w, h, x + px, y + py, 0, 3);
                            let b = at(yuv, w, h, x + qx + px, y + qy + py, 0, 3);
                            dist += (a - b) * (a - b);
                        }
                    }
                    // subtract the noise's own expected distance
                    let d = (dist - 2.0 * sigma * sigma * patch_n).max(0.0);
                    let wgt = (-d / h2).exp() * if far { 4.0 } else { 1.0 };
                    acc += wgt * at(yuv, w, h, x + qx, y + qy, 0, 3);
                    wsum += wgt;
                }
            }
            let center = yuv[(y as usize * w + x as usize) * 3];
            let wc = p.nlm_center * wsum.max(1e-6);
            out[(y as usize * w + x as usize) * 3] =
                (acc + wc * center) / (wsum + wc).max(1e-6);
        }
    }
    out
}

/// Detail recovery + texture-mask blend, in stabilized Y0U0V0 space.
/// Recovery: gated residual add-back on Y0 (large residuals = structure the
/// shrinkage ate; small ones = noise). Texture mask: local variance vs the
/// noise floor → textured regions keep up to 50% original (doc 02 §7.2).
/// Pinned deviation from doc 04 §P3.d: the block-DCT residual pass is
/// replaced by this σ-gated add-back — same slider semantics, 1 pass.
pub fn recover_and_blend(
    orig: &[f32],
    den: &[f32],
    w: usize,
    h: usize,
    p: &ChainParams,
) -> Vec<f32> {
    let sigma_y = SIGMA_Y0 * p.sigma_scale;
    let mut out = den.to_vec();
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let i = (y as usize * w + x as usize) * 3;
            // -- luma detail recovery --
            if p.detail > 0.0 {
                let r = orig[i] - den[i];
                // smoothstep gate on |r|/σ between 1.5 and 4
                let t = ((r.abs() / sigma_y.max(1e-6) - 1.5) / 2.5).clamp(0.0, 1.0);
                let gate = t * t * (3.0 - 2.0 * t);
                out[i] = den[i] + p.detail * r * gate;
            }
            // -- texture mask (5×5 local variance of orig Y0) --
            let (mut s, mut s2) = (0f32, 0f32);
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let v = at(orig, w, h, x + dx, y + dy, 0, 3);
                    s += v;
                    s2 += v * v;
                }
            }
            let m = s / 25.0;
            let var = (s2 / 25.0 - m * m).max(0.0);
            let noise_var = sigma_y * sigma_y;
            let t = ((var - noise_var) / (p.texture_k * noise_var).max(1e-9)).clamp(0.0, 1.0);
            for c in 0..3 {
                let d = out[i + c];
                out[i + c] = d + t * 0.5 * (orig[i + c] - d);
            }
        }
    }
    out
}

/// The whole classical chain on an interleaved linear RGB buffer.
pub fn denoise_rgb(rgb: &[f32], w: usize, h: usize, p: &ChainParams) -> Vec<f32> {
    if p.is_identity() {
        return rgb.to_vec();
    }
    let pre = impulse_median(rgb, w, h, &p.profile, p.impulse_k);
    // VST per RGB channel, then rotate
    let mut yuv = vec![0f32; pre.len()];
    for i in 0..(w * h) {
        let r = vst_forward(pre[i * 3], &p.profile);
        let g = vst_forward(pre[i * 3 + 1], &p.profile);
        let b = vst_forward(pre[i * 3 + 2], &p.profile);
        let (y, u, v) = rgb_to_y0u0v0(r, g, b);
        yuv[i * 3] = y;
        yuv[i * 3 + 1] = u;
        yuv[i * 3 + 2] = v;
    }
    let mut den = atrous_denoise(&yuv, w, h, p);
    if p.use_nlm {
        let nlm = nlm_luma(&yuv, w, h, p);
        for i in 0..(w * h) {
            den[i * 3] = nlm[i * 3];
        }
    }
    let fin = recover_and_blend(&yuv, &den, w, h, p);
    // rotate back + unbiased inverse
    let mut out = vec![0f32; rgb.len()];
    for i in 0..(w * h) {
        let (r, g, b) = y0u0v0_to_rgb(fin[i * 3], fin[i * 3 + 1], fin[i * 3 + 2]);
        out[i * 3] = vst_inverse_unbiased(r, &p.profile);
        out[i * 3 + 1] = vst_inverse_unbiased(g, &p.profile);
        out[i * 3 + 2] = vst_inverse_unbiased(b, &p.profile);
    }
    out
}

// ---------------------------------------------------------------------------
// P0 — hot/dead pixel suppression (raw mosaic domain)
// ---------------------------------------------------------------------------

/// Suppress defective sites on a Bayer mosaic in place: compare each pixel
/// against the median of its 4 same-CFA-color neighbors (±2); replace when
/// it sticks out beyond t·max(local spread, ε·white). Returns replaced count.
pub fn hot_pixel_suppress(
    mosaic: &mut [f32],
    w: usize,
    h: usize,
    t: f32,
    white: f32,
) -> usize {
    let eps_floor = 0.002 * white;
    let mut fixed = 0usize;
    let orig = mosaic.to_vec();
    let get = |x: i32, y: i32| -> f32 {
        let xc = x.clamp(0, w as i32 - 1) as usize;
        let yc = y.clamp(0, h as i32 - 1) as usize;
        orig[yc * w + xc]
    };
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let n = [
                get(x - 2, y),
                get(x + 2, y),
                get(x, y - 2),
                get(x, y + 2),
            ];
            let mut s = n;
            s.sort_by(f32::total_cmp);
            let med = (s[1] + s[2]) * 0.5;
            let spread = s[3] - s[0];
            let v = orig[y as usize * w + x as usize];
            if (v - med).abs() > t * spread.max(eps_floor) {
                mosaic[y as usize * w + x as usize] = med;
                fixed += 1;
            }
        }
    }
    fixed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::denoise::metrics::{add_poisson_gaussian, add_salt_pepper, psnr, Rng};

    fn test_params(profile: &NoiseProfile, lum: f32, chrom: f32) -> ChainParams {
        ChainParams::from_sliders(
            profile,
            1.0,
            lum,
            chrom,
            50.0,
            0.0,
            &[1.0; 6],
            &[1.0; 6],
            false,
            1,
            5,
            30.0,
            false,
            1.0,
        )
    }

    /// Structured but denoisable test scene: smooth gradients + a few edges.
    fn scene(w: usize, h: usize) -> Vec<f32> {
        let mut img = vec![0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let fx = x as f32 / w as f32;
                let fy = y as f32 / h as f32;
                let edge = if x > w / 2 { 0.25 } else { 0.0 };
                let v = 0.08 + 0.5 * fx * fy + edge;
                img[(y * w + x) * 3] = v;
                img[(y * w + x) * 3 + 1] = v * 0.9;
                img[(y * w + x) * 3 + 2] = v * 1.1;
            }
        }
        img
    }

    #[test]
    fn y0u0v0_rotation_is_exact_inverse() {
        let mut rng = Rng::new(3);
        for _ in 0..1000 {
            let (r, g, b) = (rng.next_f32() * 2.0, rng.next_f32() * 2.0, rng.next_f32() * 2.0);
            let (y, u, v) = rgb_to_y0u0v0(r, g, b);
            let (r2, g2, b2) = y0u0v0_to_rgb(y, u, v);
            assert!((r - r2).abs() < 1e-5 && (g - g2).abs() < 1e-5 && (b - b2).abs() < 1e-5);
        }
    }

    #[test]
    fn atrous_sigma_constants_match_monte_carlo() {
        // White noise through the decompose chain: per-level detail σ must
        // match ATROUS_SIGMA (doc 04 says verify numerically — this is it).
        let (w, h) = (128, 128);
        let mut rng = Rng::new(17);
        let noise: Vec<f32> = (0..w * h).map(|_| rng.next_gauss()).collect();
        let mut smooth = noise.clone();
        for (j, expect) in ATROUS_SIGMA.iter().enumerate().take(5) {
            let next = atrous_smooth(&smooth, w, h, 1, j);
            // interior only — edge clamping skews the tails
            let mut var = 0f64;
            let mut n = 0u64;
            let m = 40;
            for y in m..h - m {
                for x in m..w - m {
                    let d = smooth[y * w + x] - next[y * w + x];
                    var += (d as f64) * (d as f64);
                    n += 1;
                }
            }
            let got = (var / n as f64).sqrt() as f32;
            assert!(
                (got - expect).abs() / expect < 0.20,
                "level {j}: measured σ {got}, table {expect}"
            );
            smooth = next;
        }
    }

    #[test]
    fn atrous_perfect_reconstruction_at_zero_strength() {
        let (w, h) = (64, 48);
        let img = scene(w, h);
        let mut yuv = vec![0f32; img.len()];
        for i in 0..w * h {
            let (y, u, v) = rgb_to_y0u0v0(img[i * 3], img[i * 3 + 1], img[i * 3 + 2]);
            yuv[i * 3] = y;
            yuv[i * 3 + 1] = u;
            yuv[i * 3 + 2] = v;
        }
        let p = test_params(&NoiseProfile::default(), 0.0, 0.0);
        let out = atrous_denoise(&yuv, w, h, &p);
        for i in 0..yuv.len() {
            assert!(
                (out[i] - yuv[i]).abs() < 1e-4,
                "reconstruction drift at {i}: {} vs {}",
                out[i],
                yuv[i]
            );
        }
    }

    #[test]
    fn full_chain_is_identity_when_off() {
        let (w, h) = (48, 32);
        let img = scene(w, h);
        let p = test_params(&NoiseProfile::from_iso(3200), 0.0, 0.0);
        assert!(p.is_identity());
        let out = denoise_rgb(&img, w, h, &p);
        assert_eq!(out, img);
    }

    #[test]
    fn wavelet_chain_improves_psnr_on_synthetic_noise() {
        let (w, h) = (96, 96);
        let clean = scene(w, h);
        let profile = NoiseProfile::from_iso(12800);
        let noisy = add_poisson_gaussian(&clean, &profile, 23);
        let p = test_params(&profile, 60.0, 60.0);
        let den = denoise_rgb(&noisy, w, h, &p);
        let before = psnr(&clean, &noisy);
        let after = psnr(&clean, &den);
        assert!(
            after > before + 3.0,
            "wavelet should gain ≥3 dB: {before:.2} → {after:.2}"
        );
    }

    #[test]
    fn nlm_improves_psnr_on_synthetic_noise() {
        let (w, h) = (64, 64);
        let clean = scene(w, h);
        let profile = NoiseProfile::from_iso(12800);
        let noisy = add_poisson_gaussian(&clean, &profile, 29);
        let mut p = test_params(&profile, 70.0, 50.0);
        p.use_nlm = true;
        let den = denoise_rgb(&noisy, w, h, &p);
        let before = psnr(&clean, &noisy);
        let after = psnr(&clean, &den);
        assert!(
            after > before + 3.0,
            "nlm should gain ≥3 dB: {before:.2} → {after:.2}"
        );
    }

    #[test]
    fn impulse_median_kills_salt_pepper() {
        let (w, h) = (96, 96);
        let clean = scene(w, h);
        let noisy = add_salt_pepper(&clean, 0.005, 31);
        let profile = NoiseProfile::from_iso(400);
        let den = impulse_median(&noisy, w, h, &profile, 3.0);
        let before = psnr(&clean, &noisy);
        let after = psnr(&clean, &den);
        assert!(
            after > before + 10.0,
            "impulse median should gain >10 dB at 0.5% density: {before:.2} → {after:.2}"
        );
        // and must NOT soften a clean image
        let clean_out = impulse_median(&clean, w, h, &profile, 3.0);
        let s = crate::denoise::metrics::ssim_rgb(&clean, &clean_out, w, h);
        assert!(s > 0.999, "clean image ssim after impulse: {s}");
    }

    #[test]
    fn hot_pixel_removal_on_mosaic() {
        let (w, h) = (128, 128);
        // flat gray mosaic with mild noise
        let mut rng = Rng::new(41);
        let mut mosaic: Vec<f32> =
            (0..w * h).map(|_| 0.3 + 0.005 * rng.next_gauss()).collect();
        // inject 200 hot + 50 dead pixels
        let mut hot = std::collections::HashSet::new();
        for _ in 0..200 {
            let i = (rng.next_f32() * (w * h) as f32) as usize % (w * h);
            mosaic[i] = 0.98;
            hot.insert(i);
        }
        for _ in 0..50 {
            let i = (rng.next_f32() * (w * h) as f32) as usize % (w * h);
            mosaic[i] = 0.0;
            hot.insert(i);
        }
        let fixed = hot_pixel_suppress(&mut mosaic, w, h, 3.5, 1.0);
        let remaining = hot
            .iter()
            .filter(|&&i| (mosaic[i] - 0.3).abs() > 0.1)
            .count();
        // Clustered defects can shield each other via the same-CFA median;
        // require ≥90% clearance (doc 04: ≥95% on well-spaced dark-frame injects).
        assert!(
            remaining <= hot.len() / 10,
            "≥90% of injected defects must go: {remaining}/{} remain (fixed {fixed})",
            hot.len()
        );
        // clean-image guard: a smooth mosaic must be (almost) untouched
        let mut clean: Vec<f32> = (0..w * h)
            .map(|i| 0.2 + 0.4 * ((i % w) as f32 / w as f32))
            .collect();
        let before = clean.clone();
        let n = hot_pixel_suppress(&mut clean, w, h, 4.0, 1.0);
        assert_eq!(clean, before, "smooth mosaic modified ({n} px)");
    }
}
