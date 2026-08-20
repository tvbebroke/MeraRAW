//! `bench` — objective demosaic benchmark driven by the merawler engine.
//!
//! There is no ground truth for a real RAW (the scene was never captured in
//! full color), so quality is measured the standard way: take a *known* RGB
//! image, Bayer-mosaic it, demosaic it back, and compare to the original.
//!
//! Metrics (higher PSNR = better):
//!   * PSNR       — overall reconstruction accuracy (dB).
//!   * R/G/B PSNR — per-channel accuracy.
//!   * Chroma PSNR— accuracy of the R-G / B-G color differences; low values
//!                  mean visible *false color*. On a grayscale test image any
//!                  chroma at all is false color, so this is a pure fringing
//!                  score.
//!   * time       — median demosaic time on the test image.
//!
//! Test images:
//!   * `zoneplate` — grayscale radial chirp; frequencies sweep past Nyquist, so
//!     it is the classic aliasing / false-color torture test.
//!   * `chart`     — smooth color gradients + hard color edges + fine detail
//!     lines; tests color fidelity and edge handling.
//!
//! Usage: `bench [--out <dir>] [--size <px>]`  (defaults: /tmp/merawler_bench, 768)

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use merawler::{Algorithm, CfaImage, CfaPattern, RgbImage};

fn main() {
    let mut out = PathBuf::from("/tmp/merawler_bench");
    let mut size = 768usize;
    let mut truth_png: Option<PathBuf> = None;
    let mut dir: Option<PathBuf> = None;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = PathBuf::from(it.next().expect("--out needs a path")),
            "--size" => size = it.next().and_then(|s| s.parse().ok()).expect("--size N"),
            "--truth" => truth_png = it.next().map(PathBuf::from),
            "--dir" => dir = it.next().map(PathBuf::from),
            other => {
                eprintln!("unknown arg {other:?}");
                std::process::exit(1);
            }
        }
    }

    let algos: Vec<Algorithm> = Algorithm::all()
        .iter()
        .copied()
        .filter(|a| a.is_implemented())
        .collect();

    // Directory mode: average PSNR over every image (e.g. the Kodak set) — these
    // are true full-color ground truth, so this is a literature-comparable CPSNR.
    if let Some(d) = &dir {
        run_dir(d, &algos, 16);
        return;
    }

    std::fs::create_dir_all(&out).expect("create out dir");

    // A downsampled real photo (via --truth) is an artifact-free natural ground
    // truth and gives the most representative PSNR ranking; the synthetic images
    // isolate specific failure modes (aliasing, color edges).
    let mut tests: Vec<(&str, RgbImage)> = Vec::new();
    if let Some(p) = &truth_png {
        tests.push(("natural", load_truth(p)));
    }
    tests.push(("zoneplate", zoneplate(size)));
    tests.push(("chart", chart(size)));

    let mut csv = String::from("test,algorithm,psnr,psnr_r,psnr_g,psnr_b,chroma_psnr,time_ms\n");
    // rows[test] -> Vec<(algo, Metrics)>
    let mut all_rows: Vec<(&str, Vec<(Algorithm, Metrics)>)> = Vec::new();

    for (tname, truth) in &tests {
        save_rgb(truth, &out.join(format!("{tname}_truth.png")));
        let cfa = mosaic(truth, CfaPattern::Rggb);
        // also save the raw mosaic as a grayscale visualization
        save_mosaic(&cfa, &out.join(format!("{tname}_mosaic.png")));

        println!("\n=== {tname} ({size}x{size}, RGGB) ===");
        println!(
            "{:<9} {:>8} {:>8} {:>8} {:>8} {:>10} {:>9}",
            "algo", "PSNR", "PSNR-R", "PSNR-G", "PSNR-B", "chromaPSNR", "time(ms)"
        );
        let mut rows = Vec::new();
        for &algo in &algos {
            let d = algo.make().unwrap();
            // median of 3 timed runs
            let mut best = f64::INFINITY;
            let mut result = RgbImage::new(cfa.width, cfa.height);
            for _ in 0..3 {
                let t = Instant::now();
                result = d.demosaic(&cfa);
                best = best.min(t.elapsed().as_secs_f64() * 1000.0);
            }
            let m = metrics(truth, &result, 20);
            save_rgb(&result, &out.join(format!("{tname}_{}.png", algo.name())));
            save_error(
                truth,
                &result,
                &out.join(format!("{tname}_{}_err.png", algo.name())),
            );
            println!(
                "{:<9} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>10.2} {:>9.1}",
                algo.name(),
                m.psnr,
                m.psnr_r,
                m.psnr_g,
                m.psnr_b,
                m.chroma_psnr,
                best
            );
            csv += &format!(
                "{tname},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.2}\n",
                algo.name(),
                m.psnr,
                m.psnr_r,
                m.psnr_g,
                m.psnr_b,
                m.chroma_psnr,
                best
            );
            rows.push((algo, m));
        }
        all_rows.push((tname, rows));
    }

    let csv_path = out.join("results.csv");
    std::fs::File::create(&csv_path)
        .and_then(|mut f| f.write_all(csv.as_bytes()))
        .expect("write csv");
    println!(
        "\nwrote {} and per-algorithm PNGs to {}",
        csv_path.display(),
        out.display()
    );
}

