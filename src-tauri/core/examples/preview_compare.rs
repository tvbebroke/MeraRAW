//! Compare MeraRAW's rendered output against the camera's own embedded preview.
//!
//! The embedded JPEG is the manufacturer's rendering of the same exposure, so it
//! is the reference for two things this harness measures: whether neutral
//! highlights stay neutral, and whether the overall tone lands in the same
//! ballpark. It is NOT a target for exact tone matching — a scene-referred
//! neutral view legitimately differs from a camera JPEG's contrast.
//!
//! Usage:
//!   cargo run -p meratech-core --release --example preview_compare -- <dir-or-file>... [--out DIR]

use meratech_core::color::{mat_vec, Mat3};
use meratech_core::image::RgbF32Buf;
use meratech_core::raw::{Decoder, Demosaic, RawlerDecoder};
use std::path::{Path, PathBuf};

/// present.wgsl `REC2020_TO_SRGB`, transcribed row-major (WGSL literal is
/// column-major). Kept as a literal copy so a shader edit shows up as a diff.
const REC2020_TO_SRGB: Mat3 = [
    [1.6605, -0.5876, -0.0728],
    [-0.1246, 1.1329, -0.0083],
    [-0.0182, -0.1006, 1.1187],
];

const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];

fn luma(c: [f32; 3]) -> f32 {
    LUMA[0] * c[0] + LUMA[1] * c[1] + LUMA[2] * c[2]
}

const HL_PC_LO: f32 = 0.5;

fn tone_curve(v: f32, g: f32, contrast: f32) -> f32 {
    let x = v * g;
    let r = x * (1.0 + x / (g * g)) / (1.0 + x);
    let s = 0.5 - 0.5 * (r.clamp(0.0, 1.0) * std::f32::consts::PI).cos();
    (r + (s - r) * contrast).clamp(0.0, 1.0)
}

/// Exact CPU mirror of present.wgsl `view_look`. `lw_override` reproduces the
/// pre-fix transform (fixed white point 4.0, no baseline exposure, no
/// per-channel highlight blend) so the display change can be A/B'd without
/// rebuilding the shader.
fn view_look(c: [f32; 3], look: u32, lw_override: Option<f32>) -> [f32; 3] {
    let legacy = lw_override.is_some();
    let (gain, contrast, sat, baseline_ev, hl_pc) = if look == 1 {
        if legacy {
            (1.6f32, 0.34f32, 1.22f32, 0.0f32, 0.0f32)
        } else {
            (1.6f32, 0.62f32, 1.22f32, 0.75f32, 1.0f32)
        }
    } else {
        (1.15f32, 0.12f32, 1.0f32, 0.0f32, 0.0f32)
    };
    let g = gain * baseline_ev.exp2();
    let c = [c[0].max(0.0), c[1].max(0.0), c[2].max(0.0)];
    let l = luma(c);
    if l <= 1e-8 {
        return [0.0; 3];
    }
    // Legacy path keeps the decoupled white point; current path has lw == g.
    let ld = match lw_override {
        Some(lw) => {
            let x = l * gain;
            let r = x * (1.0 + x / (lw * lw)) / (1.0 + x);
            let s = 0.5 - 0.5 * (r.clamp(0.0, 1.0) * std::f32::consts::PI).cos();
            (r + (s - r) * contrast).clamp(0.0, 1.0)
        }
        None => tone_curve(l, g, contrast),
    };
    let k = ld / l;
    let mut o = [c[0] * k, c[1] * k, c[2] * k];
    let l2 = luma([o[0].max(0.0), o[1].max(0.0), o[2].max(0.0)]);
    for v in o.iter_mut() {
        *v = (l2 + (*v - l2) * sat).max(0.0);
    }
    if hl_pc > 0.0 {
        let u = ((ld - HL_PC_LO) / (1.0 - HL_PC_LO)).clamp(0.0, 1.0);
        let w = u * u * (3.0 - 2.0 * u) * hl_pc;
        for (v, &s) in o.iter_mut().zip(c.iter()) {
            *v += (tone_curve(s, g, contrast) - *v) * w;
        }
    }
    let mx = o[0].max(o[1]).max(o[2]);
    if mx > 1.0 && !legacy {
        let l3 = luma(o);
        let w = ((mx - 1.0) / mx).clamp(0.0, 1.0);
        for v in o.iter_mut() {
            *v = (*v + (l3 - *v) * w).max(0.0);
        }
    }
    o
}

fn oetf_srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Full present pass: linear Rec.2020 → display-encoded sRGB bytes.
fn present(buf: &RgbF32Buf, look: u32, lw: Option<f32>) -> Vec<u8> {
    let mut out = vec![0u8; buf.width * buf.height * 3];
    for (i, px) in buf.data.chunks_exact(3).enumerate() {
        let c = mat_vec(&REC2020_TO_SRGB, [px[0], px[1], px[2]]);
        let c = [c[0].max(0.0), c[1].max(0.0), c[2].max(0.0)];
        let looked = view_look(c, look, lw);
        for ch in 0..3 {
            out[i * 3 + ch] = (oetf_srgb(looked[ch].clamp(0.0, 1.0)) * 255.0).round() as u8;
        }
    }
    out
}

