//! Segmenter — contract B2. On-device, offline, private: pixel masks come
//! from local models, never the cloud (the conductor split, spec 4.2).
//!
//! Primary: tract-onnx (pure Rust — no FFI/runtime download; the `ort`
//! CoreML path is a contained swap behind this trait).
//! Subject: bundled u2netp, or override via `MERARAW_SUBJECT_MODEL` /
//! `~/Library/Application Support/MeraRAW/models/subject/merasubject-v1.onnx`
//! (see `segment/train/`). Sky: bundled U²-Net skyseg (MIT, xiongzhu666).
//! People faces: bundled UltraFace RFB-320 (MIT, Linzaer). Eyes: UltraFace
//! boxes plus a contrast detector on the subject (wildlife + portraits).
//! Object-by-point: color-similarity region grow seeded at the point.

use crate::error::CoreError;
use crate::image::RgbF32Buf;
use std::path::PathBuf;
use std::sync::OnceLock;
use tract_onnx::prelude::*;

/// Soft 0..1 mask at its own resolution (graph upsamples edge-aware).
pub struct Mask01 {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

/// Clickable object/subject candidate for the object-pick UI.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectProposal {
    pub id: u32,
    /// Closed polyline in normalized image coords (0..1).
    pub path: Vec<[f32; 2]>,
    pub centroid: [f32; 2],
    pub area: f32,
}

pub trait Segmenter: Send + Sync {
    fn subject(&self, img: &RgbF32Buf) -> Result<Mask01, CoreError>;
    fn sky(&self, img: &RgbF32Buf) -> Result<Mask01, CoreError>;
    fn object(&self, img: &RgbF32Buf, point: (f32, f32)) -> Result<Mask01, CoreError>;
}

type TractModel = std::sync::Arc<TypedSimplePlan>;

pub struct TractSegmenter;

static U2NETP: OnceLock<Result<TractModel, String>> = OnceLock::new();
static SKYSEG: OnceLock<Result<TractModel, String>> = OnceLock::new();

const SUBJECT_MODEL_BYTES: &[u8] = include_bytes!("../models/u2netp.onnx");
const NET_SIZE: usize = 320;

fn compile_u2net(bytes: &[u8]) -> Result<TractModel, String> {
    let mut cursor = std::io::Cursor::new(bytes);
    tract_onnx::onnx()
        .model_for_read(&mut cursor)
        .and_then(|m| {
            m.with_input_fact(
                0,
                InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 3, NET_SIZE, NET_SIZE)),
            )
        })
        .and_then(|m| m.into_optimized())
        .and_then(|m| m.into_runnable())
        .map_err(|e| e.to_string())
}

fn subject_model_path() -> Option<PathBuf> {
    for key in ["MERARAW_SUBJECT_MODEL", "MERATECH_SUBJECT_MODEL"] {
        if let Ok(p) = std::env::var(key) {
            let path = PathBuf::from(p);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    // Optional drop-in next to skyseg (not bundled — download/train separately).
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home)
            .join("Library/Application Support/MeraRAW/models/subject/merasubject-v1.onnx");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn subject_model() -> Result<&'static TractModel, CoreError> {
    U2NETP
        .get_or_init(|| {
            if let Some(path) = subject_model_path() {
                return std::fs::read(&path)
                    .map_err(|e| format!("read {}: {e}", path.display()))
                    .and_then(|bytes| compile_u2net(&bytes));
            }
            compile_u2net(SUBJECT_MODEL_BYTES)
        })
        .as_ref()
        .map_err(|e| CoreError::Engine(format!("u2netp load: {e}")))
}

fn sky_model_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("MERATECH_SKY_MODEL") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models/skyseg.onnx");
    if bundled.is_file() {
        return Some(bundled);
    }
    None
}

fn sky_model() -> Result<&'static TractModel, CoreError> {
    SKYSEG
        .get_or_init(|| {
            let path = sky_model_path().ok_or_else(|| {
                "skyseg.onnx missing — run scripts/download-sky-model.sh".to_string()
            })?;
            std::fs::read(&path)
                .map_err(|e| format!("read {}: {e}", path.display()))
                .and_then(|bytes| compile_u2net(&bytes))
        })
        .as_ref()
        .map_err(|e| CoreError::Engine(format!("skyseg load: {e}")))
}

/// Display-ish gamma for model input (models train on encoded images).
fn enc(v: f32) -> f32 {
    v.clamp(0.0, 1.0).powf(1.0 / 2.2)
}

fn run_u2net(plan: &TractModel, img: &RgbF32Buf) -> Result<Mask01, CoreError> {
    let mean = [0.485f32, 0.456, 0.406];
    let std = [0.229f32, 0.224, 0.225];
    let mut input = vec![0.0f32; 3 * NET_SIZE * NET_SIZE];
    for y in 0..NET_SIZE {
        let sy = (y * img.height / NET_SIZE).min(img.height.saturating_sub(1));
        for x in 0..NET_SIZE {
            let sx = (x * img.width / NET_SIZE).min(img.width.saturating_sub(1));
            let i = (sy * img.width + sx) * 3;
            for c in 0..3 {
                input[c * NET_SIZE * NET_SIZE + y * NET_SIZE + x] =
                    (enc(img.data[i + c]) - mean[c]) / std[c];
            }
        }
    }
    let tensor = Tensor::from_shape(&[1, 3, NET_SIZE, NET_SIZE], &input).map_err(tr_err)?;
    let result = plan.run(tvec!(tensor.into())).map_err(tr_err)?;
    let view = result[0].view();
    let raw: Vec<f32> = view.as_slice::<f32>().map_err(tr_err)?.to_vec();
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for v in &raw {
        lo = lo.min(*v);
        hi = hi.max(*v);
    }
    let range = (hi - lo).max(1e-6);
    Ok(Mask01 {
        width: NET_SIZE,
        height: NET_SIZE,
        data: raw.iter().map(|v| (v - lo) / range).collect(),
    })
}

fn tr_err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Engine(format!("segmentation: {e}"))
}

/// Box mean via integral image (Darktable-style guided-filter building block).
fn box_mean(src: &[f32], w: usize, h: usize, radius: usize) -> Vec<f32> {
    let tw = w + 1;
    let mut sat = vec![0.0f32; tw * (h + 1)];
    for y in 0..h {
        let mut row = 0.0f32;
        for x in 0..w {
            row += src[y * w + x];
            sat[(y + 1) * tw + (x + 1)] = sat[y * tw + (x + 1)] + row;
        }
    }
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        let y0 = y.saturating_sub(radius);
        let y1 = (y + radius + 1).min(h);
        for x in 0..w {
            let x0 = x.saturating_sub(radius);
            let x1 = (x + radius + 1).min(w);
            let sum = sat[y1 * tw + x1] - sat[y0 * tw + x1] - sat[y1 * tw + x0] + sat[y0 * tw + x0];
            let n = ((x1 - x0) * (y1 - y0)).max(1) as f32;
            out[y * w + x] = sum / n;
        }
    }
    out
}

/// Luma-guided filter (He, Sun, Tang) — same idea Darktable uses to feather
/// masks to image edges. Aligns the U²-Net prior to the photo; does not fill
/// a bounding box.
fn guided_filter_mask(mask: &mut [f32], w: usize, h: usize, img: &RgbF32Buf, radius: usize, eps: f32) {
    if w * h == 0 || radius == 0 {
        return;
    }
    let mut guide = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let rgb = sample_img_rgb(img, x, y, w, h);
            guide[y * w + x] = 0.2627 * rgb[0] + 0.678 * rgb[1] + 0.0593 * rgb[2];
        }
    }
    let mean_i = box_mean(&guide, w, h, radius);
    let mean_p = box_mean(mask, w, h, radius);
    let mut corr_ip = vec![0.0f32; w * h];
    let mut corr_ii = vec![0.0f32; w * h];
    for i in 0..w * h {
        corr_ip[i] = guide[i] * mask[i];
        corr_ii[i] = guide[i] * guide[i];
    }
    let mean_ip = box_mean(&corr_ip, w, h, radius);
    let mean_ii = box_mean(&corr_ii, w, h, radius);
    let mut a = vec![0.0f32; w * h];
    let mut b = vec![0.0f32; w * h];
    for i in 0..w * h {
        let var_i = (mean_ii[i] - mean_i[i] * mean_i[i]).max(0.0);
        let cov = mean_ip[i] - mean_i[i] * mean_p[i];
        a[i] = cov / (var_i + eps);
        b[i] = mean_p[i] - a[i] * mean_i[i];
    }
    let mean_a = box_mean(&a, w, h, radius);
    let mean_b = box_mean(&b, w, h, radius);
    for i in 0..w * h {
        mask[i] = (mean_a[i] * guide[i] + mean_b[i]).clamp(0.0, 1.0);
    }
}

fn box_blur_3x3(src: &[f32], w: usize, h: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0.0f32;
            let mut n = 0u32;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                        sum += src[ny as usize * w + nx as usize];
                        n += 1;
                    }
                }
            }
            out[y * w + x] = sum / n as f32;
        }
    }
    out
}

/// Post-process u2net saliency: sharper edges, less speckle, small hole fill.
fn refine_saliency_mask(data: &mut [f32], w: usize, h: usize) {
    // Mild contrast — avoid crushing weak wing / limb scores against dark BG.
    for v in data.iter_mut() {
        *v = ((*v - 0.45) * 1.25 + 0.45).clamp(0.0, 1.0);
    }
    let blurred = box_blur_3x3(data, w, h);
    for (i, b) in blurred.iter().enumerate() {
        data[i] = data[i] * 0.7 + b * 0.3;
    }
    let tmp = data.to_vec();
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let i = y * w + x;
            if tmp[i] >= 0.12 {
                continue;
            }
            let mut strong = 0u32;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let j = (y as i32 + dy) as usize * w + (x as i32 + dx) as usize;
                if tmp[j] > 0.55 {
                    strong += 1;
                }
            }
            if strong < 2 {
                data[i] = 0.0;
            }
        }
    }
    let tmp = data.to_vec();
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let i = y * w + x;
            if tmp[i] > 0.45 {
                continue;
            }
            let mut strong = 0u32;
            for (dx, dy) in [
                (-1, 0),
                (1, 0),
                (0, -1),
                (0, 1),
                (-1, -1),
                (1, -1),
                (-1, 1),
                (1, 1),
            ] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 {
                    let j = ny as usize * w + nx as usize;
                    if tmp[j] > 0.65 {
                        strong += 1;
                    }
                }
            }
            if strong >= 6 {
                data[i] = 0.55;
            }
        }
    }
}

/// Edge-aware subject refine (Darktable-style): U²-Net prior → guided filter
/// to the photo → drop islands / underside bleed. No species pad-flood.
fn refine_saliency_with_image(data: &mut [f32], w: usize, h: usize, img: &RgbF32Buf) {
    refine_saliency_mask(data, w, h);
    if w < 3 || h < 3 {
        return;
    }
    let prior = data.to_vec();
    let sx = img.width as f32 / w as f32;
    let sy = img.height as f32 / h as f32;
    let tmp = data.to_vec();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            let v = tmp[i];
            if v < 0.08 || v > 0.92 {
                continue;
            }
            let ix = ((x as f32 + 0.5) * sx) as usize;
            let iy = ((y as f32 + 0.5) * sy) as usize;
            let ii = (iy.min(img.height.saturating_sub(1)) * img.width
                + ix.min(img.width.saturating_sub(1)))
                * 3;
            let (cr, cg, cb) = (img.data[ii], img.data[ii + 1], img.data[ii + 2]);
            let mut wsum = 0.0f32;
            let mut vsum = 0.0f32;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = (x as i32 + dx) as usize;
                let ny = (y as i32 + dy) as usize;
                let j = ny * w + nx;
                let jx = ((nx as f32 + 0.5) * sx) as usize;
                let jy = ((ny as f32 + 0.5) * sy) as usize;
                let ji = (jy.min(img.height.saturating_sub(1)) * img.width
                    + jx.min(img.width.saturating_sub(1)))
                    * 3;
                let dr = img.data[ji] - cr;
                let dg = img.data[ji + 1] - cg;
                let db = img.data[ji + 2] - cb;
                let wgt = (-(dr * dr + dg * dg + db * db).sqrt() * 14.0).exp();
                wsum += wgt;
                vsum += tmp[j] * wgt;
            }
            if wsum > 0.0 {
                data[i] = (v * 0.5 + (vsum / wsum) * 0.5).clamp(0.0, 1.0);
            }
        }
    }
    guided_filter_mask(data, w, h, img, 2, 0.01);
    grow_attached_extremities(data, w, h, img);
    suppress_reflections_and_islands(data, w, h);
    suppress_underside_water_bleed(data, w, h, img);
    revert_if_boxed(data, &prior, w, h);
}

fn sample_img_rgb(img: &RgbF32Buf, mx: usize, my: usize, mw: usize, mh: usize) -> [f32; 3] {
    let sx = ((mx as f32 + 0.5) * img.width as f32 / mw as f32) as usize;
    let sy = ((my as f32 + 0.5) * img.height as f32 / mh as f32) as usize;
    let i = (sy.min(img.height.saturating_sub(1)) * img.width + sx.min(img.width.saturating_sub(1)))
        * 3;
    [img.data[i], img.data[i + 1], img.data[i + 2]]
}

fn is_waterish_rgb(r: f32, g: f32, b: f32) -> bool {
    let er = enc(r);
    let eg = enc(g);
    let eb = enc(b);
    let luma = 0.299 * er + 0.587 * eg + 0.114 * eb;
    // Blue/cyan water — not black feathers (which are low chroma + near-equal RGB).
    // Neutral dark water is rejected in grow via outside-background distance, not here —
    // a "dark flat" heuristic also kills black wings/feathers.
    eb > er + 0.035 && eb >= eg * 0.9 && luma < 0.55 && (eb - er.max(eg)) > 0.015
}

fn is_dark_extremity_rgb(r: f32, g: f32, b: f32) -> bool {
    let er = enc(r);
    let eg = enc(g);
    let eb = enc(b);
    let luma = 0.299 * er + 0.587 * eg + 0.114 * eb;
    let mx = er.max(eg).max(eb);
    let mn = er.min(eg).min(eb);
    let chroma = mx - mn;
    // Wet black / dark brown feathers: dark, low chroma, not blue-led.
    luma < 0.42 && chroma < 0.18 && !(eb > er + 0.05 && eb > eg + 0.03)
}

fn rgb_dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dr = a[0] - b[0];
    let dg = a[1] - b[1];
    let db = a[2] - b[2];
    (dr * dr + dg * dg + db * db).sqrt()
}

/// Mean color of likely background (outside the subject bbox) — usually water/ground.
fn sample_outside_bg_mean(
    img: &RgbF32Buf,
    mw: usize,
    mh: usize,
    bx0: usize,
    bx1: usize,
    by0: usize,
    by1: usize,
) -> Option<[f32; 3]> {
    let mut sum = [0.0f32; 3];
    let mut n = 0u32;
    let step = ((mw.max(mh) / 48).max(1)) as usize;
    for y in (0..mh).step_by(step) {
        for x in (0..mw).step_by(step) {
            let inside = x >= bx0 && x <= bx1 && y >= by0 && y <= by1;
            if inside {
                continue;
            }
            let rgb = sample_img_rgb(img, x, y, mw, mh);
            sum[0] += rgb[0];
            sum[1] += rgb[1];
            sum[2] += rgb[2];
            n += 1;
        }
    }
    if n < 12 {
        return None;
    }
    Some([sum[0] / n as f32, sum[1] / n as f32, sum[2] / n as f32])
}

/// Local luma variance in a 3×3 mask-space neighborhood (feathers vs flat water).
fn local_luma_var(img: &RgbF32Buf, mx: usize, my: usize, mw: usize, mh: usize) -> f32 {
    let mut sum = 0.0f32;
    let mut sum2 = 0.0f32;
    let mut n = 0u32;
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            let x = mx as i32 + dx;
            let y = my as i32 + dy;
            if x < 0 || y < 0 || x as usize >= mw || y as usize >= mh {
                continue;
            }
            let rgb = sample_img_rgb(img, x as usize, y as usize, mw, mh);
            let lum = 0.2627 * rgb[0] + 0.678 * rgb[1] + 0.0593 * rgb[2];
            sum += lum;
            sum2 += lum * lum;
            n += 1;
        }
    }
    if n < 4 {
        return 1.0;
    }
    let mean = sum / n as f32;
    (sum2 / n as f32 - mean * mean).max(0.0)
}

/// Dark pixels in a large, edge-reaching field (water, ground, sky-adjacent
/// fill) rather than a compact attached part (limb, coat, two-tone body).
/// Used so grow never treats the ocean as a second subject color.
fn mark_background_scale_dark(img: &RgbF32Buf, w: usize, h: usize) -> Vec<bool> {
    let n = w * h;
    let mut is_dark = vec![false; n];
    for y in 0..h {
        for x in 0..w {
            let rgb = sample_img_rgb(img, x, y, w, h);
            if is_waterish_rgb(rgb[0], rgb[1], rgb[2]) {
                continue;
            }
            if is_dark_extremity_rgb(rgb[0], rgb[1], rgb[2]) {
                is_dark[y * w + x] = true;
            }
        }
    }
    let mut seen = vec![false; n];
    let mut bg = vec![false; n];
    let mut stack = Vec::new();
    let img_area = n as f32;
    for i in 0..n {
        if !is_dark[i] || seen[i] {
            continue;
        }
        stack.clear();
        stack.push(i);
        seen[i] = true;
        let mut comp = Vec::new();
        let mut touch_l = false;
        let mut touch_r = false;
        let mut touch_t = false;
        let mut touch_b = false;
        while let Some(j) = stack.pop() {
            comp.push(j);
            let x = j % w;
            let y = j / w;
            if x == 0 {
                touch_l = true;
            }
            if x + 1 == w {
                touch_r = true;
            }
            if y == 0 {
                touch_t = true;
            }
            if y + 1 == h {
                touch_b = true;
            }
            for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                    continue;
                }
                let k = ny as usize * w + nx as usize;
                if seen[k] || !is_dark[k] {
                    continue;
                }
                seen[k] = true;
                stack.push(k);
            }
        }
        let area = comp.len() as f32;
        let edges = u32::from(touch_l) + u32::from(touch_r) + u32::from(touch_t) + u32::from(touch_b);
        let background = area > img_area * 0.18
            || (edges >= 3 && area > img_area * 0.06)
            || (edges >= 2 && area > img_area * 0.12);
        if background {
            for j in comp {
                bg[j] = true;
            }
        }
    }
    bg
}

