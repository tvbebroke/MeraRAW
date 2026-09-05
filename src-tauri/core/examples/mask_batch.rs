//! Batch-evaluate AI masks (subject/face/skin/background) on many photos.
//! Writes overlays + metrics JSON — no GPU required.
//!
//! Usage:
//!   cargo run -p meratech-core --release --example mask_batch -- \
//!     /path/to/images_or_dir [/path/to/out_dir]
//!
//! Optional env:
//!   MERARAW_MASK_KINDS=subject,face,skin,background,object
//!   MERARAW_MASK_POINT=0.5,0.45   # normalized click for object/instance

use meratech_core::image::RgbF32Buf;
use meratech_core::raw::decoder_for;
use meratech_core::segment::{run_ai_mask, Mask01};
use serde_json::json;
use std::path::{Path, PathBuf};

fn coverage(mask: &Mask01, thr: f32) -> f32 {
    if mask.data.is_empty() {
        return 0.0;
    }
    mask.data.iter().filter(|v| **v > thr).count() as f32 / mask.data.len() as f32
}

fn bbox_frac(mask: &Mask01, thr: f32) -> (f32, f32, f32, f32) {
    let (w, h) = (mask.width, mask.height);
    let mut x0 = w;
    let mut x1 = 0usize;
    let mut y0 = h;
    let mut y1 = 0usize;
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            if mask.data[y * w + x] > thr {
                any = true;
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    if !any {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (
        x0 as f32 / w as f32,
        y0 as f32 / h as f32,
        (x1 + 1) as f32 / w as f32,
        (y1 + 1) as f32 / h as f32,
    )
}

/// Fraction of mask mass in the bottom 30% of the image (reflection bleed heuristic).
fn lower_third_mass(mask: &Mask01, thr: f32) -> f32 {
    let (w, h) = (mask.width, mask.height);
    let y_cut = (h as f32 * 0.7) as usize;
    let mut lo = 0.0f32;
    let mut all = 0.0f32;
    for y in 0..h {
        for x in 0..w {
            let v = mask.data[y * w + x];
            if v > thr {
                all += v;
                if y >= y_cut {
                    lo += v;
                }
            }
        }
    }
    if all < 1e-3 {
        0.0
    } else {
        lo / all
    }
}

/// Bilinear sample mask at image pixel (masks are often NET_SIZE 320×320).
fn sample_mask(mask: &Mask01, x: usize, y: usize, img_w: usize, img_h: usize) -> f32 {
    if mask.width == 0 || mask.height == 0 {
        return 0.0;
    }
    if mask.width == img_w && mask.height == img_h {
        return mask.data.get(y * mask.width + x).copied().unwrap_or(0.0);
    }
    let fx = (x as f32 + 0.5) * mask.width as f32 / img_w as f32 - 0.5;
    let fy = (y as f32 + 0.5) * mask.height as f32 / img_h as f32 - 0.5;
    let x0 = fx.floor().max(0.0) as usize;
    let y0 = fy.floor().max(0.0) as usize;
    let x1 = (x0 + 1).min(mask.width - 1);
    let y1 = (y0 + 1).min(mask.height - 1);
    let tx = (fx - x0 as f32).clamp(0.0, 1.0);
    let ty = (fy - y0 as f32).clamp(0.0, 1.0);
    let v00 = mask.data[y0 * mask.width + x0];
    let v10 = mask.data[y0 * mask.width + x1];
    let v01 = mask.data[y1 * mask.width + x0];
    let v11 = mask.data[y1 * mask.width + x1];
    let a = v00 * (1.0 - tx) + v10 * tx;
    let b = v01 * (1.0 - tx) + v11 * tx;
    a * (1.0 - ty) + b * ty
}

fn save_overlay(path: &Path, img: &RgbF32Buf, mask: &Mask01) {
    let (w, h) = (img.width, img.height);
    let mut rgb = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let si = (y * w + x) * 3;
            let mut r = (img.data[si].clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
            let mut g = (img.data[si + 1].clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
            let mut b = (img.data[si + 2].clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
            let m = sample_mask(mask, x, y, w, h);
            if m > 0.25 {
                // Red overlay for mask
                let a = ((m - 0.15) / 0.85).clamp(0.0, 1.0) * 0.55;
                r = ((1.0 - a) * r as f32 + a * 255.0) as u8;
                g = ((1.0 - a) * g as f32) as u8;
                b = ((1.0 - a) * b as f32) as u8;
            }
            let o = (y * w + x) * 3;
            rgb[o] = r;
            rgb[o + 1] = g;
            rgb[o + 2] = b;
        }
    }
    let _ = image::save_buffer(path, &rgb, w as u32, h as u32, image::ColorType::Rgb8);
}

fn collect_inputs(arg: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if arg.is_file() {
        out.push(arg.to_path_buf());
        return out;
    }
    let Ok(rd) = std::fs::read_dir(arg) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        let ext = p
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "tif" | "tiff" | "webp" | "arw" | "cr2" | "nef" | "dng" | "raf"
                | "orf" | "rw2"
        ) {
            out.push(p);
        }
    }
    out.sort();
    out
}

fn main() {
    let in_arg = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: mask_batch <image|dir> [out_dir]"),
    );
    let out_dir = PathBuf::from(
        std::env::args()
            .nth(2)
            .unwrap_or_else(|| "/tmp/mera-mask-batch".into()),
    );
    std::fs::create_dir_all(&out_dir).expect("out dir");

    let kinds: Vec<String> = std::env::var("MERARAW_MASK_KINDS")
        .unwrap_or_else(|_| "subject,face,skin,background".into())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let point = std::env::var("MERARAW_MASK_POINT")
        .ok()
        .and_then(|s| {
            let mut it = s.split(',');
            Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
        });

    let inputs = collect_inputs(&in_arg);
    println!("batch: {} images → {}", inputs.len(), out_dir.display());

    let mut report = Vec::new();
    for path in &inputs {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("img")
            .to_string();
        print!("— {stem} … ");
        let dec = decoder_for(path);
        let img = match dec.decode(path) {
            Ok(i) => i,
            Err(e) => {
                println!("DECODE FAIL: {e}");
                report.push(json!({"file": stem, "error": format!("decode: {e}")}));
                continue;
            }
        };
        let small = img.working.downscale_to(768);
        let mut file_rep = json!({
            "file": stem,
            "w": small.width,
            "h": small.height,
            "kinds": {}
        });

        for kind in &kinds {
            let hint = if kind == "object" || kind == "people" {
                point.or(Some((0.5, 0.45)))
            } else {
                None
            };
            let t0 = std::time::Instant::now();
            match run_ai_mask(kind, &small, hint, None, None) {
                Ok(mask) => {
                    let cov = coverage(&mask, 0.5);
                    let cov_soft = coverage(&mask, 0.25);
                    let bb = bbox_frac(&mask, 0.35);
                    let lower = lower_third_mass(&mask, 0.35);
                    let ms = t0.elapsed().as_millis();
                    let overlay = out_dir.join(format!("{stem}_{kind}.jpg"));
                    save_overlay(&overlay, &small, &mask);
                    // Pass heuristics (soft — for triage, not ground truth)
                    let mut flags = Vec::new();
                    if kind == "subject" || kind == "background" || kind == "people" {
                        if cov < 0.015 {
                            flags.push("too_small");
                        }
                        if cov > 0.88 {
                            flags.push("too_large");
                        }
                        if lower > 0.45 && bb.1 < 0.35 {
                            // Much of mask in bottom while subject bbox starts high → bleed risk
                            flags.push("lower_bleed_risk");
                        }
                        let bw = bb.2 - bb.0;
                        let bh = bb.3 - bb.1;
                        if bw > 0.92 && bh > 0.92 {
                            flags.push("full_frame_box");
                        }
                    }
                    if kind == "face" {
                        if cov_soft < 0.003 {
                            flags.push("face_empty");
                        }
                        if cov > 0.35 {
                            flags.push("face_too_large");
                        }
                        if bb.3 - bb.1 > 0.55 {
                            flags.push("face_tall");
                        }
                    }
                    file_rep["kinds"][kind] = json!({
                        "ms": ms,
                        "cov05": (cov * 1000.0).round() / 10.0,
                        "cov025": (cov_soft * 1000.0).round() / 10.0,
                        "bbox": [bb.0, bb.1, bb.2, bb.3],
                        "lower_mass": (lower * 1000.0).round() / 10.0,
                        "flags": flags,
                        "overlay": overlay.file_name().and_then(|s| s.to_str()),
                    });
                }
                Err(e) => {
                    file_rep["kinds"][kind] = json!({"error": format!("{e}")});
                }
            }
        }
        println!("ok");
        report.push(file_rep);
    }

    let rep_path = out_dir.join("report.json");
    std::fs::write(&rep_path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    println!("wrote {}", rep_path.display());

    // Summary triage
    let mut n_ok = 0;
    let mut n_flag = 0;
    for r in &report {
        if r.get("error").is_some() {
            continue;
        }
        n_ok += 1;
        if let Some(kinds) = r.get("kinds").and_then(|k| k.as_object()) {
            for (_k, v) in kinds {
                if let Some(flags) = v.get("flags").and_then(|f| f.as_array()) {
                    if !flags.is_empty() {
                        n_flag += 1;
                        break;
                    }
                }
            }
        }
    }
    println!("summary: {n_ok} decoded, {n_flag} with triage flags");
}