/// Nearest-neighbour resample of interleaved RGB8 to a fixed grid. Both sides
/// go through the same function so sampling bias cancels in the comparison.
fn resample(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * 3];
    for y in 0..dh {
        let sy = (y * sh / dh).min(sh.saturating_sub(1));
        for x in 0..dw {
            let sx = (x * sw / dw).min(sw.saturating_sub(1));
            let si = (sy * sw + sx) * 3;
            let di = (y * dw + x) * 3;
            out[di..di + 3].copy_from_slice(&src[si..si + 3]);
        }
    }
    out
}

fn rgba_to_rgb(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect()
}

const HL_PCT: f32 = 0.02;

fn mean_rgb(rgb: &[u8]) -> [f32; 3] {
    let n = (rgb.len() / 3).max(1);
    let mut m = [0.0f64; 3];
    for p in rgb.chunks_exact(3) {
        for c in 0..3 {
            m[c] += p[c] as f64;
        }
    }
    m.map(|v| (v / n as f64) as f32)
}

/// Indices of the brightest `HL_PCT` of pixels. Both images are measured over
/// the *same* set — chosen from the camera preview — so a rendering difference
/// can't quietly move the comparison onto different parts of the scene.
fn highlight_set(reference: &[u8]) -> Vec<usize> {
    let n = reference.len() / 3;
    let mut lumas: Vec<(f32, usize)> = (0..n)
        .map(|i| {
            let p = &reference[i * 3..i * 3 + 3];
            (
                LUMA[0] * p[0] as f32 + LUMA[1] * p[1] as f32 + LUMA[2] * p[2] as f32,
                i,
            )
        })
        .collect();
    lumas.sort_by(|a, b| b.0.total_cmp(&a.0));
    lumas
        .into_iter()
        .take(((n as f32 * HL_PCT) as usize).max(1))
        .map(|(_, i)| i)
        .collect()
}

fn mean_over(rgb: &[u8], idx: &[usize]) -> [f32; 3] {
    let mut m = [0.0f64; 3];
    for &i in idx {
        for c in 0..3 {
            m[c] += rgb[i * 3 + c] as f64;
        }
    }
    m.map(|v| (v / idx.len().max(1) as f64) as f32)
}

/// How far a highlight colour sits from neutral, as a signed magenta index in
/// 0-255 display units: positive = R and B exceed G (magenta / pink cast).
fn magenta_index(c: [f32; 3]) -> f32 {
    (c[0] + c[2]) * 0.5 - c[1]
}

/// Mean absolute per-channel difference in display units — the direct "how
/// close to the camera's own rendering" number. Never reaches zero, since the
/// camera JPEG carries its own contrast and saturation styling, but it is the
/// honest way to compare two candidate pipelines.
fn mean_abs_diff(a: &[u8], b: &[u8], idx: Option<&[usize]>) -> f32 {
    let mut acc = 0.0f64;
    let mut n = 0usize;
    let mut add = |i: usize| {
        for c in 0..3 {
            acc += (a[i * 3 + c] as f32 - b[i * 3 + c] as f32).abs() as f64;
        }
        n += 3;
    };
    match idx {
        Some(ix) => ix.iter().for_each(|&i| add(i)),
        None => (0..a.len() / 3).for_each(add),
    }
    (acc / n.max(1) as f64) as f32
}

/// Chroma-only difference: strip each side's own luminance first, so a pure
/// brightness or contrast difference does not count as a colour error.
fn mean_chroma_diff(a: &[u8], b: &[u8], idx: &[usize]) -> f32 {
    let mut acc = 0.0f64;
    for &i in idx {
        let pa: [f32; 3] = std::array::from_fn(|c| a[i * 3 + c] as f32);
        let pb: [f32; 3] = std::array::from_fn(|c| b[i * 3 + c] as f32);
        let (la, lb) = (luma(pa).max(1.0), luma(pb).max(1.0));
        for c in 0..3 {
            acc += ((pa[c] / la - pb[c] / lb) * 128.0).abs() as f64;
        }
    }
    (acc / (idx.len().max(1) * 3) as f64) as f32
}

fn is_raw(p: &Path) -> bool {
    let Some(ext) = p.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    !ext.eq_ignore_ascii_case("json")
        && !ext.eq_ignore_ascii_case("md")
        && !ext.eq_ignore_ascii_case("txt")
        && !ext.eq_ignore_ascii_case("sha256")
        && RawlerDecoder::default().probe(p)
}

fn collect(root: &Path, out: &mut Vec<PathBuf>) {
    if root.is_file() {
        if is_raw(root) {
            out.push(root.to_path_buf());
        }
        return;
    }
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for e in entries {
        collect(&e, out);
    }
}

