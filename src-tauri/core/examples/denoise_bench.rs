//! denoise-bench — Phase 0 metrics harness (doc 07).
//! Synthetic Poisson–Gaussian ladder + classical chain → markdown report.
//!
//!   cargo run -p meratech-core --example denoise_bench

use meratech_core::denoise::cpu::{denoise_rgb, ChainParams};
use meratech_core::denoise::metrics::{add_poisson_gaussian, psnr, ssim_rgb};
use meratech_core::denoise::profile::NoiseProfile;
use std::time::Instant;

fn scene(w: usize, h: usize) -> Vec<f32> {
    let mut img = vec![0f32; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let edge = if x > w / 2 { 0.2 } else { 0.0 };
            let v = 0.06 + 0.55 * fx * fy + edge;
            let i = (y * w + x) * 3;
            img[i] = v;
            img[i + 1] = v * 0.92;
            img[i + 2] = v * 1.08;
        }
    }
    img
}

fn main() {
    let (w, h) = (256, 256);
    let clean = scene(w, h);
    let isos = [100u32, 400, 1600, 6400, 12800, 25600];
    println!("# MeraRAW denoise-bench\n");
    println!("Scene: {w}×{h} synthetic gradient+edge. Engine: CPU wavelet.\n");
    println!("| ISO | noisy PSNR | den PSNR | Δ dB | SSIM | ms |");
    println!("|----:|----------:|---------:|-----:|-----:|---:|");
    for iso in isos {
        let profile = NoiseProfile::from_iso(iso);
        let noisy = add_poisson_gaussian(&clean, &profile, 42 + iso as u64);
        let p = ChainParams::from_sliders(
            &profile, 1.0, 55.0, 55.0, 50.0, 0.0, &[1.0; 6], &[1.0; 6], false, 1, 5, 30.0, false,
            1.0,
        );
        let t0 = Instant::now();
        let den = denoise_rgb(&noisy, w, h, &p);
        let ms = t0.elapsed().as_secs_f32() * 1000.0;
        let before = psnr(&clean, &noisy);
        let after = psnr(&clean, &den);
        let s = ssim_rgb(&clean, &den, w, h);
        println!(
            "| {iso} | {before:.2} | {after:.2} | {delta:.2} | {s:.4} | {ms:.1} |",
            delta = after - before
        );
    }
    println!("\nExit: baseline report produced. GPU parity + NIND assets are follow-ups.");
}