/// Expand the subject into attached pixels that match the core (or a compact
/// darker attached region). General — not species-specific.
///
/// Never flood a padded axis-aligned rectangle. Large dark fields that reach
/// the frame (water/ground) are banned as grow targets.
fn grow_attached_extremities(data: &mut [f32], w: usize, h: usize, img: &RgbF32Buf) {
    if w * h == 0 {
        return;
    }
    // Mean body color from confident core.
    let mut sum = [0.0f32; 3];
    let mut n = 0u32;
    let mut x0 = w;
    let mut x1 = 0usize;
    let mut y0 = h;
    let mut y1 = 0usize;
    for y in 0..h {
        for x in 0..w {
            if data[y * w + x] < 0.55 {
                continue;
            }
            let rgb = sample_img_rgb(img, x, y, w, h);
            sum[0] += rgb[0];
            sum[1] += rgb[1];
            sum[2] += rgb[2];
            n += 1;
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
    }
    if n < 8 {
        return;
    }
    let mean = [sum[0] / n as f32, sum[1] / n as f32, sum[2] / n as f32];
    let mean_lum = 0.2627 * mean[0] + 0.678 * mean[1] + 0.0593 * mean[2];
    let color_thresh = (0.22 * (mean_lum + 0.25)).max(0.06);
    let before = data.to_vec();
    let bg_dark = mark_background_scale_dark(img, w, h);

    // Expand bbox so wings sticking out still count as "near".
    let pad_x = ((x1.saturating_sub(x0) as f32) * 0.45).max(8.0) as usize;
    let pad_y = ((y1.saturating_sub(y0) as f32) * 0.55).max(10.0) as usize;
    let mut bx0 = x0.saturating_sub(pad_x);
    let mut bx1 = (x1 + pad_x).min(w.saturating_sub(1));
    let mut by0 = y0.saturating_sub(pad_y);
    let mut by1 = (y1 + pad_y).min(h.saturating_sub(1));

    let bg = sample_outside_bg_mean(img, w, h, bx0, bx1, by0, by1);

    // Two-tone wildlife (eagle white head / dark body): sample dark pixels just
    // outside the bright core as a secondary grow target.
    let mut dark_sum = [0.0f32; 3];
    let mut dark_n = 0u32;
    for y in by0..=by1 {
        for x in bx0..=bx1 {
            let i = y * w + x;
            if data[i] >= 0.28 {
                continue;
            }
            if bg_dark[i] {
                continue;
            }
            let mut near_core = false;
            for dy in -3i32..=3 {
                for dx in -3i32..=3 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                        continue;
                    }
                    if data[ny as usize * w + nx as usize] >= 0.45 {
                        near_core = true;
                        break;
                    }
                }
                if near_core {
                    break;
                }
            }
            if !near_core {
                continue;
            }
            let rgb = sample_img_rgb(img, x, y, w, h);
            if !is_dark_extremity_rgb(rgb[0], rgb[1], rgb[2]) {
                continue;
            }
            if is_waterish_rgb(rgb[0], rgb[1], rgb[2]) {
                continue;
            }
            if let Some(bg_mean) = bg {
                if rgb_dist(rgb, bg_mean) < 0.05 {
                    continue;
                }
            }
            dark_sum[0] += rgb[0];
            dark_sum[1] += rgb[1];
            dark_sum[2] += rgb[2];
            dark_n += 1;
        }
    }
    let dark_mean = if dark_n >= 6 {
        let mean = [
            dark_sum[0] / dark_n as f32,
            dark_sum[1] / dark_n as f32,
            dark_sum[2] / dark_n as f32,
        ];
        if is_waterish_rgb(mean[0], mean[1], mean[2]) {
            None
        } else {
            Some(mean)
        }
    } else {
        None
    };
    let two_tone = mean_lum > 0.35 && dark_mean.is_some();
    if two_tone {
        // Reach a compact darker region attached below the core. Keep the
        // search band narrow — a wide pad is how water becomes a rectangle.
        let extra_y = ((y1.saturating_sub(y0) as f32) * 1.2).max(20.0) as usize;
        let extra_x = ((x1.saturating_sub(x0) as f32) * 0.25).max(8.0) as usize;
        bx0 = x0.saturating_sub(extra_x);
        bx1 = (x1 + extra_x).min(w.saturating_sub(1));
        by0 = y0;
        by1 = (y1 + extra_y).min(h.saturating_sub(1));
    }

    // Depth-limited flood — unbounded BFS fills the padded rectangle of dark water
    // (puffin splash: soft red box with bright droplet holes).
    let max_depth: u32 = if two_tone { 28 } else { 14 };
    let max_grow = ((w * h) as f32 * if two_tone { 0.18 } else { 0.08 }).max(48.0) as usize;
    let mut grown = 0usize;
    let mut queue: Vec<(usize, u32)> = Vec::new();
    for y in by0..=by1 {
        for x in bx0..=bx1 {
            let i = y * w + x;
            let v = data[i];
            if v < 0.18 {
                continue;
            }
            let mut outside = false;
            for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                    continue;
                }
                if data[ny as usize * w + nx as usize] < 0.28 {
                    outside = true;
                    break;
                }
            }
            if outside {
                queue.push((i, 0));
            }
        }
    }

    let local_color_lim = (color_thresh * 1.35).max(0.05);
    let mut qi = 0usize;
    while qi < queue.len() && grown < max_grow {
        let (i, depth) = queue[qi];
        qi += 1;
        if depth >= max_depth {
            continue;
        }
        let x = i % w;
        let y = i / w;
        let parent_rgb = sample_img_rgb(img, x, y, w, h);
        for (dx, dy) in [
            (-1i32, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                continue;
            }
            let nx = nx as usize;
            let ny = ny as usize;
            if nx < bx0 || nx > bx1 || ny < by0 || ny > by1 {
                continue;
            }
            let j = ny * w + nx;
            if data[j] >= 0.28 {
                continue;
            }
            if bg_dark[j] {
                continue;
            }
            let rgb = sample_img_rgb(img, nx, ny, w, h);
            if is_waterish_rgb(rgb[0], rgb[1], rgb[2]) {
                continue;
            }
            // Must resemble the local edge — stops cascading across flat water.
            // Exception: two-tone plumage (white head → dark body) is a hard jump.
            let d_local = rgb_dist(rgb, parent_rgb);
            let dark_pix = is_dark_extremity_rgb(rgb[0], rgb[1], rgb[2]);
            let d_dark_pre = dark_mean.map(|dm| rgb_dist(rgb, dm));
            let two_tone_jump = two_tone
                && dark_pix
                && d_dark_pre.map(|d| d <= color_thresh * 2.4).unwrap_or(false);
            if d_local > local_color_lim * 1.15 && !two_tone_jump {
                continue;
            }
            let d_body = rgb_dist(rgb, mean);
            let d_dark = d_dark_pre;
            let toward_subject = d_body.min(d_dark.unwrap_or(d_body));
            if let Some(bg_mean) = bg {
                let d_bg = rgb_dist(rgb, bg_mean);
                // Clearly closer to background than subject → water/ground fill.
                if d_bg + 0.02 < toward_subject {
                    continue;
                }
                // Ambiguous dark-on-dark: only allow if textured or two-tone body path.
                // Flat dark water between splash droplets sits in this band.
                if toward_subject + 0.015 >= d_bg {
                    let var = local_luma_var(img, nx, ny, w, h);
                    if var < 0.00025 && !two_tone_jump {
                        continue;
                    }
                }
            }
            let dark = dark_pix;
            let body_like = d_body <= color_thresh * if dark { 1.65 } else { 1.0 };
            let dark_ok = dark && d_body <= color_thresh * 2.15;
            let two_tone_ok = two_tone_jump;
            if !body_like && !dark_ok && !two_tone_ok {
                continue;
            }
            let boost = if dark_ok || two_tone_ok {
                0.62
            } else {
                (0.55 * (1.0 - d_body / (color_thresh * 1.65 + 1e-5))).clamp(0.3, 0.55)
            };
            if boost > data[j] {
                data[j] = boost;
                grown += 1;
                queue.push((j, depth + 1));
            }
        }
    }

    if grown > 0 {
        // If growth mostly painted a soft rectangle (high fill of the pad with
        // mid values), peel newly grown pixels — keep the pre-grow silhouette.
        let pad_area = ((bx1 + 1).saturating_sub(bx0)) * ((by1 + 1).saturating_sub(by0));
        let mut new_soft = 0u32;
        for y in by0..=by1 {
            for x in bx0..=bx1 {
                let i = y * w + x;
                if before[i] < 0.28 && data[i] >= 0.28 && data[i] < 0.78 {
                    new_soft += 1;
                }
            }
        }
        let fill = new_soft as f32 / pad_area.max(1) as f32;
        let fill_lim = if two_tone { 0.58 } else { 0.22 };
        let grown_lim = if two_tone {
            (n as f32) * 6.0
        } else {
            (n as f32) * 2.5
        };
        if fill > fill_lim || grown as f32 > grown_lim {
            for i in 0..data.len() {
                if before[i] < 0.28 {
                    data[i] = before[i];
                }
            }
            return;
        }
        if revert_if_boxed(data, &before, w, h) {
            return;
        }
        let blurred = box_blur_3x3(data, w, h);
        for i in 0..data.len() {
            if data[i] > 0.2 && data[i] < 0.75 {
                data[i] = (data[i] * 0.7 + blurred[i] * 0.3).clamp(0.0, 1.0);
            }
        }
    }
}

/// Score how bird-like a pixel is (black/white plumage + warm beak/feet).
fn wildlife_color_score(r: f32, g: f32, b: f32) -> f32 {
    let er = enc(r);
    let eg = enc(g);
    let eb = enc(b);
    let luma = 0.299 * er + 0.587 * eg + 0.114 * eb;
    let mx = er.max(eg).max(eb);
    let mn = er.min(eg).min(eb);
    let chroma = mx - mn;
    // Black / dark wet feathers (allow slight blue cast from wet/specular water)
    let black = if luma < 0.30
        && chroma < 0.18
        && !(eb > er + 0.06)
        && !(eb > eg + 0.045)
    {
        ((0.30 - luma) / 0.30).clamp(0.0, 1.0) * (1.0 - chroma / 0.18)
    } else {
        0.0
    };
    // White face / chest — slightly higher chroma floor so pure specular
    // splash droplets don't outrank plumage (handled with neighborhood check).
    let white = if luma > 0.52 && chroma < 0.2 {
        ((luma - 0.52) / 0.38).clamp(0.0, 1.0) * (1.0 - chroma / 0.2) * 0.85
    } else {
        0.0
    };
    // Warm beak / feet / eye ring
    let warm = if er > eg + 0.04 && er > eb + 0.06 && luma > 0.18 && luma < 0.9 {
        ((er - eg.max(eb)) / 0.28).clamp(0.0, 1.0)
    } else {
        0.0
    };
    black.max(white).max(warm)
}

/// Bright droplet surrounded by dark water — not plumage.
fn is_splash_speckle(img: &RgbF32Buf, mx: usize, my: usize, mw: usize, mh: usize) -> bool {
    let rgb = sample_img_rgb(img, mx, my, mw, mh);
    let er = enc(rgb[0]);
    let eg = enc(rgb[1]);
    let eb = enc(rgb[2]);
    let luma = 0.299 * er + 0.587 * eg + 0.114 * eb;
    if luma < 0.5 {
        return false;
    }
    let mut dark = 0u32;
    let mut n = 0u32;
    for dy in -2i32..=2 {
        for dx in -2i32..=2 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let x = mx as i32 + dx;
            let y = my as i32 + dy;
            if x < 0 || y < 0 || x as usize >= mw || y as usize >= mh {
                continue;
            }
            let nrgb = sample_img_rgb(img, x as usize, y as usize, mw, mh);
            let nl = 0.2627 * nrgb[0] + 0.678 * nrgb[1] + 0.0593 * nrgb[2];
            n += 1;
            if nl < 0.22 {
                dark += 1;
            }
        }
    }
    n >= 12 && dark as f32 / n as f32 > 0.55
}

/// Dark / mid water. Bright splash droplets are handled by `is_splash_speckle` —
/// do NOT treat solid white plumage (face/chest) as splash here.
fn water_or_splash_score(r: f32, g: f32, b: f32) -> f32 {
    let er = enc(r);
    let eg = enc(g);
    let eb = enc(b);
    let luma = 0.299 * er + 0.587 * eg + 0.114 * eb;
    let mx = er.max(eg).max(eb);
    let mn = er.min(eg).min(eb);
    let chroma = mx - mn;
    if luma < 0.42 && chroma < 0.14 {
        let blue = ((eb - er) / 0.08).clamp(0.0, 1.0);
        // Neutral dark needs a lower base so black plumage can win.
        (0.22 + 0.78 * blue) * (1.0 - chroma / 0.14) * ((0.42 - luma) / 0.42)
    } else {
        0.0
    }
}

/// Flat dark neighborhood — typical of water / rock fill, not feather texture.
fn is_flat_dark_region(img: &RgbF32Buf, mx: usize, my: usize, mw: usize, mh: usize) -> bool {
    let rgb = sample_img_rgb(img, mx, my, mw, mh);
    let luma = 0.2627 * rgb[0] + 0.678 * rgb[1] + 0.0593 * rgb[2];
    if luma > 0.22 {
        return false;
    }
    local_luma_var(img, mx, my, mw, mh) < 0.00022
}

/// Dark water — including splash-punctured patches (bright speckles raise variance
/// so plain flat-dark checks miss the classic left “red box”).
/// Uses **linear** luma (same as `is_flat_dark_region`); encoded sRGB makes dark
/// water look like mid-tones and silently disables this detector.
fn is_dark_water_like(img: &RgbF32Buf, mx: usize, my: usize, mw: usize, mh: usize) -> bool {
    if is_flat_dark_region(img, mx, my, mw, mh) {
        return true;
    }
    let mut dark_n = 0u32;
    let mut bright_n = 0u32;
    let mut mid_n = 0u32;
    let mut dark_sum = 0.0f32;
    let mut blue_led = 0u32;
    for dy in -2i32..=2 {
        for dx in -2i32..=2 {
            let x = mx as i32 + dx;
            let y = my as i32 + dy;
            if x < 0 || y < 0 || x as usize >= mw || y as usize >= mh {
                continue;
            }
            let rgb = sample_img_rgb(img, x as usize, y as usize, mw, mh);
            let luma = 0.2627 * rgb[0] + 0.678 * rgb[1] + 0.0593 * rgb[2];
            if luma < 0.14 {
                dark_n += 1;
                dark_sum += luma;
                if rgb[2] > rgb[0] + 0.02 && rgb[2] > rgb[1] + 0.01 {
                    blue_led += 1;
                }
            } else if luma > 0.35 {
                bright_n += 1;
            } else {
                mid_n += 1;
            }
        }
    }
    let n = dark_n + bright_n + mid_n;
    if n < 16 || dark_n < 10 {
        return false;
    }
    let dark_mean = dark_sum / dark_n as f32;
    // Mostly dark with optional splash speckles; reject feather-like mid tones.
    dark_mean < 0.10
        && mid_n as f32 / (n as f32) < 0.35
        && bright_n as f32 / (n as f32) < 0.40
        && (blue_led as f32 / (dark_n as f32) > 0.12 || bright_n >= 1 || dark_mean < 0.07)
}

fn wildlife_is_accent(r: f32, g: f32, b: f32) -> bool {
    let er = enc(r);
    let eg = enc(g);
    let eb = enc(b);
    let luma = 0.299 * er + 0.587 * eg + 0.114 * eb;
    let chroma = er.max(eg).max(eb) - er.min(eg).min(eb);
    let white = luma > 0.52 && chroma < 0.2;
    let warm = er > eg + 0.04 && er > eb + 0.06 && luma > 0.18 && luma < 0.9;
    white || warm
}