/// Average CPSNR over every PNG in `dir` (full-color ground-truth images such as
/// the Kodak set). Reports mean overall/chroma PSNR (per-image PSNR averaged)
/// and total demosaic time per algorithm.
fn run_dir(dir: &std::path::Path, algos: &[Algorithm], border: usize) {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|e| e == "png").unwrap_or(false))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no PNGs in {}", dir.display());
    println!(
        "Averaging over {} images in {}\n",
        paths.len(),
        dir.display()
    );

    let n = algos.len();
    let mut sum_psnr = vec![0.0f64; n];
    let mut sum_chroma = vec![0.0f64; n];
    let mut sum_ms = vec![0.0f64; n];

    for p in &paths {
        let truth = load_truth(p);
        let cfa = mosaic(&truth, CfaPattern::Rggb);
        for (k, &algo) in algos.iter().enumerate() {
            let d = algo.make().unwrap();
            let t = Instant::now();
            let result = d.demosaic(&cfa);
            sum_ms[k] += t.elapsed().as_secs_f64() * 1000.0;
            let m = metrics(&truth, &result, border);
            sum_psnr[k] += m.psnr;
            sum_chroma[k] += m.chroma_psnr;
        }
    }

    let f = paths.len() as f64;
    println!(
        "{:<9} {:>10} {:>12} {:>10}",
        "algo", "CPSNR", "chromaPSNR", "ms/img"
    );
    // print sorted by CPSNR desc
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| sum_psnr[b].partial_cmp(&sum_psnr[a]).unwrap());
    for &k in &order {
        println!(
            "{:<9} {:>10.2} {:>12.2} {:>10.1}",
            algos[k].name(),
            sum_psnr[k] / f,
            sum_chroma[k] / f,
            sum_ms[k] / f
        );
    }
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

struct Metrics {
    psnr: f64,
    psnr_r: f64,
    psnr_g: f64,
    psnr_b: f64,
    chroma_psnr: f64,
}

fn psnr_from_mse(mse: f64) -> f64 {
    if mse <= 1e-12 {
        99.0
    } else {
        (10.0 * (1.0 / mse).log10()).min(99.0)
    }
}

/// Compare `result` to `truth`, ignoring a `border`-pixel margin (algorithms
/// fall back to bilinear near edges).
fn metrics(truth: &RgbImage, result: &RgbImage, border: usize) -> Metrics {
    let (w, h) = (truth.width, truth.height);
    let (mut se, mut se_r, mut se_g, mut se_b, mut se_c) = (0.0f64, 0.0, 0.0, 0.0, 0.0);
    let mut n = 0u64;
    for y in border..h - border {
        for x in border..w - border {
            let i = y * w + x;
            let t = truth.data[i];
            let r = result.data[i];
            let (dr, dg, db) = (
                (t[0] - r[0]) as f64,
                (t[1] - r[1]) as f64,
                (t[2] - r[2]) as f64,
            );
            se_r += dr * dr;
            se_g += dg * dg;
            se_b += db * db;
            se += dr * dr + dg * dg + db * db;
            // color differences R-G, B-G
            let tcr = (t[0] - t[1]) as f64;
            let tcb = (t[2] - t[1]) as f64;
            let rcr = (r[0] - r[1]) as f64;
            let rcb = (r[2] - r[1]) as f64;
            se_c += (tcr - rcr) * (tcr - rcr) + (tcb - rcb) * (tcb - rcb);
            n += 1;
        }
    }
    let nf = n as f64;
    Metrics {
        psnr: psnr_from_mse(se / (3.0 * nf)),
        psnr_r: psnr_from_mse(se_r / nf),
        psnr_g: psnr_from_mse(se_g / nf),
        psnr_b: psnr_from_mse(se_b / nf),
        chroma_psnr: psnr_from_mse(se_c / (2.0 * nf)),
    }
}

// ---------------------------------------------------------------------------
// Synthetic ground-truth images
// ---------------------------------------------------------------------------

