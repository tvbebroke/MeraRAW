//! `validate_rt` — certify the RawTherapee backend's `Mode::Native` contract.
//!
//! Question: is zerawler-RT native output *camera-native linear RGB, unity WB*
//! — the same colour state merawler produces (rawler rescale → demosaic)?
//!
//! Method: decode the SAME raw both ways (both RCD, so demosaic differences
//! are second-order), un-rotate the RT output back to sensor orientation,
//! align by normalized cross-correlation, then least-squares fit an affine
//! 3×4 transform RT→merawler over low-gradient pixels, for several transfer-
//! curve candidates. The winning fit tells the story:
//!   * M ≈ s·I, tiny residual  → same colour space up to scale. VALIDATED.
//!   * M diagonal, non-uniform → per-channel scale (levels/WB mismatch).
//!   * M with off-diagonals    → a colour matrix is baked in. NOT native.
//!
//! Usage: validate_rt <raw>

use std::path::Path;
use std::process::ExitCode;

use zerawler::{Algorithm, Engine, Mode, RgbImage};

fn main() -> ExitCode {
    let arg = std::env::args().nth(1).unwrap_or_default();
    if arg.is_empty() {
        eprintln!("usage: validate_rt <raw-file> [algo]");
        return ExitCode::FAILURE;
    }
    let algo = std::env::args()
        .nth(2)
        .and_then(|s| Algorithm::from_name(&s))
        .unwrap_or(Algorithm::Rcd);
    let raw = Path::new(&arg);

    // --- ground truth: merawler camera-native (unity WB, sensor orientation)
    eprintln!("[1/4] merawler RCD decode (ground truth)…");
    let cfa = match merawler::rawler_adapter::cfa_from_path(raw) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("merawler decode failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    let truth = merawler::demosaic(&cfa, merawler::Algorithm::Rcd).expect("rcd");
    eprintln!("      truth: {}x{} (sensor orientation)", truth.width, truth.height);

    // --- candidate: zerawler native decode (levels forced to rawler's so the
    // normalization matches the ground truth exactly)
    eprintln!("[2/4] zerawler native {} decode…", algo.name());
    let engine = Engine::detect();
    let opts = rawler_levels(raw);
    eprintln!("      forcing levels: black={:?} white={:?}", opts.black, opts.white);
    let dec = match engine.decode_opts(raw, algo, Mode::Native, opts) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("zerawler decode failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    let zimg = dec.image;
    eprintln!(
        "      rt-native: {}x{} in {:.2?} (min/mean/max G: {})",
        zimg.width,
        zimg.height,
        dec.elapsed,
        chan_stats(&zimg, 1)
    );

    // --- orientation + alignment
    eprintln!("[3/4] aligning (rotation + offset search)…");
    let candidates = if zimg.width != truth.width {
        vec![("rot90cw", rot90cw(&zimg)), ("rot90ccw", rot90ccw(&zimg))]
    } else {
        vec![("as-is", zimg.clone())]
    };
    let mut best: Option<(String, RgbImage, usize, usize, f64)> = None;
    for (name, cand) in candidates {
        if cand.width > truth.width || cand.height > truth.height {
            continue;
        }
        let (dx, dy, ncc) = align(&truth, &cand);
        eprintln!("      {name}: offset=({dx},{dy}) ncc={ncc:.5}");
        if best.as_ref().map(|b| ncc > b.4).unwrap_or(true) {
            best = Some((name.to_string(), cand, dx, dy, ncc));
        }
    }
    let Some((rot, z, dx, dy, ncc)) = best else {
        eprintln!("no orientation candidate fits inside truth image");
        return ExitCode::FAILURE;
    };
    if ncc < 0.9 {
        eprintln!("ALIGNMENT FAILED (ncc {ncc:.3} < 0.9) — cannot validate");
        return ExitCode::FAILURE;
    }
    eprintln!("      using {rot}, offset ({dx},{dy}), ncc {ncc:.5}");

    // --- affine fit per transfer-curve candidate (samples gated once)
    eprintln!("[4/4] fitting affine 3x4 per TRC candidate…");
    let samples = collect_samples(&truth, &z, dx, dy);
    eprintln!("      {} low-gradient, non-clipped samples", samples.len());
    let trcs: [(&str, fn(f32) -> f32); 4] = [
        ("linear", |v| v),
        ("srgb-decode", srgb_decode),
        ("g2.2-decode", |v| v.max(0.0).powf(2.2)),
        ("g1.8-decode", |v| v.max(0.0).powf(1.8)),
    ];
    let mut results = Vec::new();
    for (name, f) in trcs {
        let fit = fit_affine(&samples, f);
        eprintln!("      {name:<12} residual {:.3}%", fit.residual * 100.0);
        results.push((name, fit));
    }
    results.sort_by(|a, b| a.1.residual.partial_cmp(&b.1.residual).unwrap());
    let (trc, fit) = &results[0];

    println!("\n=== RT NATIVE VALIDATION ===");
    println!("rotation: {rot}   offset: ({dx},{dy})   alignment ncc: {ncc:.5}");
    println!("best transfer curve: {trc}   relative residual: {:.3}%", fit.residual * 100.0);
    println!("affine matrix (rows map RT [r,g,b,1] -> merawler channel):");
    for r in 0..3 {
        println!(
            "  [{:+.4} {:+.4} {:+.4} | {:+.5}]",
            fit.m[r][0], fit.m[r][1], fit.m[r][2], fit.m[r][3]
        );
    }
    let diag = [fit.m[0][0], fit.m[1][1], fit.m[2][2]];
    let max_off = (0..3)
        .flat_map(|r| (0..3).map(move |c| (r, c)))
        .filter(|(r, c)| r != c)
        .map(|(r, c)| fit.m[r][c].abs())
        .fold(0.0f64, f64::max);
    let mean_diag = (diag[0] + diag[1] + diag[2]) / 3.0;
    let diag_spread = diag
        .iter()
        .map(|d| (d - mean_diag).abs() / mean_diag)
        .fold(0.0f64, f64::max);
    let off_ratio = max_off / mean_diag;
    println!(
        "diag: [{:.4} {:.4} {:.4}]  spread {:.2}%  max off-diag/diag {:.2}%",
        diag[0], diag[1], diag[2], diag_spread * 100.0, off_ratio * 100.0
    );

    // Interpretation guide (ground truth = camera-native unity-WB linear):
    //  * off-diag ≈ 0 & diag uniform  → output IS camera-native (up to scale).
    //  * off-diag ≈ 0 & diag varies   → camera-native up to per-channel (WB) scale.
    //  * off-diag large, residual low → a pure linear colour transform is baked
    //    in (e.g. WB + camera→Rec2020) — fine IF that transform is the declared
    //    contract; the matrix above IS that transform's inverse-composition.
    //  * residual high at every TRC   → nonlinear processing leaked in: unusable.
    if off_ratio < 0.02 && diag_spread < 0.02 && fit.residual < 0.03 {
        println!(
            "\nVERDICT: CAMERA-NATIVE — TRC={trc}, uniform scale {:.4} vs merawler.",
            mean_diag
        );
        ExitCode::SUCCESS
    } else if off_ratio < 0.02 && fit.residual < 0.03 {
        println!(
            "\nVERDICT: CAMERA-NATIVE + PER-CHANNEL SCALE — TRC={trc}, \
             r={:.4} g={:.4} b={:.4} (WB-like diagonal only).",
            diag[0], diag[1], diag[2]
        );
        ExitCode::SUCCESS
    } else if fit.residual < 0.06 {
        println!(
            "\nVERDICT: LINEARLY COLOUR-MANAGED — TRC={trc}, a pure linear transform \
             (matrix above) separates this output from camera-native. Consistent with \
             a declared managed contract (e.g. WB + camera→working matrix); \
             residual {:.2}% says no nonlinear processing leaked in.",
            fit.residual * 100.0
        );
        ExitCode::SUCCESS
    } else {
        println!(
            "\nVERDICT: UNEXPLAINED — residual {:.2}% at best TRC ({trc}): relationship \
             is not a clean linear transform. Do not use for pipeline work.",
            fit.residual * 100.0
        );
        ExitCode::FAILURE
    }
}

// --- helpers ---------------------------------------------------------------

/// Read rawler's black/white levels for `raw` (metadata-only decode).
fn rawler_levels(raw: &Path) -> zerawler::DecodeOpts {
    use rawler::decoders::RawDecodeParams;
    use rawler::rawsource::RawSource;
    let mut opts = zerawler::DecodeOpts::default();
    let Ok(source) = RawSource::new(raw) else { return opts };
    let loader = rawler::RawLoader::new();
    let Ok(decoder) = loader.get_decoder(&source) else { return opts };
    let Ok(img) = decoder.raw_image(&source, &RawDecodeParams::default(), true) else {
        return opts;
    };
    let blacks: Vec<f32> = img.blacklevel.levels.iter().map(|r| r.as_f32()).collect();
    opts = zerawler::DecodeOpts::from_levels(&blacks, img.whitelevel.0.first().copied());
    opts
}

fn chan_stats(img: &RgbImage, c: usize) -> String {
    let (mut lo, mut hi, mut sum) = (f32::INFINITY, f32::NEG_INFINITY, 0.0f64);
    for p in &img.data {
        lo = lo.min(p[c]);
        hi = hi.max(p[c]);
        sum += p[c] as f64;
    }
    format!("{:.4}/{:.4}/{:.4}", lo, sum / img.data.len() as f64, hi)
}

fn rot90cw(src: &RgbImage) -> RgbImage {
    let (ws, hs) = (src.width, src.height);
    let (wd, hd) = (hs, ws);
    let mut data = vec![[0.0f32; 3]; wd * hd];
    for y in 0..hd {
        for x in 0..wd {
            data[y * wd + x] = src.data[(hs - 1 - x) * ws + y];
        }
    }
    RgbImage { width: wd, height: hd, data }
}

fn rot90ccw(src: &RgbImage) -> RgbImage {
    let (ws, hs) = (src.width, src.height);
    let (wd, hd) = (hs, ws);
    let mut data = vec![[0.0f32; 3]; wd * hd];
    for y in 0..hd {
        for x in 0..wd {
            data[y * wd + x] = src.data[x * ws + (ws - 1 - y)];
        }
    }
    RgbImage { width: wd, height: hd, data }
}

/// Best (dx,dy) placing `z`'s origin inside `truth`, by NCC on green over a
/// centered 512x512 patch. Search window covers the crop-difference range.
fn align(truth: &merawler::RgbImage, z: &RgbImage) -> (usize, usize, f64) {
    let max_dx = truth.width - z.width;
    let max_dy = truth.height - z.height;
    let ps = 512usize.min(z.width / 2).min(z.height / 2);
    let zx0 = (z.width - ps) / 2;
    let zy0 = (z.height - ps) / 2;

    // patch from z (green)
    let mut zp = vec![0.0f32; ps * ps];
    for y in 0..ps {
        for x in 0..ps {
            zp[y * ps + x] = z.data[(zy0 + y) * z.width + (zx0 + x)][1];
        }
    }
    let zmean = zp.iter().sum::<f32>() / zp.len() as f32;
    let zvar: f64 = zp.iter().map(|v| ((v - zmean) as f64).powi(2)).sum();

    let (mut bdx, mut bdy, mut bncc) = (0usize, 0usize, f64::NEG_INFINITY);
    for dy in 0..=max_dy {
        for dx in 0..=max_dx {
            let (tx0, ty0) = (dx + zx0, dy + zy0);
            let mut tsum = 0.0f32;
            for y in 0..ps {
                for x in 0..ps {
                    tsum += truth.data[(ty0 + y) * truth.width + (tx0 + x)][1];
                }
            }
            let tmean = tsum / (ps * ps) as f32;
            let (mut cov, mut tvar) = (0.0f64, 0.0f64);
            for y in 0..ps {
                for x in 0..ps {
                    let t = (truth.data[(ty0 + y) * truth.width + (tx0 + x)][1] - tmean) as f64;
                    let zv = (zp[y * ps + x] - zmean) as f64;
                    cov += t * zv;
                    tvar += t * t;
                }
            }
            let ncc = cov / (tvar.sqrt() * zvar.sqrt() + 1e-12);
            if ncc > bncc {
                (bdx, bdy, bncc) = (dx, dy, ncc);
            }
        }
    }
    (bdx, bdy, bncc)
}

struct Fit {
    m: [[f64; 4]; 3],
    residual: f64,
}

/// Gather (candidate, truth) pixel pairs once: strided over the interior,
/// low-gradient on truth green (suppresses demosaic-implementation
/// differences), near-clip excluded (clipping breaks the linear model).
fn collect_samples(
    truth: &merawler::RgbImage,
    z: &RgbImage,
    dx: usize,
    dy: usize,
) -> Vec<([f32; 3], [f32; 3])> {
    let margin = 32usize;
    let stride = 7usize;
    let mut out = Vec::new();
    for zy in (margin..z.height - margin).step_by(stride) {
        for zx in (margin..z.width - margin).step_by(stride) {
            let (tx, ty) = (zx + dx, zy + dy);
            let g = |x: usize, y: usize| truth.data[y * truth.width + x][1];
            let grad = (g(tx + 1, ty) - g(tx - 1, ty)).abs()
                + (g(tx, ty + 1) - g(tx, ty - 1)).abs();
            if grad > 0.02 {
                continue;
            }
            let zp = z.data[zy * z.width + zx];
            let t = truth.data[ty * truth.width + tx];
            if zp.iter().any(|&v| v > 0.95) || t.iter().any(|&v| v > 0.8) {
                continue;
            }
            out.push((zp, t));
        }
    }
    out
}

/// Least-squares affine fit truth ≈ M·[trc(z), 1] over pre-gated samples.
fn fit_affine(samples: &[([f32; 3], [f32; 3])], trc: fn(f32) -> f32) -> Fit {
    let mut ata = [[0.0f64; 4]; 4];
    let mut atb = [[0.0f64; 4]; 3];
    for (zp, t) in samples {
        let x = [trc(zp[0]) as f64, trc(zp[1]) as f64, trc(zp[2]) as f64, 1.0];
        for i in 0..4 {
            for j in 0..4 {
                ata[i][j] += x[i] * x[j];
            }
            for c in 0..3 {
                atb[c][i] += x[i] * t[c] as f64;
            }
        }
    }
    let mut m = [[0.0f64; 4]; 3];
    for c in 0..3 {
        m[c] = solve4(ata, atb[c]);
    }
    let (mut se, mut st) = (0.0f64, 0.0f64);
    for (zp, t) in samples {
        let x = [trc(zp[0]) as f64, trc(zp[1]) as f64, trc(zp[2]) as f64, 1.0];
        for c in 0..3 {
            let pred: f64 = (0..4).map(|i| m[c][i] * x[i]).sum();
            se += (pred - t[c] as f64).powi(2);
            st += (t[c] as f64).powi(2);
        }
    }
    Fit {
        m,
        residual: (se / st.max(1e-12)).sqrt(),
    }
}

/// Gaussian elimination for a 4x4 system.
fn solve4(a: [[f64; 4]; 4], b: [f64; 4]) -> [f64; 4] {
    let mut m = [[0.0f64; 5]; 4];
    for r in 0..4 {
        m[r][..4].copy_from_slice(&a[r]);
        m[r][4] = b[r];
    }
    for col in 0..4 {
        let piv = (col..4).max_by(|&i, &j| m[i][col].abs().partial_cmp(&m[j][col].abs()).unwrap()).unwrap();
        m.swap(col, piv);
        let p = m[col][col];
        if p.abs() < 1e-12 {
            continue;
        }
        for r in 0..4 {
            if r != col {
                let f = m[r][col] / p;
                for k in col..5 {
                    m[r][k] -= f * m[col][k];
                }
            }
        }
    }
    let mut out = [0.0f64; 4];
    for r in 0..4 {
        out[r] = if m[r][r].abs() > 1e-12 { m[r][4] / m[r][r] } else { 0.0 };
    }
    out
}

fn srgb_decode(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