/// Test-only leftover: peel water boxes with plumage-colored heuristics.
/// Not used on the live subject path (`refine_saliency_with_image`).
fn refine_wildlife_on_water(data: &mut [f32], w: usize, h: usize, img: &RgbF32Buf) {
    if w * h == 0 {
        return;
    }
    let core_n = data.iter().filter(|v| **v > 0.55).count();
    if core_n < 24 {
        return;
    }
    let before = data.to_vec();

    let mut labels = vec![0u32; w * h];
    let mut areas: Vec<u32> = Vec::new();
    let mut stack = Vec::new();
    const T: f32 = 0.28;

    let relabel = |data: &[f32], labels: &mut [u32], areas: &mut Vec<u32>, stack: &mut Vec<usize>| {
        labels.fill(0);
        areas.clear();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if data[i] < T || labels[i] != 0 {
                    continue;
                }
                let id = (areas.len() + 1) as u32;
                let mut area = 0u32;
                stack.clear();
                stack.push(i);
                labels[i] = id;
                while let Some(j) = stack.pop() {
                    area += 1;
                    let jx = j % w;
                    let jy = j / w;
                    for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                        let nx = jx as i32 + dx;
                        let ny = jy as i32 + dy;
                        if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                            continue;
                        }
                        let k = ny as usize * w + nx as usize;
                        if labels[k] == 0 && data[k] >= T {
                            labels[k] = id;
                            stack.push(k);
                        }
                    }
                }
                areas.push(area);
            }
        }
    };

    relabel(data, &mut labels, &mut areas, &mut stack);
    if areas.is_empty() {
        return;
    }

    let score_blobs = |data: &[f32], labels: &[u32], nblob: usize| {
        let mut hard_mass = vec![0.0f32; nblob];
        let mut bird_mass = vec![0.0f32; nblob];
        let mut accent_mass = vec![0.0f32; nblob];
        let mut flat_dark_n = vec![0u32; nblob];
        let mut sum_x = vec![0.0f32; nblob];
        let mut sum_y = vec![0.0f32; nblob];
        let mut bx0s = vec![w; nblob];
        let mut bx1s = vec![0usize; nblob];
        let mut by0s = vec![h; nblob];
        let mut by1s = vec![0usize; nblob];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let id = labels[i];
                if id == 0 {
                    continue;
                }
                let bi = (id - 1) as usize;
                let v = data[i];
                if v >= 0.6 {
                    hard_mass[bi] += v;
                }
                sum_x[bi] += x as f32;
                sum_y[bi] += y as f32;
                bx0s[bi] = bx0s[bi].min(x);
                bx1s[bi] = bx1s[bi].max(x);
                by0s[bi] = by0s[bi].min(y);
                by1s[bi] = by1s[bi].max(y);
                let rgb = sample_img_rgb(img, x, y, w, h);
                let bird = wildlife_color_score(rgb[0], rgb[1], rgb[2]);
                let water = water_or_splash_score(rgb[0], rgb[1], rgb[2]);
                let splash = is_splash_speckle(img, x, y, w, h);
                let flat = is_dark_water_like(img, x, y, w, h);
                if flat {
                    flat_dark_n[bi] += 1;
                }
                if wildlife_is_accent(rgb[0], rgb[1], rgb[2]) && !splash {
                    accent_mass[bi] += v * bird.max(0.4);
                }
                if bird > water + 0.04 && bird > 0.18 && !splash {
                    bird_mass[bi] += v * bird;
                }
            }
        }
        (
            hard_mass, bird_mass, accent_mass, flat_dark_n, sum_x, sum_y, bx0s, bx1s, by0s, by1s,
        )
    };

    let pick_primary = |hard: &[f32], bird: &[f32], accent: &[f32]| {
        1 + (0..hard.len())
            .max_by(|&a, &b| {
                let sa = accent[a] * 8.0 + bird[a] * 2.0 + hard[a];
                let sb = accent[b] * 8.0 + bird[b] * 2.0 + hard[b];
                sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(0)
    };

    let (
        hard_mass,
        bird_mass,
        accent_mass,
        flat_dark_n,
        sum_x,
        sum_y,
        bx0s,
        bx1s,
        by0s,
        by1s,
    ) = score_blobs(data, &labels, areas.len());
    let primary = pick_primary(&hard_mass, &bird_mass, &accent_mass) as u32;
    let p_idx = (primary - 1) as usize;
    if hard_mass.get(p_idx).copied().unwrap_or(0.0) < 8.0 {
        return;
    }
    let has_accent = accent_mass[p_idx] > 4.0;
    if !has_accent {
        if std::env::var_os("MERARAW_DEBUG_WILDLIFE").is_some() {
            eprintln!(
                "wildlife: skip no accent primary={p_idx} hard={} accent={}",
                hard_mass[p_idx], accent_mass[p_idx]
            );
        }
        return;
    }
    if std::env::var_os("MERARAW_DEBUG_WILDLIFE").is_some() {
        eprintln!(
            "wildlife: enter primary={p_idx} area={} hard={:.1} accent={:.1} flat={:.2}",
            areas[p_idx],
            hard_mass[p_idx],
            accent_mass[p_idx],
            flat_dark_n[p_idx] as f32 / areas[p_idx].max(1) as f32
        );
    }

    let p_cx = sum_x[p_idx] / areas[p_idx].max(1) as f32;
    let p_cy = sum_y[p_idx] / areas[p_idx].max(1) as f32;
    let p_bw = (bx1s[p_idx] + 1).saturating_sub(bx0s[p_idx]).max(1) as f32;
    let primary_flat_frac = flat_dark_n[p_idx] as f32 / areas[p_idx].max(1) as f32;

    // 1) Drop already-detached accent-poor side boxes.
    let mut changed = false;
    for bi in 0..areas.len() {
        if bi == p_idx {
            continue;
        }
        let area = areas[bi].max(1) as f32;
        let bw = (bx1s[bi] + 1).saturating_sub(bx0s[bi]).max(1) as f32;
        let bh = (by1s[bi] + 1).saturating_sub(by0s[bi]).max(1) as f32;
        let fill = area / (bw * bh);
        let cx = sum_x[bi] / area;
        let cy = sum_y[bi] / area;
        let dist = ((cx - p_cx).powi(2) + (cy - p_cy).powi(2)).sqrt() / (w.max(h) as f32);
        let flat_frac = flat_dark_n[bi] as f32 / area;
        let accent_poor = accent_mass[bi] < (hard_mass[bi].max(1.0) * 0.06).max(0.8);
        let lateral = (cx - p_cx).abs() > p_bw * 0.28 || dist > 0.07;
        let drop = accent_poor
            && lateral
            && (flat_frac > 0.28 && (fill > 0.40 || hard_mass[bi] < hard_mass[p_idx] * 0.65)
                || hard_mass[bi] < hard_mass[p_idx] * 0.55 && area < (areas[p_idx] as f32) * 0.6);
        if !drop {
            continue;
        }
        let id = (bi + 1) as u32;
        for i in 0..data.len() {
            if labels[i] == id {
                data[i] = 0.0;
                changed = true;
            }
        }
    }

    // 2) Cut dark-water paint *far from* accents (side boxes). Do NOT assume
    // beak is on the left — that wiped the dark back on right-facing puffins.
    let mut ax0 = w;
    let mut ax1 = 0usize;
    let mut ay0 = h;
    let mut ay1 = 0usize;
    let mut accent_n = 0u32;
    let mut warm_sx = 0.0f32;
    let mut warm_n = 0u32;
    let mut white_sx = 0.0f32;
    let mut white_n = 0u32;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if labels[i] != primary || data[i] < 0.5 {
                continue;
            }
            let rgb = sample_img_rgb(img, x, y, w, h);
            if !wildlife_is_accent(rgb[0], rgb[1], rgb[2]) {
                continue;
            }
            if is_splash_speckle(img, x, y, w, h) {
                continue;
            }
            accent_n += 1;
            ax0 = ax0.min(x);
            ax1 = ax1.max(x);
            ay0 = ay0.min(y);
            ay1 = ay1.max(y);
            let er = enc(rgb[0]);
            let eg = enc(rgb[1]);
            let eb = enc(rgb[2]);
            let luma = 0.299 * er + 0.587 * eg + 0.114 * eb;
            let chroma = er.max(eg).max(eb) - er.min(eg).min(eb);
            if er > eg + 0.04 && er > eb + 0.06 && luma > 0.18 && luma < 0.9 {
                warm_sx += x as f32;
                warm_n += 1;
            } else if luma > 0.52 && chroma < 0.2 {
                white_sx += x as f32;
                white_n += 1;
            }
        }
    }
    // Beak (warm) vs face/chest (white) → which way the bird faces.
    // Search a padded neighborhood around white accents so the orange beak is
    // included even when it failed the strict accent gate.
    let mut beak_sx = 0.0f32;
    let mut beak_n = 0u32;
    if accent_n >= 8 {
        let x0 = ax0.saturating_sub(36);
        let x1 = (ax1 + 36).min(w.saturating_sub(1));
        let y0 = ay0.saturating_sub(16);
        let y1 = (ay1 + 16).min(h.saturating_sub(1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let i = y * w + x;
                if data[i] < 0.35 && labels[i] != primary {
                    continue;
                }
                let rgb = sample_img_rgb(img, x, y, w, h);
                let er = enc(rgb[0]);
                let eg = enc(rgb[1]);
                let eb = enc(rgb[2]);
                let luma = 0.299 * er + 0.587 * eg + 0.114 * eb;
                // Looser beak gate — saturated orange/red at mask resolution.
                if er > eg + 0.025 && er > eb + 0.035 && luma > 0.22 && luma < 0.95 {
                    beak_sx += x as f32;
                    beak_n += 1;
                }
            }
        }
    }
    let facing_confident = warm_n >= 4 || beak_n >= 30;
    let facing_right = if warm_n >= 4 && white_n >= 4 {
        (warm_sx / warm_n as f32) + 1.0 >= (white_sx / white_n as f32)
    } else if warm_n >= 4 {
        (warm_sx / warm_n as f32) >= (ax0 + ax1) as f32 * 0.5
    } else if beak_n >= 30 && white_n >= 4 {
        (beak_sx / beak_n as f32) >= (white_sx / white_n as f32)
    } else if beak_n >= 30 {
        (beak_sx / beak_n as f32) >= (ax0 + ax1) as f32 * 0.5
    } else {
        (ax0 + ax1) as f32 * 0.5 > w as f32 * 0.48
    };
    // When beak/warm is weak, still peel the water side away from where
    // accents sit in the frame (right-half accents → peel right streaks).
    let peel_water_right = if facing_confident {
        facing_right
    } else {
        (ax0 + ax1) as f32 * 0.5 > w as f32 * 0.48
    };
    let peel_water_left = !peel_water_right;
    if std::env::var_os("MERARAW_DEBUG_WILDLIFE").is_some() {
        eprintln!(
            "wildlife: facing_right={facing_right} confident={facing_confident} peel_L={peel_water_left} peel_R={peel_water_right} warm_n={warm_n} white_n={white_n} beak_n={beak_n} accent=[{ax0}-{ax1},{ay0}-{ay1}]"
        );
    }
    // Otter / seal / gray fur: "white accents" cover the whole animal. The
    // pad-flood then fills a rectangle of water. Only run that path for birds
    // (warm beak) with a compact face patch.
    let p_span = p_bw * (by1s[p_idx] + 1).saturating_sub(by0s[p_idx]).max(1) as f32;
    let a_span = ((ax1 + 1).saturating_sub(ax0).max(1) * (ay1 + 1).saturating_sub(ay0).max(1)) as f32;
    let spread_fur = a_span > p_span * 0.68 && warm_n < 10;
    let bird_beak = warm_n >= 6 || beak_n >= 36;
    if spread_fur || !bird_beak {
        if std::env::var_os("MERARAW_DEBUG_WILDLIFE").is_some() {
            eprintln!(
                "wildlife: skip pad-flood spread={spread_fur} beak={bird_beak} a={a_span:.0} p={p_span:.0}"
            );
        }
        if mask_bbox_fill(data, w, h, 0.28) > 0.86 {
            data.copy_from_slice(&before);
        }
        return;
    }
    // Pad strategy:
    // - Water-heavy primary (side box fused in): pad from accents only.
    // - Clean accent blob (chest-only): large pad so dark body can be recovered.
    let primary_flat = flat_dark_n[p_idx] as f32 / areas[p_idx].max(1) as f32;
    let water_heavy = primary_flat > 0.35;
    let (union_x0, union_x1, union_y0, union_y1) = if water_heavy && accent_n >= 8 {
        (ax0, ax1, ay0, ay1)
    } else if accent_n >= 8 {
        (
            bx0s[p_idx].min(ax0),
            bx1s[p_idx].max(ax1),
            by0s[p_idx].min(ay0),
            by1s[p_idx].max(ay1),
        )
    } else {
        (bx0s[p_idx], bx1s[p_idx], by0s[p_idx], by1s[p_idx])
    };
    let base_w = (union_x1.saturating_sub(union_x0)).max(1) as f32;
    let base_h = (union_y1.saturating_sub(union_y0)).max(1) as f32;
    // Tight on the beak/water side so fused side boxes stay out. Wide on the
    // back so dark plumage (crown, scapulars, tail) can be recovered.
    let pad_face = if water_heavy {
        (base_w * 0.12).max(6.0) as usize
    } else {
        (base_w * 0.12).max(6.0) as usize
    };
    let pad_back = if water_heavy {
        (base_w * 0.85).max(22.0) as usize
    } else {
        (base_w * 1.25).max(32.0) as usize
    };
    let pad_y_up = if water_heavy {
        (base_h * 0.22).max(10.0).min(20.0) as usize
    } else {
        (base_h * 0.55).max(16.0) as usize
    };
    let pad_y_dn = if water_heavy {
        (base_h * 0.28).max(10.0) as usize
    } else {
        (base_h * 0.85).max(22.0) as usize
    };
    let (sx0, sx1) = if peel_water_right {
        (
            union_x0.saturating_sub(pad_back),
            (union_x1 + pad_face).min(w.saturating_sub(1)),
        )
    } else {
        (
            union_x0.saturating_sub(pad_face),
            (union_x1 + pad_back).min(w.saturating_sub(1)),
        )
    };
    let sy0 = union_y0.saturating_sub(pad_y_up);
    let sy1 = (union_y1 + pad_y_dn).min(h.saturating_sub(1));

    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if labels[i] != primary || data[i] < 0.22 {
                continue;
            }
            let rgb = sample_img_rgb(img, x, y, w, h);
            if wildlife_is_accent(rgb[0], rgb[1], rgb[2]) {
                continue;
            }
            if is_splash_speckle(img, x, y, w, h) {
                data[i] = 0.0;
                changed = true;
                continue;
            }
            if !is_dark_water_like(img, x, y, w, h) {
                continue;
            }
            // Only cut water well outside the body pad.
            let outside = x < sx0 || x > sx1 || y < sy0 || y > sy1;
            if outside {
                data[i] = 0.0;
                changed = true;
            }
        }
    }

    // Always continue to accent flood when we have accents — even if no side
    // box was cut. Otherwise right-facing splash birds keep a chest-only mask.
    let _ = primary_flat_frac;

    // 3) Relabel and keep only the accent-winning blob; drop the rest.
    relabel(data, &mut labels, &mut areas, &mut stack);
    if areas.is_empty() {
        return;
    }
    let (hard2, bird2, accent2, _, _, _, _, _, _, _) = score_blobs(data, &labels, areas.len());
    let primary = pick_primary(&hard2, &bird2, &accent2) as u32;
    let p_idx = (primary - 1) as usize;
    if hard2.get(p_idx).copied().unwrap_or(0.0) < 6.0 || accent2[p_idx] < 3.0 {
        return;
    }
    for bi in 0..areas.len() {
        if bi == p_idx {
            continue;
        }
        let accent_poor = accent2[bi] < (hard2[bi].max(1.0) * 0.08).max(1.0);
        if accent_poor {
            let id = (bi + 1) as u32;
            for i in 0..data.len() {
                if labels[i] == id {
                    data[i] = 0.0;
                }
            }
        }
    }

    // 4) Accent flood — recover dark plumage attached to white/beak core.
    // Seeds MUST be accents only. Neutral dark water scores as black plumage;
    // seeding on bird>0.28 re-locks every water box into the keep set.
    let mut keep = vec![false; w * h];
    let mut accent_dist = vec![u16::MAX; w * h];
    let mut queue: Vec<usize> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if labels[i] != primary || data[i] < 0.45 {
                continue;
            }
            if is_splash_speckle(img, x, y, w, h) {
                continue;
            }
            let rgb = sample_img_rgb(img, x, y, w, h);
            if !wildlife_is_accent(rgb[0], rgb[1], rgb[2]) {
                continue;
            }
            let bird = wildlife_color_score(rgb[0], rgb[1], rgb[2]);
            let water = water_or_splash_score(rgb[0], rgb[1], rgb[2]);
            if bird < water {
                continue;
            }
            keep[i] = true;
            accent_dist[i] = 0;
            queue.push(i);
        }
    }
    if queue.len() < 8 {
        if std::env::var_os("MERARAW_DEBUG_WILDLIFE").is_some() {
            eprintln!("wildlife: few seeds {}", queue.len());
        }
        keep_strongest_blobs(data, w, h, 0.28, 1);
        return;
    }
    if std::env::var_os("MERARAW_DEBUG_WILDLIFE").is_some() {
        eprintln!(
            "wildlife: seeds={} pad=[{sx0}-{sx1},{sy0}-{sy1}] {w}x{h} water_heavy={water_heavy}",
            queue.len()
        );
    }

    const MAX_ACCENT_DIST: u16 = 64;
    const MAX_FLAT_DIST: u16 = 40;
    let mut qi = 0usize;
    while qi < queue.len() {
        let i = queue[qi];
        qi += 1;
        let ad = accent_dist[i];
        if ad >= MAX_ACCENT_DIST {
            continue;
        }
        let x = i % w;
        let y = i / w;
        for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)]
        {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                continue;
            }
            let nx = nx as usize;
            let ny = ny as usize;
            let j = ny * w + nx;
            if is_splash_speckle(img, nx, ny, w, h) {
                continue;
            }
            let rgb = sample_img_rgb(img, nx, ny, w, h);
            let bird = wildlife_color_score(rgb[0], rgb[1], rgb[2]);
            let water = water_or_splash_score(rgb[0], rgb[1], rgb[2]);
            let flat = is_dark_water_like(img, nx, ny, w, h);
            let accent = wildlife_is_accent(rgb[0], rgb[1], rgb[2]);
            let v = data[j];
            let in_primary = labels[j] == primary;
            let in_pad = nx >= sx0 && nx <= sx1 && ny >= sy0 && ny <= sy1;
            let lin = 0.2627 * rgb[0] + 0.678 * rgb[1] + 0.0593 * rgb[2];
            let er = enc(rgb[0]);
            let eg = enc(rgb[1]);
            let eb = enc(rgb[2]);
            let blue_water = eb > er + 0.045 && eb > eg + 0.02 && lin < 0.22;
            // Never reset accent distance on newly found white/warm pixels — bright
            // splash foam would otherwise become new origins and flood the water.
            let nd = ad.saturating_add(1);
            if keep[j] && accent_dist[j] <= nd {
                continue;
            }
            if blue_water && !accent {
                continue;
            }
            if !in_pad && !accent {
                continue;
            }
            let dark_body = !blue_water
                && lin < 0.20
                && bird > 0.16
                && bird + 0.01 >= water
                && !is_splash_speckle(img, nx, ny, w, h);
            if flat && !accent && nd > MAX_FLAT_DIST {
                continue;
            }
            if water > bird + 0.10 && !accent && !dark_body {
                continue;
            }

            let ok = if accent && in_primary && v >= 0.35 {
                true
            } else if accent && in_pad && nd <= MAX_FLAT_DIST {
                true
            } else if dark_body && nd <= MAX_ACCENT_DIST {
                let back_side = if peel_water_right {
                    nx <= ax0 + 10
                } else {
                    nx + 10 >= ax1
                };
                if flat {
                    nd <= MAX_FLAT_DIST || (back_side && nd <= MAX_FLAT_DIST + 28)
                } else {
                    true
                }
            } else if in_primary && v >= 0.22 && !flat && bird > water + 0.02 && bird > 0.14 {
                true
            } else if in_pad && !flat && bird > 0.26 && bird > water && nd < 44 {
                true
            } else if !flat && bird > 0.34 && bird > water + 0.04 && nd < 40 {
                local_luma_var(img, nx, ny, w, h) > 0.00012
            } else {
                false
            };
            if !ok {
                continue;
            }
            keep[j] = true;
            accent_dist[j] = nd;
            if v < 0.45 {
                data[j] = (0.5 + 0.45 * bird).clamp(0.5, 0.92);
            }
            queue.push(j);
        }
    }

    // Snapshot primary confidence before we clear non-keep — reclaim needs it.
    let raw_mask = data.to_vec();

    for i in 0..data.len() {
        if !keep[i] {
            data[i] = 0.0;
        }
    }

    // Peel flat water outside the (tight) accent pad.
    if water_heavy {
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if data[i] < 0.28 {
                    continue;
                }
                let rgb = sample_img_rgb(img, x, y, w, h);
                if wildlife_is_accent(rgb[0], rgb[1], rgb[2]) {
                    continue;
                }
                if !is_dark_water_like(img, x, y, w, h) {
                    continue;
                }
                if x < sx0 || x > sx1 || y < sy0 || y > sy1 {
                    data[i] = 0.0;
                    keep[i] = false;
                }
            }
        }
    }

    // Reclaim high-confidence primary mask attached to the accent core. Dark
    // plumage is often flat-water-like in color space; U²-Net still marked it.
    {
        let mut queue: Vec<(usize, u32)> = Vec::new();
        for i in 0..keep.len() {
            if keep[i] {
                queue.push((i, 0));
            }
        }
        let mut qi = 0usize;
        while qi < queue.len() {
            let (i, depth) = queue[qi];
            qi += 1;
            if depth >= 56 {
                continue;
            }
            let x = i % w;
            let y = i / w;
            for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)]
            {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                    continue;
                }
                let nx = nx as usize;
                let ny = ny as usize;
                let j = ny * w + nx;
                if keep[j] {
                    continue;
                }
                if labels[j] != primary || raw_mask[j] < 0.38 {
                    continue;
                }
                let pad_slack = if water_heavy { 22usize } else { 32usize };
                if nx + pad_slack < sx0
                    || nx > sx1 + pad_slack
                    || ny + pad_slack < sy0
                    || ny > sy1 + pad_slack
                {
                    continue;
                }
                if is_splash_speckle(img, nx, ny, w, h) {
                    continue;
                }
                let rgb = sample_img_rgb(img, nx, ny, w, h);
                let er = enc(rgb[0]);
                let eg = enc(rgb[1]);
                let eb = enc(rgb[2]);
                let lin = 0.2627 * rgb[0] + 0.678 * rgb[1] + 0.0593 * rgb[2];
                if eb > er + 0.05 && eb > eg + 0.025 && lin < 0.20 {
                    continue;
                }
                // Water-heavy: don't reclaim flat paint on the water side of the
                // beak/face (left box when facing left, right streaks when facing right).
                if water_heavy && is_dark_water_like(img, nx, ny, w, h) {
                    let water_side = if peel_water_right {
                        nx > ax1 + 3
                    } else {
                        nx + 3 < ax0
                    };
                    let over_bird = nx + 4 >= ax0 && nx <= ax1 + 4;
                    let crown_band = ny + 14 >= ay0;
                    let above_subject = ny + 8 < ay0;
                    if water_side || (above_subject && !(over_bird && crown_band)) {
                        continue;
                    }
                }
                keep[j] = true;
                data[j] = raw_mask[j].max(0.55);
                queue.push((j, depth + 1));
            }
        }
        for i in 0..data.len() {
            if !keep[i] {
                data[i] = 0.0;
            }
        }
    }

    // Final orientation peel: kill flat water on the wrong side of the bird.
    {
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if data[i] < 0.28 {
                    continue;
                }
                let rgb = sample_img_rgb(img, x, y, w, h);
                if wildlife_is_accent(rgb[0], rgb[1], rgb[2]) {
                    continue;
                }
                if !is_dark_water_like(img, x, y, w, h) {
                    continue;
                }
                let water_side = if peel_water_right {
                    x > ax1 + 3
                } else {
                    x + 3 < ax0
                };
                let over_bird = x + 4 >= ax0 && x <= ax1 + 4;
                let crown_band = y + 14 >= ay0;
                let above_subject = y + 8 < ay0;
                let mut up = 0u32;
                let mut dn = 0u32;
                for dy in 1..=3 {
                    if y >= dy && data[(y - dy) * w + x] >= 0.28 {
                        up += 1;
                    }
                    if y + dy < h && data[(y + dy) * w + x] >= 0.28 {
                        dn += 1;
                    }
                }
                let mut horiz = 0u32;
                for dx in 1..=4 {
                    if x >= dx && data[y * w + (x - dx)] >= 0.28 {
                        horiz += 1;
                    }
                    if x + dx < w && data[y * w + (x + dx)] >= 0.28 {
                        horiz += 1;
                    }
                }
                let thin_finger = horiz >= 3 && up + dn <= 1;
                if water_side || (above_subject && !(over_bird && crown_band)) || thin_finger {
                    data[i] = 0.0;
                    keep[i] = false;
                }
            }
        }
    }

    // Grow dark back plumage on the non-water side. Skip water-heavy primaries —
    // reclaim already recovered U²-Net body; unrestricted flat grow reopens boxes.
    if !water_heavy {
        let mut queue: Vec<(usize, u32)> = Vec::new();
        for i in 0..keep.len() {
            if keep[i] {
                queue.push((i, 0));
            }
        }
        let mut qi = 0usize;
        while qi < queue.len() {
            let (i, depth) = queue[qi];
            qi += 1;
            if depth >= 32 {
                continue;
            }
            let x = i % w;
            let y = i / w;
            for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                    continue;
                }
                let nx = nx as usize;
                let ny = ny as usize;
                let j = ny * w + nx;
                if keep[j] {
                    continue;
                }
                if peel_water_right && nx > ax1 + 4 {
                    continue;
                }
                if peel_water_left && nx + 4 < ax0 {
                    continue;
                }
                if ny + 6 < ay0 || ny > ay1 + 28 {
                    continue;
                }
                if is_splash_speckle(img, nx, ny, w, h) {
                    continue;
                }
                let rgb = sample_img_rgb(img, nx, ny, w, h);
                let er = enc(rgb[0]);
                let eg = enc(rgb[1]);
                let eb = enc(rgb[2]);
                let lin = 0.2627 * rgb[0] + 0.678 * rgb[1] + 0.0593 * rgb[2];
                let blue = eb > er + 0.045 && eb > eg + 0.02 && lin < 0.22;
                if blue {
                    continue;
                }
                // Black feathers are often flat-dark-like; allow a limited hop.
                if is_dark_water_like(img, nx, ny, w, h) && (depth > 22 || lin > 0.11) {
                    continue;
                }
                let bird = wildlife_color_score(rgb[0], rgb[1], rgb[2]);
                let water = water_or_splash_score(rgb[0], rgb[1], rgb[2]);
                if lin > 0.14 || bird + 0.02 < water || bird < 0.18 {
                    continue;
                }
                keep[j] = true;
                data[j] = 0.58;
                queue.push((j, depth + 1));
            }
        }
    }

    // Short hop fill for dark plumage holes next to the silhouette (non-flat only).
    {
        let mut queue: Vec<(usize, u32)> = Vec::new();
        for i in 0..keep.len() {
            if keep[i] {
                queue.push((i, 0));
            }
        }
        let mut qi = 0usize;
        while qi < queue.len() {
            let (i, depth) = queue[qi];
            qi += 1;
            if depth >= 12 {
                continue;
            }
            let x = i % w;
            let y = i / w;
            for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                    continue;
                }
                let nx = nx as usize;
                let ny = ny as usize;
                let j = ny * w + nx;
                if keep[j] {
                    continue;
                }
                if !(nx >= sx0 && nx <= sx1 && ny >= sy0 && ny <= sy1) {
                    continue;
                }
                if is_splash_speckle(img, nx, ny, w, h) {
                    continue;
                }
                if is_dark_water_like(img, nx, ny, w, h) {
                    continue;
                }
                let rgb = sample_img_rgb(img, nx, ny, w, h);
                let er = enc(rgb[0]);
                let eg = enc(rgb[1]);
                let eb = enc(rgb[2]);
                let lin = 0.2627 * rgb[0] + 0.678 * rgb[1] + 0.0593 * rgb[2];
                if eb > er + 0.045 && eb > eg + 0.02 && lin < 0.22 {
                    continue;
                }
                let bird = wildlife_color_score(rgb[0], rgb[1], rgb[2]);
                let water = water_or_splash_score(rgb[0], rgb[1], rgb[2]);
                let accent = wildlife_is_accent(rgb[0], rgb[1], rgb[2]);
                let dark = lin < 0.16 && bird + 0.02 >= water && bird > 0.18;
                if !accent && !dark {
                    continue;
                }
                keep[j] = true;
                data[j] = (0.52 + 0.4 * bird).clamp(0.5, 0.9);
                queue.push((j, depth + 1));
            }
        }
    }

    // Fill small holes inside the silhouette (splash speckles punched the mask).
    for _ in 0..2 {
        let prev = data.to_vec();
        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                let i = y * w + x;
                if prev[i] >= 0.28 {
                    continue;
                }
                let mut n = 0u32;
                let mut s = 0.0f32;
                for (dx, dy) in [
                    (-1i32, 0),
                    (1, 0),
                    (0, -1),
                    (0, 1),
                    (-1, -1),
                    (1, -1),
                    (-1, 1),
                    (1, 1),
                ] {
                    let j = (y as i32 + dy) as usize * w + (x as i32 + dx) as usize;
                    if prev[j] >= 0.32 {
                        n += 1;
                        s += prev[j];
                    }
                }
                if n >= 4 {
                    data[i] = (s / n as f32).clamp(0.4, 0.9);
                    keep[i] = true;
                }
            }
        }
    }

    // Peel thin horizontal water fingers / streak bars off the silhouette.
    {
        let prev = data.to_vec();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if prev[i] < 0.28 {
                    continue;
                }
                if !is_dark_water_like(img, x, y, w, h) {
                    continue;
                }
                let rgb = sample_img_rgb(img, x, y, w, h);
                if wildlife_is_accent(rgb[0], rgb[1], rgb[2]) {
                    continue;
                }
                // Count vertical support in this column.
                let mut up = 0u32;
                let mut dn = 0u32;
                for dy in 1..=4 {
                    if y >= dy && prev[(y - dy) * w + x] >= 0.28 {
                        up += 1;
                    }
                    if y + dy < h && prev[(y + dy) * w + x] >= 0.28 {
                        dn += 1;
                    }
                }
                let mut horiz = 0u32;
                for dx in 1..=5 {
                    if x >= dx && prev[y * w + (x - dx)] >= 0.28 {
                        horiz += 1;
                    }
                    if x + dx < w && prev[y * w + (x + dx)] >= 0.28 {
                        horiz += 1;
                    }
                }
                // Thin horizontal streak into water: wide horiz, little vertical.
                // Ignore bird score — neutral water looks like black plumage.
                if horiz >= 4 && up + dn <= 2 {
                    data[i] = 0.0;
                }
            }
        }
    }

    keep_wildlife_subject_blob(data, w, h, img, 0.28);

    // Pad-flood + hole-fill can paint an axis-aligned water rectangle (otter
    // regression). If we boxed the subject, keep the pre-wildlife silhouette.
    if mask_bbox_fill(data, w, h, 0.28) > 0.86 {
        data.copy_from_slice(&before);
        return;
    }

    let blurred = box_blur_3x3(data, w, h);
    for i in 0..data.len() {
        if data[i] > 0.15 {
            data[i] = (data[i] * 0.75 + blurred[i] * 0.25).clamp(0.0, 1.0);
        }
        if data[i] < 0.08 {
            data[i] = 0.0;
        }
    }
}