/// Grayscale radial chirp (zone plate): frequency rises with radius, sweeping
/// past Nyquist near the edges — the classic demosaic aliasing test.
fn zoneplate(n: usize) -> RgbImage {
    let mut img = RgbImage::new(n, n);
    let c = n as f32 / 2.0;
    // scale chosen so the local frequency reaches ~Nyquist toward the corners
    let k = std::f32::consts::PI / (n as f32 * 0.55);
    for y in 0..n {
        for x in 0..n {
            let dx = x as f32 - c;
            let dy = y as f32 - c;
            let v = 0.5 + 0.45 * (k * (dx * dx + dy * dy)).cos();
            img.data[y * n + x] = [v, v, v];
        }
    }
    img
}

/// Color chart: smooth hue gradient (left), hard-edged color blocks (right),
/// and a band of fine alternating color lines (bottom). Tests color fidelity,
/// edges, and fine chroma detail.
fn chart(n: usize) -> RgbImage {
    let mut img = RgbImage::new(n, n);
    let blocks = [
        [0.85, 0.15, 0.15],
        [0.15, 0.7, 0.2],
        [0.15, 0.25, 0.85],
        [0.9, 0.85, 0.15],
        [0.8, 0.2, 0.75],
        [0.15, 0.8, 0.85],
    ];
    for y in 0..n {
        for x in 0..n {
            let fx = x as f32 / n as f32;
            let fy = y as f32 / n as f32;
            let px = if fy > 0.75 {
                // resolvable alternating color lines (4px), stress chroma detail
                if (x / 4) % 2 == 0 {
                    [0.9, 0.1, 0.1]
                } else {
                    [0.1, 0.1, 0.9]
                }
            } else if fx < 0.5 {
                // smooth hue sweep + vertical luminance ramp
                let hue = fx * 2.0;
                let l = 0.35 + 0.5 * fy;
                hsv(hue, 0.8, l)
            } else {
                // hard-edged color blocks
                let bi = ((fx - 0.5) * 2.0 * 3.0) as usize % 3;
                let bj = (fy * 2.0) as usize % 2;
                blocks[(bj * 3 + bi) % blocks.len()]
            };
            img.data[y * n + x] = px;
        }
    }
    img
}

fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = (h % 1.0) * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i.rem_euclid(6) {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

// ---------------------------------------------------------------------------
// Mosaic + image I/O
// ---------------------------------------------------------------------------

/// Sample the appropriate channel per Bayer position to build a CFA mosaic.
fn mosaic(truth: &RgbImage, pattern: CfaPattern) -> CfaImage {
    let (w, h) = (truth.width, truth.height);
    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            data[i] = truth.data[i][pattern.color_at(x, y) as usize];
        }
    }
    CfaImage {
        width: w,
        height: h,
        data,
        pattern,
        wb: [1.0, 1.0, 1.0],
    }
}

/// Load an 8-bit PNG as a linear-ish [0,1] `RgbImage` ground truth.
fn load_truth(path: &std::path::Path) -> RgbImage {
    let img = image::open(path).expect("open truth png").to_rgb8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut data = vec![[0.0f32; 3]; w * h];
    for (x, y, px) in img.enumerate_pixels() {
        data[y as usize * w + x as usize] = [
            px[0] as f32 / 255.0,
            px[1] as f32 / 255.0,
            px[2] as f32 / 255.0,
        ];
    }
    RgbImage {
        width: w,
        height: h,
        data,
    }
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

fn save_rgb(img: &RgbImage, path: &std::path::Path) {
    let mut buf = image::RgbImage::new(img.width as u32, img.height as u32);
    for (i, px) in img.data.iter().enumerate() {
        let (x, y) = ((i % img.width) as u32, (i / img.width) as u32);
        buf.put_pixel(x, y, image::Rgb([to_u8(px[0]), to_u8(px[1]), to_u8(px[2])]));
    }
    buf.save(path).expect("save png");
}

fn save_mosaic(cfa: &CfaImage, path: &std::path::Path) {
    let mut buf = image::GrayImage::new(cfa.width as u32, cfa.height as u32);
    for (i, &v) in cfa.data.iter().enumerate() {
        let (x, y) = ((i % cfa.width) as u32, (i / cfa.width) as u32);
        buf.put_pixel(x, y, image::Luma([to_u8(v)]));
    }
    buf.save(path).expect("save mosaic");
}

/// Error-magnitude heatmap: |truth-result| per pixel, amplified 8x, as red.
fn save_error(truth: &RgbImage, result: &RgbImage, path: &std::path::Path) {
    let (w, h) = (truth.width, truth.height);
    let mut buf = image::RgbImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let t = truth.data[i];
            let r = result.data[i];
            let e = ((t[0] - r[0]).abs() + (t[1] - r[1]).abs() + (t[2] - r[2]).abs()) / 3.0;
            let v = to_u8(e * 8.0);
            buf.put_pixel(x as u32, y as u32, image::Rgb([v, v / 3, v / 3]));
        }
    }
    buf.save(path).expect("save error map");
}
