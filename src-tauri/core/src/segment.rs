//! Segmenter — contract B2. On-device, offline, private: pixel masks come
//! from local models, never the cloud (the conductor split, spec 4.2).
//!
//! Primary: tract-onnx (pure Rust — no FFI/runtime download; the `ort`
//! CoreML path is a contained swap behind this trait).
//! Subject: bundled u2netp, or override via `MERARAW_SUBJECT_MODEL` /
//! `~/Library/Application Support/MeraRAW/models/subject/merasubject-v1.onnx`
//! (see `segment/train/`). Sky: bundled U²-Net skyseg (MIT, xiongzhu666).
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

/// Edge-aware refinement using color similarity at mask boundaries.
fn refine_saliency_with_image(data: &mut [f32], w: usize, h: usize, img: &RgbF32Buf) {
    refine_saliency_mask(data, w, h);
    if w < 3 || h < 3 {
        return;
    }
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
    // Pull in dark attached extremities (bird wings, legs) that saliency under-scores
    // against dark water / background.
    grow_attached_extremities(data, w, h, img);
    suppress_reflections_and_islands(data, w, h);
    // Attached underside bleed (otter/animal reflections fused into the subject blob).
    suppress_underside_water_bleed(data, w, h, img);
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

/// Expand the subject into dark / body-similar pixels attached to the mask.
/// Fixes birds/animals where wings or limbs vanish into dark water/ground.
///
/// Critical: never fill the padded bbox with dark water. Dark pixels must stay
/// closer to the body color than to the outside background, or the mask becomes
/// a solid rectangle (the classic “red square instead of the bird” failure).
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

    // Expand bbox so wings sticking out still count as "near".
    let pad_x = ((x1.saturating_sub(x0) as f32) * 0.45).max(8.0) as usize;
    let pad_y = ((y1.saturating_sub(y0) as f32) * 0.45).max(8.0) as usize;
    let bx0 = x0.saturating_sub(pad_x);
    let bx1 = (x1 + pad_x).min(w.saturating_sub(1));
    let by0 = y0.saturating_sub(pad_y);
    let by1 = (y1 + pad_y).min(h.saturating_sub(1));

    let bg = sample_outside_bg_mean(img, w, h, bx0, bx1, by0, by1);

    let max_grow = ((w * h) as f32 * 0.12).max(48.0) as usize;
    let mut grown = 0usize;
    let mut queue: Vec<usize> = Vec::new();
    // Seed from the hard mask edge (and any soft fringe) so solid saliency cores still expand.
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
                queue.push(i);
            }
        }
    }

    let mut qi = 0usize;
    while qi < queue.len() && grown < max_grow {
        let i = queue[qi];
        qi += 1;
        let x = i % w;
        let y = i / w;
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
            let rgb = sample_img_rgb(img, nx, ny, w, h);
            if is_waterish_rgb(rgb[0], rgb[1], rgb[2]) {
                continue;
            }
            let d_body = rgb_dist(rgb, mean);
            // Prefer body over background — stops dark water from filling the bbox square.
            if let Some(bg_mean) = bg {
                let d_bg = rgb_dist(rgb, bg_mean);
                if d_bg + 0.015 < d_body {
                    continue;
                }
            }
            let dark = is_dark_extremity_rgb(rgb[0], rgb[1], rgb[2]);
            // Dark extremities still need to be near body color — "any dark" flooded water.
            let body_like = d_body <= color_thresh * if dark { 1.65 } else { 1.0 };
            let dark_ok = dark && d_body <= color_thresh * 2.15;
            if !body_like && !dark_ok {
                continue;
            }
            // Dark extremities get a solid boost; body-like midtones a softer one.
            let boost = if dark_ok {
                0.62
            } else {
                (0.55 * (1.0 - d_body / (color_thresh * 1.65 + 1e-5))).clamp(0.3, 0.55)
            };
            if boost > data[j] {
                data[j] = boost;
                grown += 1;
                queue.push(j);
            }
        }
    }

    if grown > 0 {
        let blurred = box_blur_3x3(data, w, h);
        for i in 0..data.len() {
            if data[i] > 0.2 && data[i] < 0.75 {
                data[i] = (data[i] * 0.7 + blurred[i] * 0.3).clamp(0.0, 1.0);
            }
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
    // Encoded-RGB skin gate — covers light → deep skin, olive, and warm undertones.
    if r < 0.05 || g < 0.03 || b < 0.015 {
        return 0.0;
    }
    // Skin is typically R ≥ G ≥ B (or near), allowing deep / olive tones.
    if r + 0.02 < g || g + 0.04 < b * 0.9 {
        return 0.0;
    }
    let rg = r - g;
    if rg < -0.02 || rg > 0.48 {
        return 0.0;
    }
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let chroma = mx - mn;
    if chroma < 0.025 || chroma > 0.62 {
        return 0.0;
    }
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    if !(0.06..=0.94).contains(&luma) {
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

/// Face ≈ upper subject ∩ skin (portrait prior), biased toward center.
pub fn extract_face_mask(subject: &Mask01, img: &RgbF32Buf) -> Mask01 {
    let (w, h) = (subject.width, subject.height);
    let (x0, x1, y0, y1) = subject_bbox(subject);
    let span_y = (y1.saturating_sub(y0)).max(1) as f32;
    let span_x = (x1.saturating_sub(x0)).max(1) as f32;
    let cx = (x0 + x1) as f32 * 0.5;
    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        let y_norm = (y.saturating_sub(y0) as f32) / span_y;
        // Face lives in the upper ~58% of the subject bbox.
        if y_norm > 0.58 {
            continue;
        }
        for x in 0..w {
            let i = y * w + x;
            let s = subject.data[i];
            if s < 0.12 {
                continue;
            }
            let x_norm = (x as f32 - cx).abs() / (span_x * 0.5);
            // Soft center bias — faces are rarely at the outer edges of the subject.
            let center = (1.0 - (x_norm - 0.15).max(0.0) / 0.95).clamp(0.2, 1.0);
            let [r, g, b] = sample_enc_rgb(img, x, y, w, h);
            let skin = is_skin_rgb(r, g, b);
            let upper = (1.0 - y_norm / 0.58).clamp(0.2, 1.0);
            data[i] = (s * skin.max(0.15) * upper * center).clamp(0.0, 1.0);
        }
    }
    soft_cleanup_part(&mut data, w, h);
    Mask01 {
        width: w,
        height: h,
        data,
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
    // Iris / pupil: dark low-chroma; sclera: bright low-chroma; mild blue iris boost.
    let dark = (1.0 - luma / 0.3).clamp(0.0, 1.0) * (1.0 - chroma / 0.28).clamp(0.15, 1.0);
    let sclera = ((luma - 0.52) / 0.38).clamp(0.0, 1.0) * (1.0 - chroma / 0.22).clamp(0.0, 1.0);
    let blue_iris = ((b - r) / 0.2).clamp(0.0, 1.0) * ((0.45 - luma).max(0.0) / 0.35 + 0.2);
    dark.max(sclera * 0.9).max(blue_iris * 0.55)
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

/// Eyes ≈ upper-face band ∩ dark iris / bright sclera, left+right bias.
pub fn extract_eyes_mask(subject: &Mask01, img: &RgbF32Buf) -> Mask01 {
    let face = extract_face_mask(subject, img);
    let (w, h) = (face.width, face.height);
    let (fx0, fx1, fy0, fy1) = subject_bbox(&face);
    let span_y = (fy1.saturating_sub(fy0)).max(1) as f32;
    let span_x = (fx1.saturating_sub(fx0)).max(1) as f32;
    let cx = (fx0 + fx1) as f32 * 0.5;
    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        let y_norm = (y.saturating_sub(fy0) as f32) / span_y;
        if !(0.18..=0.48).contains(&y_norm) {
            continue;
        }
        for x in 0..w {
            let i = y * w + x;
            if face.data[i] < 0.05 {
                continue;
            }
            // Eyes sit off-center horizontally within the face.
            let x_off = ((x as f32 - cx).abs() / (span_x * 0.5)).clamp(0.0, 1.0);
            let eye_zone = if (0.12..=0.72).contains(&x_off) {
                1.0
            } else {
                0.25
            };
            let [r, g, b] = sample_enc_rgb(img, x, y, w, h);
            data[i] = (face.data[i].max(0.3) * is_eye_rgb(r, g, b) * eye_zone).clamp(0.0, 1.0);
        }
    }
    soft_cleanup_part(&mut data, w, h);
    Mask01 {
        width: w,
        height: h,
        data,
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
    // Ocean / lake / river: blue-cyan, mid-lower frame; allow brighter shallows.
    let blue_lead = b > r + 0.015 && b >= g * 0.85;
    let teal = g > r + 0.02 && b > r + 0.02 && (b - r) > 0.02;
    if !blue_lead && !teal {
        return 0.0;
    }
    if !(0.03..=0.72).contains(&luma) {
        return 0.0;
    }
    let blue = ((b - r.max(g * 0.9)) / 0.28).clamp(0.0, 1.0);
    let cyan = ((b.min(g) - r) / 0.22).clamp(0.0, 1.0);
    let depth = (1.0 - (luma / 0.62).clamp(0.0, 1.0)).clamp(0.12, 1.0);
    let lower = ((y_norm - 0.24) / 0.6).clamp(0.0, 1.0);
    blue.max(cyan * 0.85) * depth * (0.2 + 0.8 * lower)
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

/// Landscape water (ocean / lake / river) from color + vertical priors.
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
    // Kill tiny speckles; keep large lower bodies.
    refine_saliency_mask(&mut data, w, h);
    suppress_upper_false_water(&mut data, w, h);
    Mask01 {
        width: w,
        height: h,
        data,
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

    #[test]
    fn object_grow_captures_blob_only() {
        let img = synthetic_scene();
        let m = TractSegmenter.object(&img, (0.5, 0.75)).unwrap();
        let at = |x: usize, y: usize| m.data[y * m.width + x];
        assert!(at(32, 48) > 0.5, "seed in blob: {}", at(32, 48));
        assert!(at(8, 8) < 0.05, "sky not in object: {}", at(8, 8));
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