struct BlobStats {
    area: u32,
    sum: f32,
    sum_x: f32,
    sum_y: f32,
    min_y: usize,
    max_y: usize,
}

/// Drop weak detached blobs and water/glass reflections under the primary subject.
pub fn suppress_reflections_and_islands(data: &mut [f32], w: usize, h: usize) {
    if w * h == 0 {
        return;
    }
    const T: f32 = 0.32;
    let mut labels = vec![0u32; w * h];
    let mut blobs: Vec<BlobStats> = Vec::new();
    let mut stack = Vec::new();

    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if data[i] < T || labels[i] != 0 {
                continue;
            }
            let id = (blobs.len() + 1) as u32;
            let mut st = BlobStats {
                area: 0,
                sum: 0.0,
                sum_x: 0.0,
                sum_y: 0.0,
                min_y: y,
                max_y: y,
            };
            stack.clear();
            stack.push(i);
            labels[i] = id;
            while let Some(j) = stack.pop() {
                let jx = j % w;
                let jy = j / w;
                let v = data[j];
                st.area += 1;
                st.sum += v;
                st.sum_x += jx as f32;
                st.sum_y += jy as f32;
                st.min_y = st.min_y.min(jy);
                st.max_y = st.max_y.max(jy);
                for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = jx as i32 + dx;
                    let ny = jy as i32 + dy;
                    if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                        continue;
                    }
                    let k = ny as usize * w + nx as usize;
                    if labels[k] == 0 && data[k] >= T {
                        labels[k] = id;
                        stack.push(k);
                    }
                }
            }
            blobs.push(st);
        }
    }
    if blobs.is_empty() {
        return;
    }

    let primary = blobs
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            let sa = a.sum * (a.area as f32).sqrt();
            let sb = b.sum * (b.area as f32).sqrt();
            sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(0);
    let p = &blobs[primary];
    let p_mean = p.sum / p.area.max(1) as f32;
    let p_cy = p.sum_y / p.area.max(1) as f32;
    let p_bottom = p.max_y as f32;
    let min_keep = ((p.area as f32) * 0.08).max(12.0) as u32;

    let mut keep = vec![false; blobs.len()];
    keep[primary] = true;
    for (i, b) in blobs.iter().enumerate() {
        if i == primary {
            continue;
        }
        let mean = b.sum / b.area.max(1) as f32;
        let cy = b.sum_y / b.area.max(1) as f32;
        // Tiny islands
        if b.area < min_keep {
            continue;
        }
        // Reflection heuristic: below the primary subject, weaker confidence,
        // and largely under the primary's bottom edge (water / glossy floor).
        // Do NOT treat lateral extremities (wings sticking sideways) as reflections.
        let p_cx = p.sum_x / p.area.max(1) as f32;
        let b_cx = b.sum_x / b.area.max(1) as f32;
        let lateral = (b_cx - p_cx).abs() > (w as f32) * 0.12;
        let below = cy > p_cy + (h as f32) * 0.03 && b.min_y as f32 >= p_bottom - (h as f32) * 0.04;
        let weaker = mean < p_mean * 0.96;
        if below && weaker && !lateral {
            continue;
        }
        // Detached mid-strength blobs far from primary centroid
        let dx = b.sum_x / b.area.max(1) as f32 - p.sum_x / p.area.max(1) as f32;
        let dy = cy - p_cy;
        let dist = (dx * dx + dy * dy).sqrt() / (w.max(h) as f32);
        if dist > 0.28 && mean < p_mean * 0.85 {
            continue;
        }
        keep[i] = true;
    }

    for (i, v) in data.iter_mut().enumerate() {
        let lab = labels[i];
        if lab == 0 {
            if *v < T {
                *v = 0.0;
            }
            continue;
        }
        let bi = (lab - 1) as usize;
        if !keep[bi] {
            *v = 0.0;
        } else if *v < T {
            // Soft fringe outside hard threshold on kept blobs: keep a little.
            *v *= 0.35;
        }
    }
}

/// Cut soft / water-colored mask that hangs below the solid subject silhouette.
/// Catches reflections fused into the primary blob (otters, birds on water) that
/// blob-island suppression cannot see because they are still connected.
fn suppress_underside_water_bleed(data: &mut [f32], w: usize, h: usize, img: &RgbF32Buf) {
    if w * h == 0 {
        return;
    }
    // Per-column lowest solid core pixel.
    let mut col_bottom = vec![None::<usize>; w];
    let mut solid_n = 0u32;
    let mut solid_sum = [0.0f32; 3];
    for y in 0..h {
        for x in 0..w {
            if data[y * w + x] < 0.55 {
                continue;
            }
            col_bottom[x] = Some(y);
            let rgb = sample_img_rgb(img, x, y, w, h);
            solid_sum[0] += rgb[0];
            solid_sum[1] += rgb[1];
            solid_sum[2] += rgb[2];
            solid_n += 1;
        }
    }
    if solid_n < 8 {
        return;
    }
    let body = [
        solid_sum[0] / solid_n as f32,
        solid_sum[1] / solid_n as f32,
        solid_sum[2] / solid_n as f32,
    ];
    let bg = sample_outside_bg_mean(
        img,
        w,
        h,
        0,
        w.saturating_sub(1),
        0,
        col_bottom.iter().flatten().copied().max().unwrap_or(0),
    );

    let reach = ((h as f32) * 0.22).max(10.0) as usize;
    for x in 0..w {
        let Some(yb) = col_bottom[x] else {
            continue;
        };
        let solid_v = data[yb * w + x];
        let y_end = (yb + reach).min(h);
        for y in (yb + 1)..y_end {
            let i = y * w + x;
            let v = data[i];
            if v < 0.04 {
                continue;
            }
            let rgb = sample_img_rgb(img, x, y, w, h);
            let water = is_waterish_rgb(rgb[0], rgb[1], rgb[2]);
            let d_body = rgb_dist(rgb, body);
            let closer_bg = bg
                .map(|b| rgb_dist(rgb, b) + 0.02 < d_body)
                .unwrap_or(false);
            // Solid paw/limb dipping into water: keep high-confidence body-colored pixels.
            if v >= 0.78 && !water && !closer_bg && d_body < 0.18 {
                continue;
            }
            let soft = v < solid_v * 0.88 || v < 0.7;
            let unsupported = {
                let mut ok = false;
                for dy in 1..=2 {
                    if y >= dy && data[(y - dy) * w + x] >= 0.48 {
                        ok = true;
                        break;
                    }
                }
                !ok
            };
            if water || closer_bg || (soft && (unsupported || v < 0.55)) {
                data[i] = if water || closer_bg {
                    0.0
                } else {
                    v * 0.12
                };
            }
        }
        // Anything still hanging further down with no solid column support → clear.
        for y in y_end..h {
            let i = y * w + x;
            if data[i] > 0.04 {
                data[i] = 0.0;
            }
        }
    }
}

fn sample_enc_rgb(img: &RgbF32Buf, mx: usize, my: usize, mw: usize, mh: usize) -> [f32; 3] {
    let sx = ((mx as f32 + 0.5) * img.width as f32 / mw as f32) as usize;
    let sy = ((my as f32 + 0.5) * img.height as f32 / mh as f32) as usize;
    let i = (sy.min(img.height.saturating_sub(1)) * img.width + sx.min(img.width.saturating_sub(1)))
        * 3;
    [enc(img.data[i]), enc(img.data[i + 1]), enc(img.data[i + 2])]
}

fn is_skin_rgb(r: f32, g: f32, b: f32) -> f32 {
    // Encoded-RGB skin gate — covers light → deep skin, olive, and cool fill.
    if r < 0.05 || g < 0.03 || b < 0.015 {
        return 0.0;
    }
    // Skin is typically R ≥ G (or near). Overcast fill can lift B toward G
    // without becoming sky-blue (that still fails the B-vs-R check below).
    if r + 0.025 < g {
        return 0.0;
    }
    if b > r + 0.08 {
        return 0.0;
    }
    if g + 0.055 < b * 0.92 && r + 0.01 < b {
        return 0.0;
    }
    let rg = r - g;
    if rg < -0.02 || rg > 0.48 {
        return 0.0;
    }
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let chroma = mx - mn;
    if chroma < 0.018 || chroma > 0.65 {
        return 0.0;
    }
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    if !(0.05..=0.95).contains(&luma) {
        return 0.0;
    }
    // Deep skin: lower luma + modest chroma still valid when R leads.
    let deep = if luma < 0.28 {
        ((r - b) / 0.2).clamp(0.25, 1.0)
    } else {
        1.0
    };
    let rg_score = ((rg + 0.02) / 0.28).clamp(0.15, 1.0);
    let chroma_score = ((0.58 - chroma) / 0.45).clamp(0.2, 1.0);
    rg_score * chroma_score * deep
}

/// True when the subject region is mostly grayscale (B&W portraits).
fn subject_is_achromatic(subject: &Mask01, img: &RgbF32Buf) -> bool {
    let (w, h) = (subject.width, subject.height);
    let mut n = 0u32;
    let mut achro = 0u32;
    let step = ((w.max(h) / 64).max(1)) as usize;
    for y in (0..h).step_by(step) {
        for x in (0..w).step_by(step) {
            if subject.data[y * w + x] < 0.25 {
                continue;
            }
            let [r, g, b] = sample_enc_rgb(img, x, y, w, h);
            let chroma = r.max(g).max(b) - r.min(g).min(b);
            n += 1;
            if chroma < 0.045 {
                achro += 1;
            }
        }
    }
    n >= 40 && (achro as f32 / n as f32) > 0.82
}

fn is_hair_rgb(r: f32, g: f32, b: f32, y_norm: f32) -> f32 {
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let chroma = mx - mn;
    // Dark / brown / auburn / blonde — prefer upper subject, suppress vivid clothes.
    let dark = (1.0 - (luma / 0.55).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let blonde = ((luma - 0.35) / 0.4).clamp(0.0, 1.0) * (1.0 - chroma / 0.32).clamp(0.0, 1.0);
    let auburn = ((r - g.max(b)) / 0.22).clamp(0.0, 1.0) * ((0.55 - luma).max(0.0) / 0.4 + 0.3);
    let low_c = (1.0 - (chroma / 0.42).clamp(0.0, 1.0)).clamp(0.15, 1.0);
    let upper = (1.0 - y_norm * 1.25).clamp(0.0, 1.0);
    // Reject strong greens/blues (hats / foliage bleed).
    if g > r + 0.08 && g > b + 0.04 {
        return 0.0;
    }
    if b > r + 0.1 && b > g + 0.06 {
        return 0.0;
    }
    let tone = dark.max(blonde * 0.75).max(auburn * 0.7);
    tone * low_c * upper
}

/// Soft blur + island cleanup for people-part masks.
fn soft_cleanup_part(data: &mut [f32], w: usize, h: usize) {
    let blurred = box_blur_3x3(data, w, h);
    for i in 0..data.len() {
        data[i] = (data[i] * 0.55 + blurred[i] * 0.45).clamp(0.0, 1.0);
        if data[i] < 0.06 {
            data[i] = 0.0;
        }
    }
    suppress_reflections_and_islands(data, w, h);
}

/// Face-specific cleanup: lower floor + keep strongest blobs (no reflection
/// heuristic — that path assumes a body-over-water layout and wipes faces).
fn soft_cleanup_face(data: &mut [f32], w: usize, h: usize) {
    let blurred = box_blur_3x3(data, w, h);
    for i in 0..data.len() {
        data[i] = (data[i] * 0.4 + blurred[i] * 0.6).clamp(0.0, 1.0);
        if data[i] < 0.04 {
            data[i] = 0.0;
        }
    }
    keep_strongest_blobs(data, w, h, 0.10, 2);
}

/// If refine painted a box (high bbox fill) that the prior did not already
/// have, restore the pre-refine silhouette. Does not punish an already-boxy
/// U²-Net prior that we only slightly improved.
fn revert_if_boxed(data: &mut [f32], prior: &[f32], w: usize, h: usize) -> bool {
    let fill = mask_bbox_fill(data, w, h, 0.28);
    if fill <= 0.86 {
        return false;
    }
    let prior_fill = mask_bbox_fill(prior, w, h, 0.28);
    if fill > prior_fill + 0.04 {
        data.copy_from_slice(prior);
        return true;
    }
    false
}

/// Fraction of the mask's bounding box that is actually on. ~1.0 = a painted rectangle.
fn mask_bbox_fill(data: &[f32], w: usize, h: usize, thr: f32) -> f32 {
    let mut x0 = w;
    let mut x1 = 0usize;
    let mut y0 = h;
    let mut y1 = 0usize;
    let mut n = 0u32;
    for y in 0..h {
        for x in 0..w {
            if data[y * w + x] < thr {
                continue;
            }
            n += 1;
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
    }
    if n < 8 {
        return 0.0;
    }
    let area = ((x1 + 1).saturating_sub(x0)).max(1) * ((y1 + 1).saturating_sub(y0)).max(1);
    n as f32 / area as f32
}

/// Prefer the blob that looks like a bird (accents + plumage) over a larger water box.
fn keep_wildlife_subject_blob(
    data: &mut [f32],
    w: usize,
    h: usize,
    img: &RgbF32Buf,
    thr: f32,
) {
    if w * h == 0 {
        return;
    }
    let mut labels = vec![0u32; w * h];
    let mut scores: Vec<(u32, f32)> = Vec::new();
    let mut stack = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if data[i] < thr || labels[i] != 0 {
                continue;
            }
            let id = (scores.len() + 1) as u32;
            let mut accent = 0.0f32;
            let mut bird = 0.0f32;
            let mut water = 0.0f32;
            stack.clear();
            stack.push(i);
            labels[i] = id;
            while let Some(j) = stack.pop() {
                let jx = j % w;
                let jy = j / w;
                let rgb = sample_img_rgb(img, jx, jy, w, h);
                let a = if wildlife_is_accent(rgb[0], rgb[1], rgb[2]) {
                    1.0
                } else {
                    0.0
                };
                accent += a;
                bird += wildlife_color_score(rgb[0], rgb[1], rgb[2]);
                water += water_or_splash_score(rgb[0], rgb[1], rgb[2]);
                for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = jx as i32 + dx;
                    let ny = jy as i32 + dy;
                    if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                        continue;
                    }
                    let k = ny as usize * w + nx as usize;
                    if labels[k] == 0 && data[k] >= thr {
                        labels[k] = id;
                        stack.push(k);
                    }
                }
            }
            // Accents (beak/eye/white) beat a larger water rectangle.
            scores.push((id, accent * 18.0 + bird - water * 0.85));
        }
    }
    if scores.is_empty() {
        return;
    }
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let keep = scores[0].0;
    for i in 0..data.len() {
        if labels[i] != 0 && labels[i] != keep {
            data[i] = 0.0;
        }
    }
}

/// Keep the top-N connected components by mass; zero the rest.
fn keep_strongest_blobs(data: &mut [f32], w: usize, h: usize, thr: f32, keep_n: usize) {
    if w * h == 0 || keep_n == 0 {
        return;
    }
    let mut labels = vec![0u32; w * h];
    let mut masses: Vec<(u32, f32)> = Vec::new(); // (label, mass)
    let mut stack = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if data[i] < thr || labels[i] != 0 {
                continue;
            }
            let id = (masses.len() + 1) as u32;
            let mut mass = 0.0f32;
            stack.clear();
            stack.push(i);
            labels[i] = id;
            while let Some(j) = stack.pop() {
                mass += data[j];
                let jx = j % w;
                let jy = j / w;
                for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = jx as i32 + dx;
                    let ny = jy as i32 + dy;
                    if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                        continue;
                    }
                    let k = ny as usize * w + nx as usize;
                    if labels[k] == 0 && data[k] >= thr {
                        labels[k] = id;
                        stack.push(k);
                    }
                }
            }
            masses.push((id, mass));
        }
    }
    if masses.is_empty() {
        return;
    }
    masses.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let keep: std::collections::HashSet<u32> =
        masses.iter().take(keep_n).map(|(id, _)| *id).collect();
    for i in 0..data.len() {
        if labels[i] != 0 && !keep.contains(&labels[i]) {
            data[i] = 0.0;
        }
    }
}

/// Restrict a subject mask to skin-tone pixels.
pub fn extract_skin_mask(subject: &Mask01, img: &RgbF32Buf) -> Mask01 {
    let (w, h) = (subject.width, subject.height);
    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let s = subject.data[i];
            if s < 0.1 {
                continue;
            }
            let [r, g, b] = sample_enc_rgb(img, x, y, w, h);
            data[i] = (s * is_skin_rgb(r, g, b)).clamp(0.0, 1.0);
        }
    }
    soft_cleanup_part(&mut data, w, h);
    Mask01 {
        width: w,
        height: h,
        data,
    }
}

