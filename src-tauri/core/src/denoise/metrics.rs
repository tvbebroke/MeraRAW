//! Ground-truth metrics (PSNR / SSIM) and the synthetic Poisson–Gaussian
//! noise injector — shared by unit tests and the `denoise_bench` harness.

use super::profile::NoiseProfile;

/// Deterministic xorshift RNG — no rand dep, reproducible across platforms.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1).wrapping_mul(0x9E3779B97F4A7C15) | 1)
    }
    pub fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 11) as f32) / ((1u64 << 53) as f32)
    }
    /// Standard normal via Box–Muller.
    pub fn next_gauss(&mut self) -> f32 {
        let u1 = self.next_f32().max(1e-12);
        let u2 = self.next_f32();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
    }
}

/// Inject signal-dependent noise: z = x + N(0, sqrt(a·x + b)). Gaussian
/// approximation of the Poisson part — accurate for the photon counts real
/// sensors see, and exactly the model the estimator assumes.
pub fn add_poisson_gaussian(clean: &[f32], p: &NoiseProfile, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    clean
        .iter()
        .map(|&x| (x + rng.next_gauss() * p.sigma_at(x)).max(0.0))
        .collect()
}

/// Salt & pepper impulse noise at `density` (fraction of pixels hit).
pub fn add_salt_pepper(clean: &[f32], density: f32, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    clean
        .iter()
        .map(|&x| {
            if rng.next_f32() < density {
                if rng.next_f32() < 0.5 {
                    0.0
                } else {
                    1.0
                }
            } else {
                x
            }
        })
        .collect()
}

/// PSNR in dB against a peak of 1.0 (linear working range).
pub fn psnr(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    let mse = a
        .iter()
        .zip(b)
        .map(|(x, y)| {
            let d = (x - y) as f64;
            d * d
        })
        .sum::<f64>()
        / a.len() as f64;
    if mse <= 1e-20 {
        return 99.0;
    }
    (10.0 * (1.0 / mse).log10()) as f32
}

/// Single-channel SSIM (8×8 windows, stride 4). `w`/`h` in pixels, data is
/// one channel. Standard constants for L = 1.0.
pub fn ssim(a: &[f32], b: &[f32], w: usize, h: usize) -> f32 {
    assert_eq!(a.len(), w * h);
    assert_eq!(b.len(), w * h);
    const WIN: usize = 8;
    const C1: f64 = 0.01 * 0.01;
    const C2: f64 = 0.03 * 0.03;
    if w < WIN || h < WIN {
        return 1.0;
    }
    let mut total = 0f64;
    let mut count = 0u64;
    let mut y = 0;
    while y + WIN <= h {
        let mut x = 0;
        while x + WIN <= w {
            let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0f64, 0f64, 0f64, 0f64, 0f64);
            for dy in 0..WIN {
                for dx in 0..WIN {
                    let i = (y + dy) * w + x + dx;
                    let (va, vb) = (a[i] as f64, b[i] as f64);
                    sa += va;
                    sb += vb;
                    saa += va * va;
                    sbb += vb * vb;
                    sab += va * vb;
                }
            }
            let n = (WIN * WIN) as f64;
            let (ma, mb) = (sa / n, sb / n);
            let va = (saa / n - ma * ma).max(0.0);
            let vb = (sbb / n - mb * mb).max(0.0);
            let cov = sab / n - ma * mb;
            let s = ((2.0 * ma * mb + C1) * (2.0 * cov + C2))
                / ((ma * ma + mb * mb + C1) * (va + vb + C2));
            total += s;
            count += 1;
            x += 4;
        }
        y += 4;
    }
    (total / count.max(1) as f64) as f32
}

/// SSIM averaged over the 3 channels of interleaved RGB.
pub fn ssim_rgb(a: &[f32], b: &[f32], w: usize, h: usize) -> f32 {
    let mut acc = 0f32;
    for c in 0..3 {
        let ca: Vec<f32> = a.iter().skip(c).step_by(3).copied().collect();
        let cb: Vec<f32> = b.iter().skip(c).step_by(3).copied().collect();
        acc += ssim(&ca, &cb, w, h);
    }
    acc / 3.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psnr_and_ssim_sanity() {
        let a = vec![0.5f32; 64 * 64];
        assert_eq!(psnr(&a, &a), 99.0);
        assert!((ssim(&a, &a, 64, 64) - 1.0).abs() < 1e-6);

        let p = NoiseProfile::from_iso(6400);
        let noisy = add_poisson_gaussian(&a, &p, 3);
        let pn = psnr(&a, &noisy);
        assert!(pn > 15.0 && pn < 60.0, "noisy psnr {pn}");
        assert!(ssim(&a, &noisy, 64, 64) < 0.999);
    }

    #[test]
    fn injector_matches_model_variance() {
        let p = NoiseProfile {
            a: 1e-3,
            b: 1e-6,
            source: super::super::profile::ProfileSource::Default,
        };
        let clean = vec![0.25f32; 50000];
        let noisy = add_poisson_gaussian(&clean, &p, 9);
        let m = noisy.iter().sum::<f32>() / noisy.len() as f32;
        let var = noisy.iter().map(|v| (v - m) * (v - m)).sum::<f32>() / noisy.len() as f32;
        let expect = p.a * 0.25 + p.b;
        assert!(
            (var - expect).abs() / expect < 0.05,
            "var {var} vs model {expect}"
        );
    }
}