fn main() {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut out_dir: Option<PathBuf> = None;
    let mut look = 1u32; // engine default is Camera
    let mut lw_override: Option<f32> = None;
    let mut demosaic = Demosaic::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => out_dir = args.next().map(PathBuf::from),
            "--look" => look = args.next().and_then(|v| v.parse().ok()).unwrap_or(1),
            "--lw" => lw_override = args.next().and_then(|v| v.parse().ok()),
            "--demosaic" => demosaic = Demosaic::parse_or_default(args.next().as_deref()),
            _ => roots.push(PathBuf::from(a)),
        }
    }
    assert!(!roots.is_empty(), "usage: preview_compare <dir-or-file>...");

    let mut files = Vec::new();
    for r in &roots {
        collect(r, &mut files);
    }
    println!("{} candidate RAW files\n", files.len());

    let dec = RawlerDecoder::default();
    if let Some(d) = &out_dir {
        std::fs::create_dir_all(d).expect("create out dir");
    }

    println!(
        "{:<34} {:>13} {:>13} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "file", "ours HL RGB", "cam HL RGB", "ourMg", "camMg", "lumaR", "hlD", "hlC", "allD"
    );
    println!("{}", "-".repeat(116));

    let mut rows = 0usize;
    let mut worst: Vec<(f32, String)> = Vec::new();
    let mut totals: Vec<(f32, f32, f32)> = Vec::new();

    for path in &files {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let decoded = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dec.decode_with_options(path, None, demosaic)
        })) {
            Ok(Ok(d)) => d,
            Ok(Err(e)) => {
                println!("{name:<34} decode failed: {e}");
                continue;
            }
            Err(_) => {
                println!("{name:<34} decode PANICKED");
                continue;
            }
        };

        let small = decoded.working.downscale_to(400);
        let ours = present(&small, look, lw_override);

        let cam = match dec.embedded_preview(path, 400) {
            Ok(Some((rgba, w, h))) => Some((rgba_to_rgb(&rgba), w as usize, h as usize)),
            _ => None,
        };

        let (gw, gh) = (256usize, 256usize);
        let ours_g = resample(&ours, small.width, small.height, gw, gh);

        match cam {
            Some((cam_rgb, cw, ch)) => {
                let cam_g = resample(&cam_rgb, cw, ch, gw, gh);
                let idx = highlight_set(&cam_g);
                let ohl = mean_over(&ours_g, &idx);
                let chl = mean_over(&cam_g, &idx);
                let om = magenta_index(ohl);
                let cm = magenta_index(chl);
                let luma_ratio = luma(mean_rgb(&ours_g)) / luma(mean_rgb(&cam_g)).max(1e-6);
                let hl_d = mean_abs_diff(&ours_g, &cam_g, Some(&idx));
                let hl_c = mean_chroma_diff(&ours_g, &cam_g, &idx);
                let all_d = mean_abs_diff(&ours_g, &cam_g, None);
                println!(
                    "{name:<34} {:>4.0},{:>3.0},{:>3.0} {:>5.0},{:>3.0},{:>3.0} {om:>6.1} {cm:>6.1} {luma_ratio:>6.2} {hl_d:>6.1} {hl_c:>6.1} {all_d:>6.1}",
                    ohl[0], ohl[1], ohl[2], chl[0], chl[1], chl[2],
                );
                worst.push((hl_c, name.clone()));
                totals.push((hl_d, hl_c, all_d));

                if let Some(d) = &out_dir {
                    // side by side: ours | camera
                    let mut sbs = vec![0u8; gw * 2 * gh * 3];
                    for y in 0..gh {
                        for x in 0..gw {
                            let d0 = (y * gw * 2 + x) * 3;
                            let s0 = (y * gw + x) * 3;
                            sbs[d0..d0 + 3].copy_from_slice(&ours_g[s0..s0 + 3]);
                            let d1 = (y * gw * 2 + gw + x) * 3;
                            sbs[d1..d1 + 3].copy_from_slice(&cam_g[s0..s0 + 3]);
                        }
                    }
                    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                    let _ = image::save_buffer(
                        d.join(format!("{stem}.png")),
                        &sbs,
                        (gw * 2) as u32,
                        gh as u32,
                        image::ColorType::Rgb8,
                    );
                }
            }
            None => {
                let idx = highlight_set(&ours_g);
                let ohl = mean_over(&ours_g, &idx);
                let om = magenta_index(ohl);
                println!(
                    "{name:<34} {:>4.0},{:>3.0},{:>3.0} {:>13} {om:>6.1}",
                    ohl[0], ohl[1], ohl[2], "(no preview)",
                );
            }
        }
        rows += 1;
    }

    worst.sort_by(|a, b| b.0.total_cmp(&a.0));
    let n = totals.len().max(1) as f32;
    let sum = totals.iter().fold((0.0, 0.0, 0.0), |a, t| {
        (a.0 + t.0, a.1 + t.1, a.2 + t.2)
    });
    println!(
        "\n{rows} compared. MEANS  highlight dRGB {:.1}  highlight chroma {:.1}  frame dRGB {:.1}",
        sum.0 / n,
        sum.1 / n,
        sum.2 / n
    );
    println!("Worst highlight chroma error vs camera:");
    for (d, nm) in worst.iter().take(8) {
        println!("  {d:>6.1}  {nm}");
    }
}