/// Restrict a subject mask to likely hair regions (upper / darker).
pub fn extract_hair_mask(subject: &Mask01, img: &RgbF32Buf) -> Mask01 {
    let (w, h) = (subject.width, subject.height);
    // Subject vertical extent for relative Y.
    let mut y0 = h;
    let mut y1 = 0usize;
    for y in 0..h {
        for x in 0..w {
            if subject.data[y * w + x] > 0.25 {
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    let span = (y1.saturating_sub(y0)).max(1) as f32;
    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let s = subject.data[i];
            if s < 0.12 {
                continue;
            }
            let y_norm = (y.saturating_sub(y0) as f32) / span;
            let [r, g, b] = sample_enc_rgb(img, x, y, w, h);
            // Hair should not also score strongly as skin.
            let skin = is_skin_rgb(r, g, b);
            let hair = is_hair_rgb(r, g, b, y_norm) * (1.0 - skin * 0.9);
            data[i] = (s * hair).clamp(0.0, 1.0);
        }
    }
    soft_cleanup_part(&mut data, w, h);
    Mask01 {
        width: w,
        height: h,
        data,
    }
}

fn subject_bbox(subject: &Mask01) -> (usize, usize, usize, usize) {
    let (w, h) = (subject.width, subject.height);
    let mut x0 = w;
    let mut x1 = 0usize;
    let mut y0 = h;
    let mut y1 = 0usize;
    for y in 0..h {
        for x in 0..w {
            if subject.data[y * w + x] > 0.22 {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    if x0 > x1 {
        (0, w.saturating_sub(1), 0, h.saturating_sub(1))
    } else {
        (x0, x1, y0, y1)
    }
}

/// Face ≈ detected people boxes ∪ upper subject ∩ skin ∪ head around eyes.
///
/// Skin-only fallback still requires real skin hits so a cool grey torso
/// does not become a face. Wildlife heads come from eye blobs, not skin.
pub fn extract_face_mask(subject: &Mask01, img: &RgbF32Buf) -> Mask01 {
    let (w, h) = (subject.width, subject.height);
    let mut data = vec![0.0f32; w * h];

    for face in crate::face_detect::detect_faces(img) {
        if face.width() * face.height() > 0.22 {
            // Huge boxes without skin are usually animals / torso false hits.
            let skin_frac = face_box_skin_fraction(img, face);
            if skin_frac < 0.12 {
                continue;
            }
        }
        paint_detected_face(&mut data, w, h, face, subject, img);
    }

    let skin = face_from_skin_heuristic(subject, img);
    for i in 0..data.len() {
        data[i] = data[i].max(skin.data[i]);
    }

    let eyes = find_eye_blobs(subject, img, img.width, img.height);
    let sx = w as f32 / img.width.max(1) as f32;
    let sy = h as f32 / img.height.max(1) as f32;
    let scaled: Vec<EyeBlob> = eyes
        .iter()
        .map(|e| EyeBlob {
            cx: e.cx * sx,
            cy: e.cy * sy,
            rx: e.rx * sx,
            ry: e.ry * sy,
            score: e.score,
        })
        .collect();
    paint_heads_around_eyes(&mut data, w, h, &scaled, subject, img);

    if data.iter().all(|v| *v < 0.08) {
        return Mask01 {
            width: w,
            height: h,
            data,
        };
    }
    soft_cleanup_face(&mut data, w, h);
    Mask01 {
        width: w,
        height: h,
        data,
    }
}

fn face_from_skin_heuristic(subject: &Mask01, img: &RgbF32Buf) -> Mask01 {
    let (w, h) = (subject.width, subject.height);
    let (x0, x1, y0, y1) = subject_bbox(subject);
    let span_y = (y1.saturating_sub(y0)).max(1) as f32;
    let span_x = (x1.saturating_sub(x0)).max(1) as f32;
    let cx = (x0 + x1) as f32 * 0.5;
    let bw = subject_is_achromatic(subject, img);
    let cy = if bw {
        y0 as f32 + span_y * 0.34
    } else {
        y0 as f32 + span_y * 0.28
    };
    let mut data = vec![0.0f32; w * h];
    let mut skin_hits = 0u32;
    for y in 0..h {
        let y_norm = (y.saturating_sub(y0) as f32) / span_y;
        if y_norm > 0.55 {
            continue;
        }
        for x in 0..w {
            let i = y * w + x;
            let s = subject.data[i];
            if s < 0.12 {
                continue;
            }
            let x_norm = (x as f32 - cx).abs() / (span_x * 0.5);
            let center = (1.0 - (x_norm - 0.1).max(0.0) / 0.95).clamp(0.15, 1.0);
            let upper = (1.0 - y_norm / 0.55).clamp(0.25, 1.0);
            let dx = (x as f32 - cx) / (span_x * 0.36).max(1.0);
            let dy = (y as f32 - cy) / (span_y * 0.30).max(1.0);
            let oval = (1.0 - (dx * dx + dy * dy) * 0.6).clamp(0.0, 1.0);
            let [r, g, b] = sample_enc_rgb(img, x, y, w, h);
            let mut skin = is_skin_rgb(r, g, b);
            if bw && skin < 0.16 {
                let luma = 0.299 * r + 0.587 * g + 0.114 * b;
                if (0.28..=0.82).contains(&luma) && oval > 0.35 {
                    skin = ((0.72 - (luma - 0.5).abs()) / 0.5).clamp(0.2, 0.7) * oval;
                }
            }
            if skin < 0.16 {
                continue;
            }
            skin_hits += 1;
            let prior = (upper * center.max(oval)).max(0.35);
            data[i] = (s * (0.25 + 0.75 * skin) * prior).clamp(0.0, 1.0);
        }
    }
    let upper_area = ((span_y * 0.55) * span_x).max(1.0);
    let min_hits = if bw {
        (upper_area * 0.008).max(8.0)
    } else {
        (upper_area * 0.012).max(12.0)
    };
    if (skin_hits as f32) < min_hits {
        return Mask01 {
            width: w,
            height: h,
            data: vec![0.0; w * h],
        };
    }
    Mask01 {
        width: w,
        height: h,
        data,
    }
}

fn face_box_skin_fraction(img: &RgbF32Buf, face: crate::face_detect::FaceBox) -> f32 {
    let (w, h) = (img.width.max(1), img.height.max(1));
    let x0 = (face.x0 * w as f32) as usize;
    let y0 = (face.y0 * h as f32) as usize;
    let x1 = (face.x1 * w as f32).min(w as f32) as usize;
    let y1 = (face.y1 * h as f32).min(h as f32) as usize;
    if x1 <= x0 || y1 <= y0 {
        return 0.0;
    }
    let step = ((x1 - x0).max(y1 - y0) / 16).max(1);
    let mut n = 0u32;
    let mut skin = 0u32;
    for y in (y0..y1).step_by(step) {
        for x in (x0..x1).step_by(step) {
            n += 1;
            let i = (y * w + x) * 3;
            let (r, g, b) = (enc(img.data[i]), enc(img.data[i + 1]), enc(img.data[i + 2]));
            if is_skin_rgb(r, g, b) > 0.16 {
                skin += 1;
            }
        }
    }
    if n == 0 {
        0.0
    } else {
        skin as f32 / n as f32
    }
}

fn paint_detected_face(
    data: &mut [f32],
    w: usize,
    h: usize,
    face: crate::face_detect::FaceBox,
    subject: &Mask01,
    img: &RgbF32Buf,
) {
    let pad_x = face.width() * 0.08;
    let pad_y = face.height() * 0.10;
    let x0 = ((face.x0 - pad_x) * w as f32).floor().max(0.0) as usize;
    let y0 = ((face.y0 - pad_y) * h as f32).floor().max(0.0) as usize;
    let x1 = ((face.x1 + pad_x) * w as f32).ceil().min(w as f32) as usize;
    let y1 = ((face.y1 + pad_y) * h as f32).ceil().min(h as f32) as usize;
    let cx = face.cx() * w as f32;
    let cy = face.cy() * h as f32;
    let rx = (face.width() * w as f32 * 0.52).max(2.0);
    let ry = (face.height() * h as f32 * 0.56).max(2.0);
    for y in y0..y1 {
        for x in x0..x1 {
            let nx = (x as f32 - cx) / rx;
            let ny = (y as f32 - cy) / ry;
            let d = nx * nx + ny * ny;
            if d > 1.05 {
                continue;
            }
            let i = y * w + x;
            let [r, g, b] = sample_enc_rgb(img, x, y, w, h);
            let skin = is_skin_rgb(r, g, b);
            let luma = 0.299 * r + 0.587 * g + 0.114 * b;
            let sub = subject.data.get(i).copied().unwrap_or(0.0);
            let inside = (1.0 - d * 0.45).clamp(0.2, 1.0);
            let boost = skin.max(0.35).max(if (0.12..=0.92).contains(&luma) {
                0.45
            } else {
                0.2
            });
            data[i] = data[i].max((inside * boost * (0.55 + 0.45 * sub.max(0.4))).clamp(0.0, 1.0));
        }
    }
}

fn is_lip_rgb(r: f32, g: f32, b: f32) -> f32 {
    if r < 0.12 {
        return 0.0;
    }
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    if !(0.1..=0.78).contains(&luma) {
        return 0.0;
    }
    let red = ((r - g.max(b)) / 0.26).clamp(0.0, 1.0);
    let warm = ((r - b) / 0.32).clamp(0.0, 1.0) * ((r - g) / 0.2).clamp(0.0, 1.0);
    // Natural lips are warmer than surrounding skin, not neon.
    if r < g * 0.98 && red < 0.15 {
        return 0.0;
    }
    red.max(warm * 0.75) * ((0.68 - (luma - 0.38).abs()) / 0.4).clamp(0.2, 1.0)
}

fn is_eye_rgb(r: f32, g: f32, b: f32) -> f32 {
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let chroma = mx - mn;
    // Iris / pupil: dark low-chroma; sclera: bright low-chroma; blue / amber irises.
    let dark = (1.0 - luma / 0.42).clamp(0.0, 1.0) * (1.0 - chroma / 0.28).clamp(0.15, 1.0);
    let sclera = ((luma - 0.52) / 0.38).clamp(0.0, 1.0) * (1.0 - chroma / 0.22).clamp(0.0, 1.0);
    let blue_iris = ((b - r) / 0.2).clamp(0.0, 1.0) * ((0.45 - luma).max(0.0) / 0.35 + 0.2);
    let amber = ((r.max(g) - b) / 0.28).clamp(0.0, 1.0)
        * (1.0 - (r - g).abs() / 0.22).clamp(0.2, 1.0)
        * ((0.52 - luma).max(0.0) / 0.4 + 0.15);
    dark.max(sclera * 0.9)
        .max(blue_iris * 0.55)
        .max(amber * 0.7)
}

/// Lips ≈ lower-face skin band ∩ warm red.
pub fn extract_lips_mask(subject: &Mask01, img: &RgbF32Buf) -> Mask01 {
    let face = extract_face_mask(subject, img);
    let (w, h) = (face.width, face.height);
    let (_x0, _x1, fy0, fy1) = subject_bbox(&face);
    let span = (fy1.saturating_sub(fy0)).max(1) as f32;
    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        let y_norm = (y.saturating_sub(fy0) as f32) / span;
        // Lips sit in the lower third of the *face* band.
        if !(0.52..=0.88).contains(&y_norm) {
            continue;
        }
        for x in 0..w {
            let i = y * w + x;
            if face.data[i] < 0.08 {
                continue;
            }
            let [r, g, b] = sample_enc_rgb(img, x, y, w, h);
            data[i] = (face.data[i] * is_lip_rgb(r, g, b)).clamp(0.0, 1.0);
        }
    }
    soft_cleanup_part(&mut data, w, h);
    Mask01 {
        width: w,
        height: h,
        data,
    }
}

fn subject_at(subject: &Mask01, x: usize, y: usize, w: usize, h: usize) -> f32 {
    if subject.width == 0 || subject.height == 0 {
        return 0.0;
    }
    if subject.width == w && subject.height == h {
        return subject.data.get(y * w + x).copied().unwrap_or(0.0);
    }
    let sx = ((x as f32 + 0.5) * subject.width as f32 / w.max(1) as f32) as usize;
    let sy = ((y as f32 + 0.5) * subject.height as f32 / h.max(1) as f32) as usize;
    subject.data.get(
        sy.min(subject.height.saturating_sub(1)) * subject.width + sx.min(subject.width.saturating_sub(1)),
    )
    .copied()
    .unwrap_or(0.0)
}

fn subject_near(subject: &Mask01, x: usize, y: usize, w: usize, h: usize, radius: i32) -> f32 {
    let mut best = subject_at(subject, x, y, w, h);
    if best > 0.2 || radius <= 0 {
        return best;
    }
    for (dx, dy) in [
        (-radius, 0),
        (radius, 0),
        (0, -radius),
        (0, radius),
        (-radius, -radius),
        (radius, -radius),
        (-radius, radius),
        (radius, radius),
    ] {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
            continue;
        }
        best = best.max(subject_at(subject, nx as usize, ny as usize, w, h));
    }
    best
}

/// Eyes ≈ UltraFace eye bands ∪ dark/amber contrast blobs on the subject.
pub fn extract_eyes_mask(subject: &Mask01, img: &RgbF32Buf) -> Mask01 {
    let (w, h) = (img.width, img.height);
    let mut data = vec![0.0f32; w * h];
    for face in crate::face_detect::detect_faces(img) {
        paint_human_eye_pair(&mut data, w, h, face, img);
    }
    for blob in find_eye_blobs(subject, img, w, h) {
        paint_ellipse(
            &mut data,
            w,
            h,
            blob.cx,
            blob.cy,
            blob.rx * 1.15,
            blob.ry * 1.15,
            blob.score.clamp(0.45, 1.0),
        );
    }
    let blurred = box_blur_3x3(&data, w, h);
    for i in 0..data.len() {
        data[i] = (data[i] * 0.65 + blurred[i] * 0.35).clamp(0.0, 1.0);
        if data[i] < 0.05 {
            data[i] = 0.0;
        }
    }
    keep_strongest_blobs(&mut data, w, h, 0.10, 4);
    Mask01 {
        width: w,
        height: h,
        data,
    }
}

#[derive(Clone, Copy)]
struct EyeBlob {
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    score: f32,
}

fn mask_coverage(subject: &Mask01, thr: f32) -> f32 {
    if subject.data.is_empty() {
        return 0.0;
    }
    subject.data.iter().filter(|v| **v > thr).count() as f32 / subject.data.len() as f32
}

fn find_eye_blobs(subject: &Mask01, img: &RgbF32Buf, w: usize, h: usize) -> Vec<EyeBlob> {
    if w < 12 || h < 12 {
        return Vec::new();
    }
    let cov = mask_coverage(subject, 0.22);
    if cov < 0.002 {
        return Vec::new();
    }
    let loose_subject = cov > 0.42;
    let min_contrast = if loose_subject { 0.16 } else { 0.08 };
    let mut heat = vec![0.0f32; w * h];
    let (sx0, sx1, sy0, sy1) = subject_bbox(subject);
    let scale_x = w as f32 / subject.width.max(1) as f32;
    let scale_y = h as f32 / subject.height.max(1) as f32;
    let sub_w = ((sx1.saturating_sub(sx0)) as f32 * scale_x).max(1.0);
    let sub_h = ((sy1.saturating_sub(sy0)) as f32 * scale_y).max(1.0);
    let pad_x = (sub_w * 0.28).max(12.0);
    let pad_y = (sub_h * 0.28).max(12.0);
    let x_lo = ((sx0 as f32 * scale_x) - pad_x).max(2.0) as usize;
    let y_lo = ((sy0 as f32 * scale_y) - pad_y).max(2.0) as usize;
    let x_hi = ((sx1 as f32 * scale_x) + pad_x).min((w.saturating_sub(2)) as f32) as usize;
    let y_hi = ((sy1 as f32 * scale_y) + pad_y).min((h.saturating_sub(2)) as f32) as usize;

    for y in y_lo..y_hi {
        let y_norm = y as f32 / h.max(1) as f32;
        for x in x_lo..x_hi {
            let i = y * w + x;
            let sub = subject_near(subject, x, y, w, h, ((w.min(h) as f32) * 0.04) as i32);
            let [r, g, b] = sample_enc_rgb(img, x, y, w, h);
            if g > r + 0.07 && g > b + 0.04 {
                continue;
            }
            // Dark pupils look like water; keep them when the surround is much brighter.
            let luma = 0.299 * r + 0.587 * g + 0.114 * b;
            let mut sur = 0.0f32;
            let mut n = 0.0f32;
            for (dx, dy) in [
                (-2, 0),
                (2, 0),
                (0, -2),
                (0, 2),
                (-2, -2),
                (2, -2),
                (-2, 2),
                (2, 2),
            ] {
                let [sr, sg, sb] = sample_enc_rgb(
                    img,
                    (x as i32 + dx) as usize,
                    (y as i32 + dy) as usize,
                    w,
                    h,
                );
                sur += 0.299 * sr + 0.587 * sg + 0.114 * sb;
                n += 1.0;
            }
            let surround = sur / n.max(1.0);
            let contrast = (surround - luma).clamp(0.0, 1.0);
            if contrast < min_contrast {
                continue;
            }
            if water_score(r, g, b, y_norm) > 0.45 && contrast < 0.22 {
                continue;
            }
            let color = is_eye_rgb(r, g, b);
            let hole = contrast * (1.0 - luma / 0.55).clamp(0.25, 1.0);
            let on_face = if surround > 0.48 { 1.35 } else { 1.0 };
            heat[i] = (color.max(hole) * (0.3 + 0.7 * contrast) * sub.max(0.35) * on_face).clamp(0.0, 1.0);
        }
    }

    let thr = if loose_subject { 0.24 } else { 0.14 };
    let mut labels = vec![0u32; w * h];
    let mut blobs = Vec::new();
    let mut stack = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if heat[i] < thr || labels[i] != 0 {
                continue;
            }
            let id = (blobs.len() + 1) as u32;
            let mut mass = 0.0f32;
            let mut mx = 0.0f32;
            let mut my = 0.0f32;
            let mut x0 = x;
            let mut x1 = x;
            let mut y0 = y;
            let mut y1 = y;
            stack.clear();
            stack.push(i);
            labels[i] = id;
            while let Some(j) = stack.pop() {
                mass += heat[j];
                let jx = j % w;
                let jy = j / w;
                mx += jx as f32 * heat[j];
                my += jy as f32 * heat[j];
                x0 = x0.min(jx);
                x1 = x1.max(jx);
                y0 = y0.min(jy);
                y1 = y1.max(jy);
                for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = jx as i32 + dx;
                    let ny = jy as i32 + dy;
                    if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                        continue;
                    }
                    let k = ny as usize * w + nx as usize;
                    if labels[k] == 0 && heat[k] >= thr {
                        labels[k] = id;
                        stack.push(k);
                    }
                }
            }
            if mass < 0.35 {
                continue;
            }
            let bw = (x1 + 1).saturating_sub(x0).max(1) as f32;
            let bh = (y1 + 1).saturating_sub(y0).max(1) as f32;
            let area = bw * bh;
            let fill = mass / area.max(1.0);
            if fill < 0.10 {
                continue;
            }
            let max_dim = if loose_subject {
                (w.min(h) as f32) * 0.05
            } else {
                sub_w.min(sub_h) * 0.42
            };
            let min_dim = if loose_subject { 1.4 } else { 0.9 };
            if bw.min(bh) < min_dim || bw.max(bh) > max_dim.max(4.0) {
                continue;
            }
            let aspect = bw.max(bh) / bw.min(bh).max(1.0);
            if aspect > 2.4 {
                continue;
            }
            blobs.push(EyeBlob {
                cx: mx / mass.max(1e-5),
                cy: my / mass.max(1e-5),
                rx: (bw * 0.62).max(1.4),
                ry: (bh * 0.62).max(1.4),
                score: (mass * fill).clamp(0.3, 1.0),
            });
        }
    }
    if blobs.is_empty() {
        return blobs;
    }
    blobs.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    pick_eye_pair_or_best(&blobs, sub_w)
}

fn pick_eye_pair_or_best(blobs: &[EyeBlob], subject_w: f32) -> Vec<EyeBlob> {
    let limit = blobs.iter().take(6).cloned().collect::<Vec<_>>();
    let mut best_pair: Option<(f32, EyeBlob, EyeBlob)> = None;
    for i in 0..limit.len() {
        for j in (i + 1)..limit.len() {
            let a = limit[i];
            let b = limit[j];
            let dx = (a.cx - b.cx).abs();
            let dy = (a.cy - b.cy).abs();
            let size = (a.rx + b.rx) * 0.5;
            if dy > size * 1.8 {
                continue;
            }
            if dx < size * 1.4 || dx > subject_w.max(8.0) * 0.7 {
                continue;
            }
            let pair = a.score + b.score - dy * 0.02;
            if best_pair.map(|p| pair > p.0).unwrap_or(true) {
                best_pair = Some((pair, a, b));
            }
        }
    }
    if let Some((_, a, b)) = best_pair {
        return vec![a, b];
    }
    limit.into_iter().take(2).collect()
}

fn paint_ellipse(data: &mut [f32], w: usize, h: usize, cx: f32, cy: f32, rx: f32, ry: f32, peak: f32) {
    let rx = rx.max(1.0);
    let ry = ry.max(1.0);
    let x0 = (cx - rx).floor().max(0.0) as usize;
    let y0 = (cy - ry).floor().max(0.0) as usize;
    let x1 = ((cx + rx).ceil() as usize + 1).min(w);
    let y1 = ((cy + ry).ceil() as usize + 1).min(h);
    for y in y0..y1 {
        for x in x0..x1 {
            let nx = (x as f32 - cx) / rx;
            let ny = (y as f32 - cy) / ry;
            let d = nx * nx + ny * ny;
            if d <= 1.0 {
                let i = y * w + x;
                data[i] = data[i].max(peak * (1.0 - d * 0.4).clamp(0.2, 1.0));
            }
        }
    }
}

fn paint_human_eye_pair(
    data: &mut [f32],
    w: usize,
    h: usize,
    face: crate::face_detect::FaceBox,
    img: &RgbF32Buf,
) {
    for &xf in &[0.30f32, 0.70] {
        let wx0 = ((face.x0 + face.width() * (xf - 0.14)) * w as f32).max(0.0) as usize;
        let wx1 = ((face.x0 + face.width() * (xf + 0.14)) * w as f32).min(w as f32) as usize;
        let wy0 = ((face.y0 + face.height() * 0.24) * h as f32).max(0.0) as usize;
        let wy1 = ((face.y0 + face.height() * 0.50) * h as f32).min(h as f32) as usize;
        if wx1 <= wx0 || wy1 <= wy0 {
            continue;
        }
        let mut best = 0.0f32;
        let mut bx = (wx0 + wx1) as f32 * 0.5;
        let mut by = (wy0 + wy1) as f32 * 0.5;
        for y in wy0..wy1 {
            for x in wx0..wx1 {
                let [r, g, bcol] = sample_enc_rgb(img, x, y, w, h);
                let luma = 0.299 * r + 0.587 * g + 0.114 * bcol;
                let mut sur = 0.0f32;
                let mut n = 0.0f32;
                for (dx, dy) in [(-2i32, 0), (2, 0), (0, -2), (0, 2)] {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                        continue;
                    }
                    let [sr, sg, sb] = sample_enc_rgb(img, nx as usize, ny as usize, w, h);
                    sur += 0.299 * sr + 0.587 * sg + 0.114 * sb;
                    n += 1.0;
                }
                let contrast = (sur / n.max(1.0) - luma).clamp(0.0, 1.0);
                let s = is_eye_rgb(r, g, bcol) * (0.3 + 0.7 * contrast);
                if s > best {
                    best = s;
                    bx = x as f32;
                    by = y as f32;
                }
            }
        }
        if best < 0.12 {
            continue;
        }
        let rx = (face.width() * w as f32 * 0.09).max(1.6);
        let ry = (face.height() * h as f32 * 0.055).max(1.2);
        paint_ellipse(data, w, h, bx, by, rx, ry, best.clamp(0.5, 1.0));
    }
}

fn paint_heads_around_eyes(
    data: &mut [f32],
    w: usize,
    h: usize,
    eyes: &[EyeBlob],
    subject: &Mask01,
    img: &RgbF32Buf,
) {
    if eyes.is_empty() {
        return;
    }
    let (sx0, sx1, sy0, sy1) = subject_bbox(subject);
    let sub_w = (sx1.saturating_sub(sx0)).max(1) as f32;
    let sub_h = (sy1.saturating_sub(sy0)).max(1) as f32;
    let (cx, cy, rx, ry) = if eyes.len() >= 2 {
        let a = eyes[0];
        let b = eyes[1];
        let mid_x = (a.cx + b.cx) * 0.5;
        let mid_y = (a.cy + b.cy) * 0.5;
        let dx = (a.cx - b.cx).abs().max(6.0);
        (
            mid_x,
            mid_y + dx * 0.12,
            dx * 1.15,
            dx * 1.25,
        )
    } else {
        let e = eyes[0];
        (
            e.cx,
            e.cy,
            (e.rx * 6.5).min(sub_w * 0.28).max(4.0),
            (e.ry * 6.5).min(sub_h * 0.28).max(4.0),
        )
    };
    let x0 = (cx - rx).floor().max(0.0) as usize;
    let y0 = (cy - ry).floor().max(0.0) as usize;
    let x1 = ((cx + rx).ceil() as usize + 1).min(w);
    let y1 = ((cy + ry).ceil() as usize + 1).min(h);
    for y in y0..y1 {
        for x in x0..x1 {
            let nx = (x as f32 - cx) / rx.max(1.0);
            let ny = (y as f32 - cy) / ry.max(1.0);
            if nx * nx + ny * ny > 1.05 {
                continue;
            }
            let i = y * w + x;
            if subject.data[i] < 0.10 {
                continue;
            }
            let [r, g, b] = sample_enc_rgb(img, x, y, w, h);
            let luma = 0.299 * r + 0.587 * g + 0.114 * b;
            let skin = is_skin_rgb(r, g, b);
            let white_head = (luma - 0.55).clamp(0.0, 1.0) * (1.0 - (r.max(g).max(b) - r.min(g).min(b)) / 0.22).clamp(0.0, 1.0);
            let fur_face = (1.0 - (luma - 0.35).abs() / 0.4).clamp(0.0, 1.0);
            let hit = skin.max(white_head).max(fur_face * 0.45).max(0.28);
            data[i] = data[i].max((subject.data[i] * hit * (1.0 - (nx * nx + ny * ny) * 0.35)).clamp(0.0, 1.0));
        }
    }
}

fn mountain_score(r: f32, g: f32, b: f32, y_norm: f32) -> f32 {
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let chroma = mx - mn;
    // Mid/low chroma rock / distant ridges; prefer mid-frame (not sky, not foreground).
    if chroma > 0.3 || luma > 0.85 || luma < 0.06 {
        return 0.0;
    }
    // Reject vivid vegetation / water.
    if g > r + 0.06 && g > b + 0.04 {
        return 0.0;
    }
    if b > r + 0.08 && b > g * 0.95 && luma < 0.55 {
        return 0.0;
    }
    let rock = (1.0 - chroma / 0.3).clamp(0.0, 1.0) * ((luma - 0.12) / 0.55).clamp(0.0, 1.0);
    let band = (1.0 - ((y_norm - 0.4).abs() / 0.4)).clamp(0.0, 1.0);
    rock * band
}

fn architecture_score(r: f32, g: f32, b: f32, y_norm: f32) -> f32 {
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let chroma = mx - mn;
    // Man-made: neutrals / warm neutrals, mid luma, mid-lower frame; reject sky/veg.
    if y_norm < 0.12 || luma > 0.9 {
        return 0.0;
    }
    if g > r + 0.07 && g > b + 0.05 {
        return 0.0;
    }
    let neutral = (1.0 - chroma / 0.28).clamp(0.0, 1.0);
    let mid = (1.0 - ((luma - 0.42).abs() / 0.42)).clamp(0.0, 1.0);
    let band = ((y_norm - 0.12) / 0.75).clamp(0.0, 1.0);
    // Prefer slightly edge-rich looking midtones (buildings vs flat sky).
    let structure = (chroma / 0.12).clamp(0.35, 1.0);
    neutral * mid * band * structure
}

fn ground_score(r: f32, g: f32, b: f32, y_norm: f32) -> f32 {
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    // Earth / pavement / soil — lower frame, not vivid blue.
    if y_norm < 0.42 {
        return 0.0;
    }
    if b > r + 0.06 && b > g {
        return 0.0; // water/sky bleed
    }
    let green = g - r.max(b);
    let lower = ((y_norm - 0.42) / 0.58).clamp(0.0, 1.0);
    if green > 0.06 {
        // Soft grass as ground, but weaker than dedicated vegetation.
        return (green / 0.22).clamp(0.0, 1.0) * lower * 0.55;
    }
    let earth =
        ((r - b).max(0.0) / 0.28).clamp(0.0, 1.0) * (1.0 - (g - r).max(0.0) / 0.22).clamp(0.2, 1.0);
    let pavement = (1.0 - ((r - g).abs() + (g - b).abs()) / 0.2).clamp(0.0, 1.0)
        * (1.0 - ((luma - 0.4).abs() / 0.45)).clamp(0.15, 1.0);
    (earth.max(pavement * 0.85).max(0.2) * lower * ((0.7 - luma).max(0.0) / 0.55 + 0.25).clamp(0.0, 1.0))
        .clamp(0.0, 1.0)
}

pub fn extract_mountains_mask(img: &RgbF32Buf) -> Mask01 {
    let (w, h) = (img.width, img.height);
    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        let y_norm = y as f32 / h.max(1) as f32;
        for x in 0..w {
            let i = (y * w + x) * 3;
            data[y * w + x] = mountain_score(enc(img.data[i]), enc(img.data[i + 1]), enc(img.data[i + 2]), y_norm);
        }
    }
    refine_saliency_mask(&mut data, w, h);
    Mask01 { width: w, height: h, data }
}

pub fn extract_architecture_mask(img: &RgbF32Buf) -> Mask01 {
    let (w, h) = (img.width, img.height);
    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        let y_norm = y as f32 / h.max(1) as f32;
        for x in 0..w {
            let i = (y * w + x) * 3;
            data[y * w + x] =
                architecture_score(enc(img.data[i]), enc(img.data[i + 1]), enc(img.data[i + 2]), y_norm);
        }
    }
    refine_saliency_mask(&mut data, w, h);
    Mask01 { width: w, height: h, data }
}

pub fn extract_ground_mask(img: &RgbF32Buf) -> Mask01 {
    let (w, h) = (img.width, img.height);
    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        let y_norm = y as f32 / h.max(1) as f32;
        for x in 0..w {
            let i = (y * w + x) * 3;
            data[y * w + x] = ground_score(enc(img.data[i]), enc(img.data[i + 1]), enc(img.data[i + 2]), y_norm);
        }
    }
    refine_saliency_mask(&mut data, w, h);
    Mask01 { width: w, height: h, data }
}

/// Approximate depth range using vertical position (far=top … near=bottom).
pub fn extract_depth_mask(img: &RgbF32Buf, near: f32, far: f32) -> Mask01 {
    let (w, h) = (img.width, img.height);
    let lo = far.min(near);
    let hi = far.max(near);
    let soft = 0.08f32;
    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        // depth 0 at top (far), 1 at bottom (near)
        let d = y as f32 / h.max(1) as f32;
        let m = ((d - (lo - soft)) / (soft * 2.0 + 1e-5)).clamp(0.0, 1.0)
            * (1.0 - ((d - (hi - soft)) / (soft * 2.0 + 1e-5)).clamp(0.0, 1.0));
        for x in 0..w {
            let i = (y * w + x) * 3;
            let luma =
                0.299 * enc(img.data[i]) + 0.587 * enc(img.data[i + 1]) + 0.114 * enc(img.data[i + 2]);
            // Bright upper sky shouldn't dominate "far" when user wants mid-ground.
            let sky_pen = if d < 0.32 && luma > 0.68 { 0.45 } else { 1.0 };
            data[y * w + x] = (m * sky_pen).clamp(0.0, 1.0);
        }
    }
    Mask01 { width: w, height: h, data }
}

/// Shared AI mask runner — keep live preview and export in lockstep.
pub fn run_ai_mask(
    kind: &str,
    img: &RgbF32Buf,
    hint_point: Option<(f32, f32)>,
    depth_near: Option<f32>,
    depth_far: Option<f32>,
) -> Result<Mask01, CoreError> {
    let seg = TractSegmenter;
    match kind {
        "subject" | "background" | "people" => {
            if let Some(p) = hint_point {
                instance_mask_at_point(img, p)
            } else {
                seg.subject(img)
            }
        }
        "face" => seg.subject(img).map(|m| extract_face_mask(&m, img)),
        "skin" => seg.subject(img).map(|m| extract_skin_mask(&m, img)),
        "hair" => seg.subject(img).map(|m| extract_hair_mask(&m, img)),
        "lips" => seg.subject(img).map(|m| extract_lips_mask(&m, img)),
        "eyes" => seg.subject(img).map(|m| extract_eyes_mask(&m, img)),
        "sky" => seg.sky(img),
        "water" => Ok(extract_water_mask(img)),
        "vegetation" => Ok(extract_vegetation_mask(img)),
        "mountains" => Ok(extract_mountains_mask(img)),
        "architecture" => Ok(extract_architecture_mask(img)),
        "ground" => Ok(extract_ground_mask(img)),
        "depth" => Ok(extract_depth_mask(
            img,
            depth_near.unwrap_or(0.35),
            depth_far.unwrap_or(1.0),
        )),
        "object" => {
            let p = hint_point.unwrap_or((0.5, 0.5));
            instance_mask_at_point(img, p).or_else(|_| seg.object(img, p))
        }
        other => Err(CoreError::InvalidOp(format!("kind {other} is not segmented"))),
    }
}

/// Cache / lookup key for a segmented mask texture.
/// Top-level segmented masks use `mask_id`; composite children use `mask_id#c{i}`.
pub fn segment_cache_key(mask_id: &str, component_index: Option<usize>) -> String {
    match component_index {
        Some(i) => format!("{mask_id}#c{i}"),
        None => mask_id.to_string(),
    }
}

/// Parent mask id from a possibly composite cache key (`id#c0` → `id`).
pub fn segment_parent_mask_id(cache_key: &str) -> &str {
    cache_key.split("#c").next().unwrap_or(cache_key)
}

/// Infer AI kind from a segmented source's `model` field (e.g. `skin_v1` → `skin`).
pub fn kind_from_segmented_source(parent_kind: &str, source: &serde_json::Value) -> String {
    if let Some(model) = source.get("model").and_then(|m| m.as_str()) {
        if let Some(base) = model.split('_').next() {
            if !base.is_empty() {
                return base.to_string();
            }
        }
    }
    parent_kind.to_string()
}

/// Propose clickable object/subject outlines from saliency (keeps multiple blobs).
pub fn propose_objects(img: &RgbF32Buf) -> Result<Vec<ObjectProposal>, CoreError> {
    let mut mask = run_u2net(subject_model()?, img)?;
    // Soft refine — keep secondary subjects; avoid the full extremity grow that
    // can box-fill dark water (that path belongs to instance masks only).
    refine_saliency_mask(&mut mask.data, mask.width, mask.height);
    Ok(proposals_from_saliency(&mask.data, mask.width, mask.height))
}

fn adaptive_saliency_thresh(data: &[f32]) -> f32 {
    if data.is_empty() {
        return 0.28;
    }
    let mut vals: Vec<f32> = data.iter().copied().filter(|v| *v > 0.05).collect();
    if vals.len() < 32 {
        return 0.22;
    }
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    // ~78th percentile of non-trivial scores — catches small wildlife saliency.
    let idx = ((vals.len() as f32) * 0.78) as usize;
    vals[idx.min(vals.len() - 1)].clamp(0.16, 0.42)
}

fn proposals_from_saliency(data: &[f32], w: usize, h: usize) -> Vec<ObjectProposal> {
    let t = adaptive_saliency_thresh(data);
    let mut labels = vec![0u32; w * h];
    let mut stats: Vec<(u32, f32, f32, f32)> = Vec::new(); // area, sum, sum_x, sum_y
    let mut stack = Vec::new();

    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if data[i] < t || labels[i] != 0 {
                continue;
            }
            let id = (stats.len() + 1) as u32;
            let mut area = 0u32;
            let mut sum = 0.0f32;
            let mut sx = 0.0f32;
            let mut sy = 0.0f32;
            stack.clear();
            stack.push(i);
            labels[i] = id;
            while let Some(j) = stack.pop() {
                let jx = j % w;
                let jy = j / w;
                area += 1;
                sum += data[j];
                sx += jx as f32;
                sy += jy as f32;
                for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = jx as i32 + dx;
                    let ny = jy as i32 + dy;
                    if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                        continue;
                    }
                    let k = ny as usize * w + nx as usize;
                    if labels[k] == 0 && data[k] >= t {
                        labels[k] = id;
                        stack.push(k);
                    }
                }
            }
            stats.push((area, sum, sx, sy));
        }
    }

    let img_area = (w * h) as f32;
    // Wildlife / distant subjects can be << 1% of frame — old 1.2% floor hid them.
    let min_area = (img_area * 0.0018).max(16.0) as u32;
    let mut ranked: Vec<(usize, f32)> = stats
        .iter()
        .enumerate()
        .filter(|(_, (a, ..))| *a >= min_area)
        .map(|(i, (a, s, ..))| (i, *s * (*a as f32).sqrt()))
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(8);

    let mut out = Vec::new();
    for (rank, (bi, _)) in ranked.into_iter().enumerate() {
        let lab = (bi + 1) as u32;
        let (area, _sum, sx, sy) = stats[bi];
        let mut path = trace_contour(&labels, w, h, lab);
        if path.len() < 4 {
            continue;
        }
        path = simplify_path(&path, 1.35);
        if path.len() < 3 {
            continue;
        }
        // Close the path for SVG.
        if let Some(first) = path.first().copied() {
            if path.last() != Some(&first) {
                path.push(first);
            }
        }
        let path_n: Vec<[f32; 2]> = path
            .iter()
            .map(|&(x, y)| {
                [
                    (x as f32 + 0.5) / w as f32,
                    (y as f32 + 0.5) / h as f32,
                ]
            })
            .collect();
        out.push(ObjectProposal {
            id: rank as u32,
            path: path_n,
            centroid: [sx / area as f32 / w as f32, sy / area as f32 / h as f32],
            area: area as f32 / img_area,
        });
    }
    out
}

/// Moore neighborhood contour trace → pixel centers of the outer boundary.
fn trace_contour(labels: &[u32], w: usize, h: usize, lab: u32) -> Vec<(i32, i32)> {
    // Find leftmost topmost pixel of the component.
    let mut start = None;
    'outer: for y in 0..h {
        for x in 0..w {
            if labels[y * w + x] == lab {
                start = Some((x as i32, y as i32));
                break 'outer;
            }
        }
    }
    let Some((sx, sy)) = start else {
        return Vec::new();
    };

    // Directions: E, SE, S, SW, W, NW, N, NE
    const DIRS: [(i32, i32); 8] = [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ];
    let inside = |x: i32, y: i32| -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < w
            && (y as usize) < h
            && labels[y as usize * w + x as usize] == lab
    };

    let mut path = Vec::new();
    let mut x = sx;
    let mut y = sy;
    let mut dir = 4usize; // come from west so first search starts north-ish
    let max_steps = (w * h).saturating_mul(2).max(64);
    for _ in 0..max_steps {
        path.push((x, y));
        let mut found = false;
        // Start searching from dir-2 (right-hand rule).
        for k in 0..8 {
            let nd = (dir + 6 + k) % 8;
            let (dx, dy) = DIRS[nd];
            let nx = x + dx;
            let ny = y + dy;
            if inside(nx, ny) {
                x = nx;
                y = ny;
                dir = nd;
                found = true;
                break;
            }
        }
        if !found {
            break;
        }
        if x == sx && y == sy && path.len() > 2 {
            break;
        }
    }
    path
}

fn simplify_path(pts: &[(i32, i32)], eps: f32) -> Vec<(i32, i32)> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    rdp(pts, 0, pts.len() - 1, eps, &mut keep);
    pts.iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, p)| *p)
        .collect()
}

fn rdp(pts: &[(i32, i32)], a: usize, b: usize, eps: f32, keep: &mut [bool]) {
    if b <= a + 1 {
        return;
    }
    let (ax, ay) = (pts[a].0 as f32, pts[a].1 as f32);
    let (bx, by) = (pts[b].0 as f32, pts[b].1 as f32);
    let mut max_d = 0.0f32;
    let mut max_i = a;
    for i in a + 1..b {
        let (px, py) = (pts[i].0 as f32, pts[i].1 as f32);
        let d = point_line_dist(px, py, ax, ay, bx, by);
        if d > max_d {
            max_d = d;
            max_i = i;
        }
    }
    if max_d > eps {
        keep[max_i] = true;
        rdp(pts, a, max_i, eps, keep);
        rdp(pts, max_i, b, eps, keep);
    }
}

fn point_line_dist(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-6 {
        return (px - ax).hypot(py - ay);
    }
    let t = ((px - ax) * dx + (py - ay) * dy) / len2;
    let qx = ax + t * dx;
    let qy = ay + t * dy;
    (px - qx).hypot(py - qy)
}

/// Keep only the saliency connected component under `point` (normalized 0..1).
/// Used for multi-subject / object outline picks so each click becomes one instance.
pub fn instance_mask_at_point(img: &RgbF32Buf, point: (f32, f32)) -> Result<Mask01, CoreError> {
    let mut mask = run_u2net(subject_model()?, img)?;
    refine_saliency_with_image(&mut mask.data, mask.width, mask.height, img);
    if !keep_blob_at_point(&mut mask.data, mask.width, mask.height, point, 0.22) {
        // Click missed a saliency blob — fall back to color region-grow.
        return TractSegmenter.object(img, point);
    }
    // Soft fringe after instance isolation.
    let (w, h) = (mask.width, mask.height);
    let blurred = box_blur_3x3(&mask.data, w, h);
    for i in 0..mask.data.len() {
        if mask.data[i] > 0.03 {
            mask.data[i] = (mask.data[i] * 0.68 + blurred[i] * 0.32).clamp(0.0, 1.0);
        }
    }
    // Grow slightly into similar colors but stay near the instance (no full-image flood).
    // `object` returns full image res — resample into NET_SIZE mask space.
    if let Ok(grown) = TractSegmenter.object(img, point) {
        let (mw, mh) = (mask.width, mask.height);
        let (gw, gh) = (grown.width, grown.height);
        if gw > 0 && gh > 0 {
            for y in 0..mh {
                for x in 0..mw {
                    let gx = ((x as f32 + 0.5) * gw as f32 / mw as f32) as usize;
                    let gy = ((y as f32 + 0.5) * gh as f32 / mh as f32) as usize;
                    let gi = gy.min(gh - 1) * gw + gx.min(gw - 1);
                    let gv = grown.data[gi];
                    let i = y * mw + x;
                    if mask.data[i] > 0.1 && gv > 0.18 {
                        mask.data[i] = mask.data[i].max(gv * 0.72);
                    } else if mask.data[i] > 0.3 && gv > 0.4 {
                        mask.data[i] = mask.data[i].max(gv * 0.45);
                    }
                }
            }
        }
    }
    // Second extremity pass after isolation — recovers the clicked bird's wing.
    grow_attached_extremities(&mut mask.data, mask.width, mask.height, img);
    suppress_reflections_and_islands(&mut mask.data, mask.width, mask.height);
    suppress_underside_water_bleed(&mut mask.data, mask.width, mask.height, img);
    Ok(mask)
}

/// Zero every pixel not belonging to the connected component containing `point`.
/// Returns false if the seed itself is below threshold (no blob).
fn keep_blob_at_point(
    data: &mut [f32],
    w: usize,
    h: usize,
    point: (f32, f32),
    thresh: f32,
) -> bool {
    if w * h == 0 {
        return false;
    }
    let cx = ((point.0.clamp(0.0, 1.0) * w as f32) as usize).min(w.saturating_sub(1));
    let cy = ((point.1.clamp(0.0, 1.0) * h as f32) as usize).min(h.saturating_sub(1));
    let seed = cy * w + cx;
    if data[seed] < thresh {
        // Search a neighborhood for a strong seed (outline click may land on soft edge).
        let mut best = None;
        let mut best_v = thresh;
        let mut best_d2 = i32::MAX;
        for dy in -6i32..=6 {
            for dx in -6i32..=6 {
                let nx = cx as i32 + dx;
                let ny = cy as i32 + dy;
                if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                    continue;
                }
                let i = ny as usize * w + nx as usize;
                let d2 = dx * dx + dy * dy;
                if data[i] > best_v || (data[i] >= best_v && d2 < best_d2) {
                    if data[i] >= thresh {
                        best_v = data[i];
                        best_d2 = d2;
                        best = Some(i);
                    }
                }
            }
        }
        let Some(s) = best else {
            return false;
        };
        return flood_keep(data, w, h, s, thresh);
    }
    flood_keep(data, w, h, seed, thresh)
}

fn flood_keep(data: &mut [f32], w: usize, h: usize, seed: usize, thresh: f32) -> bool {
    let mut keep = vec![false; w * h];
    let mut stack = vec![seed];
    keep[seed] = true;
    while let Some(i) = stack.pop() {
        let x = i % w;
        let y = i / w;
        for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                continue;
            }
            let j = ny as usize * w + nx as usize;
            if !keep[j] && data[j] >= thresh {
                keep[j] = true;
                stack.push(j);
            }
        }
    }
    for (i, v) in data.iter_mut().enumerate() {
        if !keep[i] {
            *v = 0.0;
        }
    }
    true
}

/// Drop ocean / water false-positives from a sky segmentation.
/// Prefers components that live in the upper frame and attenuates
/// darker blue-cyan pixels in the lower half.
pub fn refine_sky_mask(data: &mut [f32], w: usize, h: usize, img: &RgbF32Buf) {
    if w * h == 0 {
        return;
    }
    // Mild contrast stretch so soft skyseg output separates better.
    for v in data.iter_mut() {
        *v = ((*v - 0.35) * 1.55 + 0.35).clamp(0.0, 1.0);
    }

    let sx = img.width as f32 / w as f32;
    let sy = img.height as f32 / h as f32;
    let sample = |mx: usize, my: usize| -> [f32; 3] {
        let ix = ((mx as f32 + 0.5) * sx) as usize;
        let iy = ((my as f32 + 0.5) * sy) as usize;
        let i = (iy.min(img.height.saturating_sub(1)) * img.width
            + ix.min(img.width.saturating_sub(1)))
            * 3;
        [
            enc(img.data[i]),
            enc(img.data[i + 1]),
            enc(img.data[i + 2]),
        ]
    };

    // Per-pixel: in the lower half, crush water-looking skyseg hits.
    for y in 0..h {
        let y_norm = y as f32 / h.max(1) as f32;
        for x in 0..w {
            let i = y * w + x;
            if data[i] < 0.08 {
                data[i] = 0.0;
                continue;
            }
            let [r, g, b] = sample(x, y);
            let luma = 0.299 * r + 0.587 * g + 0.114 * b;
            let waterish = b > r + 0.04
                && b > g * 0.92
                && luma < 0.55
                && (b - r.max(g)) > 0.02;
            if y_norm > 0.48 && waterish {
                data[i] *= 0.05;
            } else if y_norm > 0.55 && luma < 0.48 && b > r + 0.02 {
                data[i] *= 0.08;
            } else if y_norm > 0.62 && luma < 0.42 && b > r {
                data[i] *= 0.06;
            } else if y_norm < 0.35 && luma > 0.45 && b >= g * 0.9 {
                // Boost pale upper sky a touch.
                data[i] = (data[i] * 1.1).clamp(0.0, 1.0);
            }
        }
    }

    // Connected components: drop blobs that live mostly in the lower frame.
    const T: f32 = 0.22;
    let mut labels = vec![0u32; w * h];
    let mut stats: Vec<(u32, f32, f32, u32)> = Vec::new(); // area, sum_y, sum, upper_count
    let mut stack = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if data[i] < T || labels[i] != 0 {
                continue;
            }
            let id = (stats.len() + 1) as u32;
            let mut area = 0u32;
            let mut sum_y = 0.0f32;
            let mut sum = 0.0f32;
            let mut upper = 0u32;
            stack.clear();
            stack.push(i);
            labels[i] = id;
            while let Some(j) = stack.pop() {
                let jx = j % w;
                let jy = j / w;
                area += 1;
                sum_y += jy as f32;
                sum += data[j];
                if jy < h / 2 {
                    upper += 1;
                }
                for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = jx as i32 + dx;
                    let ny = jy as i32 + dy;
                    if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                        continue;
                    }
                    let k = ny as usize * w + nx as usize;
                    if labels[k] == 0 && data[k] >= T {
                        labels[k] = id;
                        stack.push(k);
                    }
                }
            }
            stats.push((area, sum_y, sum, upper));
        }
    }

    let img_area = (w * h) as f32;
    let mut keep = vec![false; stats.len()];
    for (i, (area, sum_y, _sum, upper)) in stats.iter().enumerate() {
        if *area < (img_area * 0.004).max(8.0) as u32 {
            continue;
        }
        let cy = sum_y / (*area).max(1) as f32;
        let upper_frac = *upper as f32 / (*area).max(1) as f32;
        // Ocean / lake blobs: centroid in lower half and little upper presence.
        if cy > h as f32 * 0.48 && upper_frac < 0.32 {
            continue;
        }
        // Prefer anything with a real foothold in the top half.
        if upper_frac >= 0.15 || cy < h as f32 * 0.42 {
            keep[i] = true;
        }
    }
    // Always keep the strongest upper-anchored blob if nothing passed.
    if !keep.iter().any(|k| *k) {
        if let Some((bi, _)) = stats
            .iter()
            .enumerate()
            .filter(|(_, (_, _, _, u))| *u > 0)
            .max_by(|(_, a), (_, b)| {
                let sa = a.2 * (a.3 as f32 + 1.0);
                let sb = b.2 * (b.3 as f32 + 1.0);
                sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            keep[bi] = true;
        }
    }

    for (i, v) in data.iter_mut().enumerate() {
        let lab = labels[i];
        if lab == 0 {
            if *v < T {
                *v = 0.0;
            }
            continue;
        }
        if !keep[(lab - 1) as usize] {
            *v = 0.0;
        }
    }

    // Soft fringe so sky edges (branches / hair) don't look crunchy.
    let blurred = box_blur_3x3(data, w, h);
    for i in 0..data.len() {
        if data[i] > 0.04 {
            data[i] = (data[i] * 0.65 + blurred[i] * 0.35).clamp(0.0, 1.0);
        }
    }
}

fn water_score(r: f32, g: f32, b: f32, y_norm: f32) -> f32 {
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let chroma = mx - mn;
    // Trees / grass are not water even when the frame is low.
    if g > r + 0.05 && g > b + 0.03 && chroma > 0.06 && luma > 0.12 {
        return 0.0;
    }
    let lower = ((y_norm - 0.18) / 0.65).clamp(0.0, 1.0);
    let blue_lead = b > r + 0.012 && b >= g * 0.82;
    let teal = g > r + 0.015 && b > r + 0.015 && (b - r) > 0.015;
    let blue = ((b - r.max(g * 0.9)) / 0.28).clamp(0.0, 1.0);
    let cyan = ((b.min(g) - r) / 0.22).clamp(0.0, 1.0);
    let mut score = 0.0f32;
    if (blue_lead || teal) && (0.03..=0.88).contains(&luma) {
        let depth = (1.0 - (luma / 0.80).clamp(0.0, 1.0)).clamp(0.22, 1.0);
        score = score.max(blue.max(cyan * 0.9) * depth * (0.30 + 0.70 * lower));
    }
    // Glacial / overcast lakes: grey-green, low chroma, mid luma, mid-lower frame.
    if chroma < 0.12 && (0.06..=0.64).contains(&luma) && y_norm > 0.20 {
        let grey = (1.0 - chroma / 0.12).clamp(0.0, 1.0);
        let cool = ((b + g) * 0.5 - r).clamp(0.0, 0.14) / 0.14;
        score = score.max(grey * (0.40 + 0.60 * cool) * (0.35 + 0.65 * lower));
    }
    // Dark water under ice / dusk.
    if luma < 0.20 && chroma < 0.14 && y_norm > 0.26 {
        score = score.max((1.0 - luma / 0.20) * (1.0 - chroma / 0.14) * lower * 0.85);
    }
    // White foam / falls — scored weakly; extract_water_mask grows these
    // only when they touch other water so clouds stay out.
    if luma > 0.70 && chroma < 0.10 && y_norm > 0.14 {
        score = score.max(((luma - 0.70) / 0.30).clamp(0.0, 1.0) * (1.0 - chroma / 0.10) * 0.40);
    }
    score
}

fn vegetation_score(r: f32, g: f32, b: f32) -> f32 {
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    // Green + yellow-green foliage (sunlit leaves).
    let green_lead = g > r * 0.98 && g > b * 0.98;
    let yellow_green = g > b + 0.04 && r > b && g >= r * 0.9;
    if !green_lead && !yellow_green {
        return 0.0;
    }
    if !(0.04..=0.82).contains(&luma) {
        return 0.0;
    }
    let green = ((g - r.max(b)) / 0.3).clamp(0.0, 1.0);
    let sunlit = ((g - b) / 0.25).clamp(0.0, 1.0) * ((r - b) / 0.2).clamp(0.0, 1.0);
    let mid = (1.0 - ((luma - 0.38).abs() / 0.45)).clamp(0.15, 1.0);
    green.max(sunlit * 0.7) * mid
}

/// Landscape water (ocean / lake / river / falls) from color + vertical priors.
pub fn extract_water_mask(img: &RgbF32Buf) -> Mask01 {
    let (w, h) = (img.width, img.height);
    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        let y_norm = y as f32 / h.max(1) as f32;
        for x in 0..w {
            let i = (y * w + x) * 3;
            let (r, g, b) = (enc(img.data[i]), enc(img.data[i + 1]), enc(img.data[i + 2]));
            data[y * w + x] = water_score(r, g, b, y_norm);
        }
    }
    grow_foam_on_water(&mut data, img, w, h);
    suppress_compact_bright_objects(&mut data, img, w, h);
    let blurred = box_blur_3x3(&data, w, h);
    for i in 0..data.len() {
        data[i] = (data[i] * 0.7 + blurred[i] * 0.3).clamp(0.0, 1.0);
        if data[i] < 0.06 {
            data[i] = 0.0;
        }
    }
    suppress_upper_false_water(&mut data, w, h);
    Mask01 {
        width: w,
        height: h,
        data,
    }
}

fn grow_foam_on_water(data: &mut [f32], img: &RgbF32Buf, w: usize, h: usize) {
    if w < 3 || h < 3 {
        return;
    }
    for _ in 0..10 {
        let prior = data.to_vec();
        let mut grew = false;
        for y in 1..h - 1 {
            let y_norm = y as f32 / h.max(1) as f32;
            if y_norm < 0.12 {
                continue;
            }
            for x in 1..w - 1 {
                let i = y * w + x;
                if prior[i] > 0.2 {
                    continue;
                }
                let pi = (y * w + x) * 3;
                let (r, g, b) = (enc(img.data[pi]), enc(img.data[pi + 1]), enc(img.data[pi + 2]));
                let luma = 0.299 * r + 0.587 * g + 0.114 * b;
                let chroma = r.max(g).max(b) - r.min(g).min(b);
                if luma < 0.68 || chroma > 0.14 {
                    continue;
                }
                let mut n = 0u32;
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    if prior[(y as i32 + dy) as usize * w + (x as i32 + dx) as usize] > 0.14 {
                        n += 1;
                    }
                }
                if n >= 1 {
                    data[i] = data[i].max(0.62);
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }
}

fn suppress_compact_bright_objects(data: &mut [f32], img: &RgbF32Buf, w: usize, h: usize) {
    if w * h == 0 {
        return;
    }
    let mut labels = vec![0u32; w * h];
    let mut stats: Vec<(u32, f32, f32, usize, usize, usize, usize)> = Vec::new();
    let mut stack = Vec::new();
    const T: f32 = 0.22;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if data[i] < T || labels[i] != 0 {
                continue;
            }
            let id = (stats.len() + 1) as u32;
            let mut area = 0.0f32;
            let mut luma_sum = 0.0f32;
            let mut x0 = x;
            let mut x1 = x;
            let mut y0 = y;
            let mut y1 = y;
            stack.clear();
            stack.push(i);
            labels[i] = id;
            while let Some(j) = stack.pop() {
                area += 1.0;
                let jx = j % w;
                let jy = j / w;
                x0 = x0.min(jx);
                x1 = x1.max(jx);
                y0 = y0.min(jy);
                y1 = y1.max(jy);
                let pi = (jy.min(img.height.saturating_sub(1)) * img.width
                    + jx.min(img.width.saturating_sub(1)))
                    * 3;
                // data and img may share resolution (water path).
                let (r, g, b) = if img.width == w && img.height == h {
                    (enc(img.data[pi]), enc(img.data[pi + 1]), enc(img.data[pi + 2]))
                } else {
                    let s = sample_enc_rgb(img, jx, jy, w, h);
                    (s[0], s[1], s[2])
                };
                luma_sum += 0.299 * r + 0.587 * g + 0.114 * b;
                for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = jx as i32 + dx;
                    let ny = jy as i32 + dy;
                    if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                        continue;
                    }
                    let k = ny as usize * w + nx as usize;
                    if labels[k] == 0 && data[k] >= T {
                        labels[k] = id;
                        stack.push(k);
                    }
                }
            }
            stats.push((id, area, luma_sum, x0, x1, y0, y1));
        }
    }
    let frame = (w * h) as f32;
    let mut drop: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for (id, area, luma_sum, x0, x1, y0, y1) in stats {
        let bw = (x1 + 1).saturating_sub(x0).max(1) as f32;
        let bh = (y1 + 1).saturating_sub(y0).max(1) as f32;
        let fill = area / (bw * bh);
        let mean_luma = luma_sum / area.max(1.0);
        let frac = area / frame;
        if mean_luma > 0.68 && fill > 0.42 && (0.008..=0.18).contains(&frac) && bh <= bw * 1.2 {
            drop.insert(id);
        }
    }
    if drop.is_empty() {
        return;
    }
    for i in 0..data.len() {
        if drop.contains(&labels[i]) {
            data[i] = 0.0;
        }
    }
}

fn suppress_upper_false_water(data: &mut [f32], w: usize, h: usize) {
    for y in 0..h {
        let y_norm = y as f32 / h.max(1) as f32;
        let atten = if y_norm < 0.22 {
            0.05
        } else if y_norm < 0.38 {
            0.35
        } else {
            1.0
        };
        for x in 0..w {
            data[y * w + x] *= atten;
        }
    }
}

/// Landscape vegetation (grass / trees / foliage) from green dominance.
pub fn extract_vegetation_mask(img: &RgbF32Buf) -> Mask01 {
    let (w, h) = (img.width, img.height);
    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 3;
            let (r, g, b) = (enc(img.data[i]), enc(img.data[i + 1]), enc(img.data[i + 2]));
            data[y * w + x] = vegetation_score(r, g, b);
        }
    }
    refine_saliency_mask(&mut data, w, h);
    Mask01 {
        width: w,
        height: h,
        data,
    }
}

impl Segmenter for TractSegmenter {
    fn subject(&self, img: &RgbF32Buf) -> Result<Mask01, CoreError> {
        let mut mask = run_u2net(subject_model()?, img)?;
        refine_saliency_with_image(&mut mask.data, mask.width, mask.height, img);
        Ok(mask)
    }

    /// Sky via dedicated U²-Net weights (MIT — xiongzhu666 Sky-Segmentation).
    fn sky(&self, img: &RgbF32Buf) -> Result<Mask01, CoreError> {
        let mut mask = run_u2net(sky_model()?, img)?;
        refine_sky_mask(&mut mask.data, mask.width, mask.height, img);
        Ok(mask)
    }

    /// Object-by-point: color-similarity region grow seeded at the point.
    /// Stops at water / large chroma jumps so wildlife on dark water doesn't
    /// flood into a rectangular blob of background.
    fn object(&self, img: &RgbF32Buf, point: (f32, f32)) -> Result<Mask01, CoreError> {
        let (w, h) = (img.width, img.height);
        let cx = ((point.0.clamp(0.0, 1.0) * w as f32) as usize).min(w.saturating_sub(1));
        let cy = ((point.1.clamp(0.0, 1.0) * h as f32) as usize).min(h.saturating_sub(1));
        let seed_i = cy * w + cx;
        let seed = [
            img.data[seed_i * 3],
            img.data[seed_i * 3 + 1],
            img.data[seed_i * 3 + 2],
        ];
        let dist = |i: usize| -> f32 {
            let dr = img.data[i * 3] - seed[0];
            let dg = img.data[i * 3 + 1] - seed[1];
            let db = img.data[i * 3 + 2] - seed[2];
            (dr * dr + dg * dg + db * db).sqrt()
        };
        let lum = 0.2627 * seed[0] + 0.678 * seed[1] + 0.0593 * seed[2];
        let thresh = (0.16 * (lum + 0.25)).max(0.045);
        // Local search radius — full-image flood on water creates giant masks.
        let max_r = ((w.min(h) as f32) * 0.42).max(32.0) as i32;
        let mut mask = vec![0.0f32; w * h];
        let mut stack = vec![seed_i];
        let mut visited = 0usize;
        let max_visit = ((w * h) as f32 * 0.28) as usize;
        while let Some(i) = stack.pop() {
            if mask[i] > 0.0 {
                continue;
            }
            let (x, y) = (i % w, i / w);
            let dx = x as i32 - cx as i32;
            let dy = y as i32 - cy as i32;
            if dx * dx + dy * dy > max_r * max_r {
                continue;
            }
            let rgb = [img.data[i * 3], img.data[i * 3 + 1], img.data[i * 3 + 2]];
            if is_waterish_rgb(rgb[0], rgb[1], rgb[2]) && dist(i) > thresh * 0.35 {
                continue;
            }
            let d = dist(i);
            if d > thresh {
                continue;
            }
            mask[i] = (1.0 - d / thresh).clamp(0.3, 1.0);
            visited += 1;
            if visited > max_visit {
                break;
            }
            for (nx, ny) in [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ] {
                if nx < w && ny < h {
                    let j = ny * w + nx;
                    if mask[j] == 0.0 {
                        // Edge stop: large neighbor jump = silhouette boundary.
                        let nd = dist(j);
                        let edge = (nd - d).abs();
                        if edge < thresh * 0.85 || nd <= thresh {
                            stack.push(j);
                        }
                    }
                }
            }
        }
        Ok(Mask01 {
            width: w,
            height: h,
            data: mask,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_scene() -> RgbF32Buf {
        let (w, h) = (64usize, 64usize);
        let mut data = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                if y < h / 2 {
                    data[i] = 0.3;
                    data[i + 1] = 0.45;
                    data[i + 2] = 0.8;
                } else {
                    data[i] = 0.05;
                    data[i + 1] = 0.05;
                    data[i + 2] = 0.05;
                }
                let (dx, dy) = (x as i32 - 32, y as i32 - 48);
                if dx * dx + dy * dy < 80 {
                    data[i] = 0.5;
                    data[i + 1] = 0.08;
                    data[i + 2] = 0.08;
                }
            }
        }
        RgbF32Buf {
            width: w,
            height: h,
            data,
        }
    }

    /// Blue top band (sky) + blue bottom band (ocean) — heuristic confuses these.
    fn sky_ocean_scene() -> RgbF32Buf {
        let (w, h) = (128usize, 128usize);
        let mut data = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                if y < h / 2 {
                    // pale sky
                    data[i] = 0.55;
                    data[i + 1] = 0.72;
                    data[i + 2] = 0.95;
                } else {
                    // deep ocean — similar hue, lower in frame
                    data[i] = 0.02;
                    data[i + 1] = 0.18;
                    data[i + 2] = 0.42;
                }
            }
        }
        RgbF32Buf {
            width: w,
            height: h,
            data,
        }
    }

    #[test]
    #[ignore = "loads the ~84MB sky model; run with --ignored"]
    fn sky_model_prefers_top_over_ocean() {
        let img = sky_ocean_scene();
        let m = TractSegmenter.sky(&img).unwrap();
        let at = |x: usize, y: usize| m.data[y * m.width + x];
        let sky = at(m.width / 2, m.height / 8);
        let ocean = at(m.width / 2, m.height * 7 / 8);
        assert!(sky > 0.45, "sky band: {sky}");
        assert!(ocean < sky * 0.55, "ocean {ocean} should be below sky {sky}");
    }

    #[test]
    fn sky_refine_drops_lower_ocean_blob() {
        let img = sky_ocean_scene();
        let (w, h) = (img.width, img.height);
        // Fake skyseg that lights up both sky and ocean equally.
        let mut data = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                data[y * w + x] = 0.85;
            }
        }
        refine_sky_mask(&mut data, w, h, &img);
        let mut sky = 0.0f32;
        let mut ocean = 0.0f32;
        for y in 0..h / 4 {
            for x in 0..w {
                sky += data[y * w + x];
            }
        }
        for y in (3 * h / 4)..h {
            for x in 0..w {
                ocean += data[y * w + x];
            }
        }
        assert!(sky > ocean * 2.0, "sky {sky} should dominate ocean {ocean}");
        assert!(ocean < (w * h / 4) as f32 * 0.25, "ocean residual {ocean}");
    }

    #[test]
    fn keep_blob_isolates_one_instance() {
        let (w, h) = (64usize, 64usize);
        let mut data = vec![0.0f32; w * h];
        for y in 5..20 {
            for x in 5..20 {
                data[y * w + x] = 0.9;
            }
        }
        for y in 40..55 {
            for x in 40..55 {
                data[y * w + x] = 0.9;
            }
        }
        assert!(keep_blob_at_point(
            &mut data,
            w,
            h,
            (12.0 / w as f32, 12.0 / h as f32),
            0.28
        ));
        assert!(data[12 * w + 12] > 0.5);
        assert!(data[48 * w + 48] < 0.05);
    }

    #[test]
    fn reflection_blob_below_primary_is_cleared() {
        let (w, h) = (64usize, 64usize);
        let mut data = vec![0.0f32; w * h];
        // Strong primary subject in upper half
        for y in 8..28 {
            for x in 20..44 {
                data[y * w + x] = 0.9;
            }
        }
        // Weaker reflection in lower half
        for y in 40..56 {
            for x in 22..42 {
                data[y * w + x] = 0.55;
            }
        }
        suppress_reflections_and_islands(&mut data, w, h);
        let mut upper = 0.0f32;
        for y in 8..28 {
            for x in 20..44 {
                upper += data[y * w + x];
            }
        }
        let mut lower = 0.0f32;
        for y in 40..56 {
            for x in 22..42 {
                lower += data[y * w + x];
            }
        }
        assert!(upper > 100.0, "primary kept: {upper}");
        assert!(lower < 5.0, "reflection cleared: {lower}");
    }

    #[test]
    fn underside_water_bleed_cleared_but_body_kept() {
        // Otter-style: solid body with soft water-colored bleed fused underneath.
        let (w, h) = (64usize, 64usize);
        let mut rgb = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                rgb[i] = 0.08;
                rgb[i + 1] = 0.22;
                rgb[i + 2] = 0.28; // blue-green water
            }
        }
        for y in 18..36 {
            for x in 18..46 {
                let i = (y * w + x) * 3;
                rgb[i] = 0.35;
                rgb[i + 1] = 0.22;
                rgb[i + 2] = 0.12; // brown fur
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data: rgb,
        };
        let mut mask = vec![0.0f32; w * h];
        for y in 18..36 {
            for x in 18..46 {
                mask[y * w + x] = 0.92;
            }
        }
        // Soft reflection fused below belly (same blob)
        for y in 36..50 {
            for x in 22..42 {
                mask[y * w + x] = 0.48;
            }
        }
        suppress_underside_water_bleed(&mut mask, w, h, &img);
        let mut body = 0.0f32;
        for y in 18..36 {
            for x in 18..46 {
                body += mask[y * w + x];
            }
        }
        let mut bleed = 0.0f32;
        for y in 38..50 {
            for x in 22..42 {
                bleed += mask[y * w + x];
            }
        }
        assert!(body > 200.0, "body kept: {body}");
        assert!(bleed < 30.0, "underside water bleed cleared: {bleed}");
    }

    #[test]
    fn grow_extremities_recovers_dark_wing() {
        // Dark body + dark wing protrusion on dark blue water — classic bird fail.
        let (w, h) = (96usize, 96usize);
        let mut data = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                // dark blue water
                data[i] = 0.02;
                data[i + 1] = 0.06;
                data[i + 2] = 0.14;
            }
        }
        // Body (black feathers)
        for y in 30..62 {
            for x in 38..62 {
                let i = (y * w + x) * 3;
                data[i] = 0.04;
                data[i + 1] = 0.04;
                data[i + 2] = 0.045;
            }
        }
        // Wing sticking down-left (also black, often missed by saliency)
        for y in 55..78 {
            for x in 18..40 {
                let i = (y * w + x) * 3;
                data[i] = 0.035;
                data[i + 1] = 0.035;
                data[i + 2] = 0.038;
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data,
        };
        // Fake saliency: body only (no wing).
        let mut mask = vec![0.0f32; w * h];
        for y in 30..62 {
            for x in 38..62 {
                mask[y * w + x] = 0.9;
            }
        }
        grow_attached_extremities(&mut mask, w, h, &img);
        let mut wing = 0.0f32;
        for y in 58..76 {
            for x in 20..36 {
                wing += mask[y * w + x];
            }
        }
        let mut water = 0.0f32;
        for y in 8..20 {
            for x in 8..20 {
                water += mask[y * w + x];
            }
        }
        assert!(wing > 40.0, "wing should be grown into mask: {wing}");
        assert!(water < 5.0, "water should stay clear: {water}");
    }

    #[test]
    fn grow_does_not_box_fill_dark_gray_water() {
        // Regression: dark gray water (not blue-led) used to fill the padded
        // subject bbox → solid red square overlay instead of a silhouette.
        let (w, h) = (96usize, 96usize);
        let mut data = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                data[i] = 0.03;
                data[i + 1] = 0.035;
                data[i + 2] = 0.04; // nearly neutral dark water
            }
        }
        // Compact body with slightly different black + a warm beak cue
        for y in 36..58 {
            for x in 40..58 {
                let i = (y * w + x) * 3;
                data[i] = 0.055;
                data[i + 1] = 0.05;
                data[i + 2] = 0.048;
            }
        }
        for y in 40..48 {
            for x in 56..64 {
                let i = (y * w + x) * 3;
                data[i] = 0.55;
                data[i + 1] = 0.22;
                data[i + 2] = 0.05;
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data,
        };
        let mut mask = vec![0.0f32; w * h];
        for y in 36..58 {
            for x in 40..58 {
                mask[y * w + x] = 0.92;
            }
        }
        for y in 40..48 {
            for x in 56..64 {
                mask[y * w + x] = 0.85;
            }
        }
        let before: f32 = mask.iter().sum();
        grow_attached_extremities(&mut mask, w, h, &img);
        let after: f32 = mask.iter().sum();
        // Must not explode into the padded rectangle of water.
        assert!(
            after < before * 2.2,
            "grow flooded water into a box: before={before} after={after}"
        );
        let mut far_water = 0.0f32;
        for y in 8..18 {
            for x in 8..18 {
                far_water += mask[y * w + x];
            }
        }
        assert!(far_water < 3.0, "far water filled: {far_water}");
    }

    /// Puffin/splash regression: dark bird on dark water with bright droplets.
    /// Unbounded grow used to paint a soft rectangle (holes where droplets are).
    #[test]
    fn grow_does_not_box_fill_splash_water() {
        let (w, h) = (96usize, 96usize);
        let mut data = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                // dark water / background
                data[i] = 0.028;
                data[i + 1] = 0.032;
                data[i + 2] = 0.036;
            }
        }
        // Bright splash droplets above the bird (swiss-cheese holes if box-filled)
        for &(sx, sy) in &[
            (40usize, 18),
            (48, 12),
            (55, 20),
            (62, 15),
            (35, 22),
            (70, 24),
            (44, 8),
            (58, 10),
        ] {
            for dy in 0..3 {
                for dx in 0..3 {
                    let i = ((sy + dy) * w + (sx + dx)) * 3;
                    data[i] = 0.85;
                    data[i + 1] = 0.88;
                    data[i + 2] = 0.9;
                }
            }
        }
        // Dark bird body (similar luma to water — the hard case)
        for y in 40..70 {
            for x in 34..62 {
                let i = (y * w + x) * 3;
                data[i] = 0.04;
                data[i + 1] = 0.038;
                data[i + 2] = 0.036;
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data,
        };
        let mut mask = vec![0.0f32; w * h];
        for y in 40..70 {
            for x in 34..62 {
                mask[y * w + x] = 0.9;
            }
        }
        let before: f32 = mask.iter().sum();
        grow_attached_extremities(&mut mask, w, h, &img);
        let after: f32 = mask.iter().sum();
        assert!(
            after < before * 1.8,
            "splash water box fill: before={before} after={after}"
        );
        // Region above the bird (where the red box appeared) must stay mostly clear.
        let mut above = 0.0f32;
        for y in 5..32 {
            for x in 30..70 {
                above += mask[y * w + x];
            }
        }
        assert!(above < 25.0, "upper splash box still painted: {above}");
    }

    /// Soft rectangular water fill + missing black body → wildlife refine peels box.
    #[test]
    fn wildlife_refine_peels_water_box_keeps_bird() {
        let (w, h) = (96usize, 96usize);
        let mut rgb = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                rgb[i] = 0.03;
                rgb[i + 1] = 0.05;
                rgb[i + 2] = 0.12; // clearly blue-led dark water
            }
        }
        // Black body
        for y in 45..72 {
            for x in 30..58 {
                let i = (y * w + x) * 3;
                rgb[i] = 0.03;
                rgb[i + 1] = 0.03;
                rgb[i + 2] = 0.032;
            }
        }
        // White chest
        for y in 50..68 {
            for x in 50..62 {
                let i = (y * w + x) * 3;
                rgb[i] = 0.85;
                rgb[i + 1] = 0.84;
                rgb[i + 2] = 0.82;
            }
        }
        // Orange beak
        for y in 42..50 {
            for x in 56..68 {
                let i = (y * w + x) * 3;
                rgb[i] = 0.75;
                rgb[i + 1] = 0.35;
                rgb[i + 2] = 0.08;
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data: rgb,
        };
        let mut mask = vec![0.0f32; w * h];
        // Soft rectangular "box" over water above the bird
        for y in 8..40 {
            for x in 28..70 {
                mask[y * w + x] = 0.48;
            }
        }
        // Partial bird (head/chest only)
        for y in 42..62 {
            for x in 48..66 {
                mask[y * w + x] = 0.9;
            }
        }
        refine_wildlife_on_water(&mut mask, w, h, &img);
        let mut box_mass = 0.0f32;
        for y in 8..36 {
            for x in 28..70 {
                box_mass += mask[y * w + x];
            }
        }
        let mut bird_mass = 0.0f32;
        for y in 45..70 {
            for x in 32..62 {
                bird_mass += mask[y * w + x];
            }
        }
        assert!(box_mass < 40.0, "water box should be peeled: {box_mass}");
        assert!(bird_mass > 80.0, "bird body should remain/grow: {bird_mass}");
    }

    #[test]
    fn keep_wildlife_prefers_accent_blob_over_larger_water() {
        let (w, h) = (48usize, 32usize);
        let mut rgb = vec![0.0f32; w * h * 3];
        let mut data = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                rgb[i] = 0.04;
                rgb[i + 1] = 0.05;
                rgb[i + 2] = 0.11;
            }
        }
        // Large water rectangle
        for y in 2..28 {
            for x in 2..22 {
                data[y * w + x] = 0.7;
            }
        }
        // Smaller bird: white + orange
        for y in 10..22 {
            for x in 30..44 {
                let i = (y * w + x) * 3;
                rgb[i] = 0.85;
                rgb[i + 1] = 0.84;
                rgb[i + 2] = 0.82;
                data[y * w + x] = 0.8;
            }
        }
        for y in 12..18 {
            for x in 42..47 {
                let i = (y * w + x) * 3;
                rgb[i] = 0.8;
                rgb[i + 1] = 0.35;
                rgb[i + 2] = 0.08;
                data[y * w + x] = 0.85;
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data: rgb,
        };
        keep_wildlife_subject_blob(&mut data, w, h, &img, 0.28);
        let mut water = 0.0f32;
        let mut bird = 0.0f32;
        for y in 2..28 {
            for x in 2..22 {
                water += data[y * w + x];
            }
        }
        for y in 10..22 {
            for x in 30..44 {
                bird += data[y * w + x];
            }
        }
        assert!(water < 8.0, "larger water box should lose: {water}");
        assert!(bird > 20.0, "accent bird should win: {bird}");
    }

    /// Hard (high-confidence) detached water boxes must not survive when a bird core exists.
    #[test]
    fn wildlife_refine_drops_hard_detached_water_box() {
        let (w, h) = (96usize, 96usize);
        let mut rgb = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                rgb[i] = 0.04;
                rgb[i + 1] = 0.055;
                rgb[i + 2] = 0.09;
            }
        }
        for y in 40..70 {
            for x in 48..72 {
                let i = (y * w + x) * 3;
                rgb[i] = 0.03;
                rgb[i + 1] = 0.03;
                rgb[i + 2] = 0.032;
            }
        }
        for y in 44..58 {
            for x in 66..78 {
                let i = (y * w + x) * 3;
                rgb[i] = 0.82;
                rgb[i + 1] = 0.8;
                rgb[i + 2] = 0.78;
            }
        }
        for y in 38..46 {
            for x in 72..84 {
                let i = (y * w + x) * 3;
                rgb[i] = 0.78;
                rgb[i + 1] = 0.4;
                rgb[i + 2] = 0.1;
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data: rgb,
        };
        let mut mask = vec![0.0f32; w * h];
        // Detached hard "left box" over water (the UI failure mode).
        for y in 28..58 {
            for x in 8..28 {
                mask[y * w + x] = 0.88;
            }
        }
        // Soft bridge that used to fuse box into primary.
        for y in 46..52 {
            for x in 28..48 {
                mask[y * w + x] = 0.5;
            }
        }
        // Bird core
        for y in 42..66 {
            for x in 50..76 {
                mask[y * w + x] = 0.92;
            }
        }
        refine_wildlife_on_water(&mut mask, w, h, &img);
        let mut left = 0.0f32;
        for y in 28..58 {
            for x in 8..28 {
                left += mask[y * w + x];
            }
        }
        let mut bird = 0.0f32;
        for y in 42..66 {
            for x in 50..76 {
                bird += mask[y * w + x];
            }
        }
        assert!(left < 25.0, "hard left water box should be cleared: {left}");
        assert!(bird > 120.0, "bird core should remain: {bird}");
    }

    #[test]
    fn subject_refine_does_not_box_silhouette_on_water() {
        let (w, h) = (96usize, 64usize);
        let mut rgb = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                rgb[i] = 0.08;
                rgb[i + 1] = 0.11;
                rgb[i + 2] = 0.16;
            }
        }
        let mut mask = vec![0.0f32; w * h];
        for y in 22..42 {
            for x in 18..78 {
                let nx = (x as f32 - 48.0) / 28.0;
                let ny = (y as f32 - 32.0) / 9.0;
                if nx * nx + ny * ny > 1.0 {
                    continue;
                }
                let i = (y * w + x) * 3;
                rgb[i] = 0.62;
                rgb[i + 1] = 0.58;
                rgb[i + 2] = 0.52;
                mask[y * w + x] = 0.9;
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data: rgb,
        };
        refine_saliency_with_image(&mut mask, w, h, &img);
        let fill = mask_bbox_fill(&mask, w, h, 0.28);
        assert!(fill < 0.82, "subject must stay a silhouette, not a box: {fill}");
        let mut body = 0.0f32;
        for y in 26..38 {
            for x in 36..60 {
                body += mask[y * w + x];
            }
        }
        assert!(body > 40.0, "subject body should remain: {body}");
        assert!(mask[4 * w + 4] < 0.1 && mask[4 * w + 90] < 0.1);
    }

    #[test]
    fn object_grow_captures_blob_only() {
        let img = synthetic_scene();
        let m = TractSegmenter.object(&img, (0.5, 0.75)).unwrap();
        let at = |x: usize, y: usize| m.data[y * m.width + x];
        assert!(at(32, 48) > 0.5, "seed in blob: {}", at(32, 48));
        assert!(at(8, 8) < 0.05, "sky not in object: {}", at(8, 8));
    }

    /// Portrait silhouette + skin oval → non-empty face; legs stay clear.
    #[test]
    fn face_mask_keeps_upper_skin_oval() {
        let (w, h) = (64usize, 96usize);
        let mut rgb = vec![0.0f32; w * h * 3];
        let mut sub = vec![0.0f32; w * h];
        // Neutral backdrop
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                rgb[i] = 0.15;
                rgb[i + 1] = 0.18;
                rgb[i + 2] = 0.22;
            }
        }
        // Body torso (clothing — cool gray, not skin)
        for y in 40..88 {
            for x in 22..42 {
                let i = (y * w + x) * 3;
                rgb[i] = 0.12;
                rgb[i + 1] = 0.14;
                rgb[i + 2] = 0.2;
                sub[y * w + x] = 0.95;
            }
        }
        // Head / face oval with skin tones
        let (cx, cy) = (32.0f32, 22.0f32);
        for y in 8..36 {
            for x in 18..46 {
                let dx = (x as f32 - cx) / 10.0;
                let dy = (y as f32 - cy) / 12.0;
                if dx * dx + dy * dy > 1.0 {
                    continue;
                }
                let i = (y * w + x) * 3;
                rgb[i] = 0.72;
                rgb[i + 1] = 0.52;
                rgb[i + 2] = 0.42;
                sub[y * w + x] = 0.95;
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data: rgb,
        };
        let subject = Mask01 {
            width: w,
            height: h,
            data: sub,
        };
        let face = extract_face_mask(&subject, &img);
        let mut face_mass = 0.0f32;
        let mut leg_mass = 0.0f32;
        for y in 0..h {
            for x in 0..w {
                let v = face.data[y * w + x];
                if y < 38 {
                    face_mass += v;
                } else if y > 55 {
                    leg_mass += v;
                }
            }
        }
        assert!(face_mass > 40.0, "face should light up: {face_mass}");
        assert!(leg_mass < face_mass * 0.15, "legs should stay dark: {leg_mass} vs {face_mass}");
        let cov = face.data.iter().filter(|v| **v > 0.12).count() as f32 / (w * h) as f32;
        assert!(cov > 0.02, "face coverage too small: {cov}");
        assert!(cov < 0.35, "face coverage too large: {cov}");
    }

    /// Shade / cool "skin" that fails the warm skin gate → empty face (not oval fill).
    #[test]
    fn face_mask_empty_without_skin_evidence() {
        let (w, h) = (48usize, 64usize);
        let mut rgb = vec![0.0f32; w * h * 3];
        let mut sub = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                // Cool blue-gray that fails the warm skin gate
                rgb[i] = 0.35;
                rgb[i + 1] = 0.38;
                rgb[i + 2] = 0.45;
            }
        }
        for y in 8..56 {
            for x in 14..34 {
                sub[y * w + x] = 0.9;
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data: rgb,
        };
        let subject = Mask01 {
            width: w,
            height: h,
            data: sub,
        };
        let face = extract_face_mask(&subject, &img);
        let cov = face.data.iter().filter(|v| **v > 0.12).count() as f32 / (w * h) as f32;
        assert!(cov < 0.005, "no-skin face should stay empty: {cov}");
    }

    /// Puffin / eagle: white head + dark circular eye on a compact subject.
    #[test]
    fn eyes_mask_finds_dark_dot_on_white_head() {
        let (w, h) = (96usize, 72usize);
        let mut rgb = vec![0.0f32; w * h * 3];
        let mut sub = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                rgb[i] = 0.04;
                rgb[i + 1] = 0.07;
                rgb[i + 2] = 0.10;
            }
        }
        // White facial disc
        let (cx, cy) = (48.0f32, 28.0f32);
        for y in 12..46 {
            for x in 28..68 {
                let dx = (x as f32 - cx) / 16.0;
                let dy = (y as f32 - cy) / 14.0;
                if dx * dx + dy * dy > 1.0 {
                    continue;
                }
                let i = (y * w + x) * 3;
                rgb[i] = 0.86;
                rgb[i + 1] = 0.84;
                rgb[i + 2] = 0.80;
                sub[y * w + x] = 0.95;
            }
        }
        // Dark eye
        for y in 24..32 {
            for x in 40..48 {
                let dx = (x as f32 - 44.0) / 3.2;
                let dy = (y as f32 - 28.0) / 3.2;
                if dx * dx + dy * dy > 1.0 {
                    continue;
                }
                let i = (y * w + x) * 3;
                rgb[i] = 0.04;
                rgb[i + 1] = 0.04;
                rgb[i + 2] = 0.05;
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data: rgb,
        };
        let subject = Mask01 {
            width: w,
            height: h,
            data: sub,
        };
        let eyes = extract_eyes_mask(&subject, &img);
        let mut eye_mass = 0.0f32;
        for y in 24..33 {
            for x in 40..49 {
                eye_mass += eyes.data[y * w + x];
            }
        }
        assert!(eye_mass > 1.5, "puffin eye should light up: {eye_mass}");
        let face = extract_face_mask(&subject, &img);
        let mut face_mass = 0.0f32;
        for y in 16..40 {
            for x in 32..64 {
                face_mass += face.data[y * w + x];
            }
        }
        assert!(face_mass > 8.0, "white head around the eye should be a face: {face_mass}");
    }

    #[test]
    fn water_mask_keeps_dark_grey_lower() {
        let (w, h) = (64usize, 64usize);
        let mut rgb = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                if y < 22 {
                    rgb[i] = 0.62;
                    rgb[i + 1] = 0.68;
                    rgb[i + 2] = 0.74;
                } else {
                    rgb[i] = 0.16;
                    rgb[i + 1] = 0.18;
                    rgb[i + 2] = 0.19;
                }
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data: rgb,
        };
        let m = extract_water_mask(&img);
        let mut lo = 0.0f32;
        let mut hi = 0.0f32;
        for y in 0..16 {
            for x in 0..w {
                hi += m.data[y * w + x];
            }
        }
        for y in 40..h {
            for x in 0..w {
                lo += m.data[y * w + x];
            }
        }
        assert!(lo > hi * 2.0, "grey lake {lo} should beat pale sky {hi}");
        assert!(lo > 80.0, "grey glacial water should be detected: {lo}");
    }

    #[test]
    fn water_mask_keeps_white_falls_on_pool() {
        let (w, h) = (48usize, 64usize);
        let mut rgb = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                rgb[i] = 0.12;
                rgb[i + 1] = 0.28;
                rgb[i + 2] = 0.16; // moss
            }
        }
        for y in 36..64 {
            for x in 8..40 {
                let i = (y * w + x) * 3;
                rgb[i] = 0.06;
                rgb[i + 1] = 0.16;
                rgb[i + 2] = 0.22; // pool
            }
        }
        for y in 8..40 {
            for x in 18..30 {
                let i = (y * w + x) * 3;
                rgb[i] = 0.92;
                rgb[i + 1] = 0.93;
                rgb[i + 2] = 0.94; // falls
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data: rgb,
        };
        let m = extract_water_mask(&img);
        let pool = m.data[50 * w + 24];
        let fall = m.data[24 * w + 24];
        assert!(pool > 0.15, "pool should be water: {pool}");
        assert!(fall > 0.08, "falls touching the pool should grow in: {fall}");
    }

    /// Bright core + compact darker region attached below (coat, two-tone
    /// animal, object). Grow must recover the dark part without boxing.
    #[test]
    fn grow_recovers_attached_dark_region() {
        let (w, h) = (96usize, 96usize);
        let mut data = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                data[i] = 0.28;
                data[i + 1] = 0.27;
                data[i + 2] = 0.26;
            }
        }
        // Dark oval body (compact — does not touch the frame).
        for y in 32..76 {
            for x in 26..70 {
                let nx = (x as f32 - 48.0) / 18.0;
                let ny = (y as f32 - 52.0) / 18.0;
                if nx * nx + ny * ny > 1.0 {
                    continue;
                }
                let i = (y * w + x) * 3;
                data[i] = 0.04;
                data[i + 1] = 0.04;
                data[i + 2] = 0.045;
            }
        }
        // Light oval core with a wide overlap against the dark body.
        let mut mask = vec![0.0f32; w * h];
        for y in 14..40 {
            for x in 28..68 {
                let nx = (x as f32 - 48.0) / 16.0;
                let ny = (y as f32 - 26.0) / 12.0;
                if nx * nx + ny * ny > 1.0 {
                    continue;
                }
                let i = (y * w + x) * 3;
                data[i] = 0.85;
                data[i + 1] = 0.84;
                data[i + 2] = 0.82;
                mask[y * w + x] = 0.92;
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data,
        };
        grow_attached_extremities(&mut mask, w, h, &img);
        let mut body = 0.0f32;
        for y in 48..68 {
            for x in 36..60 {
                body += mask[y * w + x];
            }
        }
        let mut bg = 0.0f32;
        for y in 8..16 {
            for x in 8..16 {
                bg += mask[y * w + x];
            }
        }
        assert!(body > 80.0, "attached dark region should grow from the light core: {body}");
        assert!(bg < 5.0, "background should stay clear: {bg}");
        let fill = mask_bbox_fill(&mask, w, h, 0.28);
        assert!(fill < 0.86, "two-tone grow must stay a silhouette: {fill}");
        // Combined bbox corners stay off — not an axis-aligned fill.
        assert!(mask[16 * w + 28] < 0.28 && mask[73 * w + 67] < 0.28);
    }

    /// Full subject refine on a non-rectangular two-tone subject over water
    /// must not paint the water bbox.
    #[test]
    fn subject_refine_two_tone_on_water_stays_silhouette() {
        let (w, h) = (96usize, 64usize);
        let mut rgb = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                rgb[i] = 0.07;
                rgb[i + 1] = 0.10;
                rgb[i + 2] = 0.16;
            }
        }
        let mut mask = vec![0.0f32; w * h];
        for y in 18..36 {
            for x in 32..64 {
                let nx = (x as f32 - 48.0) / 12.0;
                let ny = (y as f32 - 26.0) / 8.0;
                if nx * nx + ny * ny > 1.0 {
                    continue;
                }
                let i = (y * w + x) * 3;
                rgb[i] = 0.72;
                rgb[i + 1] = 0.68;
                rgb[i + 2] = 0.62;
                mask[y * w + x] = 0.9;
            }
        }
        for y in 32..56 {
            for x in 30..66 {
                let nx = (x as f32 - 48.0) / 14.0;
                let ny = (y as f32 - 44.0) / 10.0;
                if nx * nx + ny * ny > 1.0 {
                    continue;
                }
                let i = (y * w + x) * 3;
                rgb[i] = 0.12;
                rgb[i + 1] = 0.11;
                rgb[i + 2] = 0.10;
            }
        }
        let img = RgbF32Buf {
            width: w,
            height: h,
            data: rgb,
        };
        refine_saliency_with_image(&mut mask, w, h, &img);
        let fill = mask_bbox_fill(&mask, w, h, 0.28);
        assert!(fill < 0.86, "refine must not box two-tone-on-water: {fill}");
        assert!(mask[4 * w + 4] < 0.1 && mask[4 * w + 90] < 0.1);
        let mut core = 0.0f32;
        for y in 20..32 {
            for x in 40..56 {
                core += mask[y * w + x];
            }
        }
        assert!(core > 20.0, "light core should remain: {core}");
    }

    #[test]
    fn keep_strongest_blobs_drops_weak_island() {
        let (w, h) = (32usize, 32usize);
        let mut data = vec![0.0f32; w * h];
        for y in 4..14 {
            for x in 4..14 {
                data[y * w + x] = 0.9;
            }
        }
        for y in 20..24 {
            for x in 20..24 {
                data[y * w + x] = 0.5;
            }
        }
        keep_strongest_blobs(&mut data, w, h, 0.2, 1);
        assert!(data[8 * w + 8] > 0.5);
        assert!(data[22 * w + 22] < 0.05);
    }

    #[test]
    #[ignore = "loads the 4.5MB model; run with --ignored"]
    fn u2netp_subject_runs_and_outputs_mask() {
        let img = synthetic_scene();
        let m = TractSegmenter.subject(&img).unwrap();
        assert_eq!((m.width, m.height), (NET_SIZE, NET_SIZE));
        assert!(m.data.iter().all(|v| (0.0..=1.0).contains(v)));
        let hi = m.data.iter().cloned().fold(0.0f32, f32::max);
        assert!(hi > 0.9);
    }
}
