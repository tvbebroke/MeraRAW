//! Segmenter — contract B2. On-device, offline, private: pixel masks come
//! from local models, never the cloud (the conductor split, spec 4.2).
//!
//! Primary: tract-onnx (pure Rust — no FFI/runtime download; the `ort`
//! CoreML path is a contained swap behind this trait).
//! Subject: bundled u2netp. Sky: bundled U²-Net skyseg (MIT, xiongzhu666).
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

fn subject_model() -> Result<&'static TractModel, CoreError> {
    U2NETP
        .get_or_init(|| compile_u2net(SUBJECT_MODEL_BYTES))
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
    for v in data.iter_mut() {
        *v = ((*v - 0.5) * 1.4 + 0.5).clamp(0.0, 1.0);
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
    suppress_reflections_and_islands(data, w, h);
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
        let below = cy > p_cy + (h as f32) * 0.04 && b.min_y as f32 >= p_bottom - (h as f32) * 0.02;
        let weaker = mean < p_mean * 0.92;
        if below && weaker {
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

fn sample_enc_rgb(img: &RgbF32Buf, mx: usize, my: usize, mw: usize, mh: usize) -> [f32; 3] {
    let sx = ((mx as f32 + 0.5) * img.width as f32 / mw as f32) as usize;
    let sy = ((my as f32 + 0.5) * img.height as f32 / mh as f32) as usize;
    let i = (sy.min(img.height.saturating_sub(1)) * img.width + sx.min(img.width.saturating_sub(1)))
        * 3;
    [enc(img.data[i]), enc(img.data[i + 1]), enc(img.data[i + 2])]
}

fn is_skin_rgb(r: f32, g: f32, b: f32) -> f32 {
    // Classic encoded-RGB skin gate — tuned for portrait subjects.
    if r < 0.12 || g < 0.06 || b < 0.04 {
        return 0.0;
    }
    if !(r > g && g > b * 0.85) {
        return 0.0;
    }
    let rg = r - g;
    if rg < 0.015 || rg > 0.42 {
        return 0.0;
    }
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let chroma = mx - mn;
    if chroma < 0.04 || chroma > 0.55 {
        return 0.0;
    }
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    if !(0.12..=0.92).contains(&luma) {
        return 0.0;
    }
    ((rg - 0.015) / 0.25).clamp(0.0, 1.0) * ((0.55 - chroma) / 0.4).clamp(0.2, 1.0)
}

fn is_hair_rgb(r: f32, g: f32, b: f32, y_norm: f32) -> f32 {
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let chroma = mx - mn;
    // Prefer darker, lower-chroma pixels in the upper part of the subject.
    let dark = (1.0 - (luma / 0.45).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let low_c = (1.0 - (chroma / 0.35).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let upper = (1.0 - y_norm * 1.35).clamp(0.0, 1.0);
    dark * low_c * upper
}

/// Restrict a subject mask to skin-tone pixels.
pub fn extract_skin_mask(subject: &Mask01, img: &RgbF32Buf) -> Mask01 {
    let (w, h) = (subject.width, subject.height);
    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let s = subject.data[i];
            if s < 0.12 {
                continue;
            }
            let [r, g, b] = sample_enc_rgb(img, x, y, w, h);
            data[i] = (s * is_skin_rgb(r, g, b)).clamp(0.0, 1.0);
        }
    }
    suppress_reflections_and_islands(&mut data, w, h);
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
            if s < 0.15 {
                continue;
            }
            let y_norm = (y.saturating_sub(y0) as f32) / span;
            let [r, g, b] = sample_enc_rgb(img, x, y, w, h);
            // Hair should not also score strongly as skin.
            let skin = is_skin_rgb(r, g, b);
            let hair = is_hair_rgb(r, g, b, y_norm) * (1.0 - skin * 0.85);
            data[i] = (s * hair).clamp(0.0, 1.0);
        }
    }
    suppress_reflections_and_islands(&mut data, w, h);
    Mask01 {
        width: w,
        height: h,
        data,
    }
}

/// Propose clickable object/subject outlines from saliency (keeps multiple blobs).
pub fn propose_objects(img: &RgbF32Buf) -> Result<Vec<ObjectProposal>, CoreError> {
    let mut mask = run_u2net(subject_model()?, img)?;
    // Soft refine without killing secondary subjects — proposals want plural blobs.
    refine_saliency_mask(&mut mask.data, mask.width, mask.height);
    Ok(proposals_from_saliency(&mask.data, mask.width, mask.height))
}

fn proposals_from_saliency(data: &[f32], w: usize, h: usize) -> Vec<ObjectProposal> {
    const T: f32 = 0.28;
    let mut labels = vec![0u32; w * h];
    let mut stats: Vec<(u32, f32, f32, f32)> = Vec::new(); // area, sum, sum_x, sum_y
    let mut stack = Vec::new();

    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if data[i] < T || labels[i] != 0 {
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
                    if labels[k] == 0 && data[k] >= T {
                        labels[k] = id;
                        stack.push(k);
                    }
                }
            }
            stats.push((area, sum, sx, sy));
        }
    }

    let img_area = (w * h) as f32;
    let min_area = (img_area * 0.012).max(24.0) as u32;
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
        return ((px - ax).hypot(py - ay));
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
    refine_saliency_mask(&mut mask.data, mask.width, mask.height);
    if !keep_blob_at_point(&mut mask.data, mask.width, mask.height, point, 0.28) {
        // Click missed a saliency blob — fall back to color region-grow.
        return TractSegmenter.object(img, point);
    }
    // Soft fringe cleanup without killing the chosen instance.
    let (w, h) = (mask.width, mask.height);
    let blurred = box_blur_3x3(&mask.data, w, h);
    for i in 0..mask.data.len() {
        if mask.data[i] > 0.05 {
            mask.data[i] = (mask.data[i] * 0.75 + blurred[i] * 0.25).clamp(0.0, 1.0);
        }
    }
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
        // Search a small neighborhood for a strong seed (outline click may land on edge).
        let mut best = None;
        let mut best_v = thresh;
        for dy in -3i32..=3 {
            for dx in -3i32..=3 {
                let nx = cx as i32 + dx;
                let ny = cy as i32 + dy;
                if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                    continue;
                }
                let i = ny as usize * w + nx as usize;
                if data[i] > best_v {
                    best_v = data[i];
                    best = Some(i);
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
                data[i] *= 0.08;
            } else if y_norm > 0.62 && luma < 0.42 && b > r {
                data[i] *= 0.12;
            } else if y_norm < 0.35 && luma > 0.45 && b >= g * 0.9 {
                // Boost pale upper sky a touch.
                data[i] = (data[i] * 1.08).clamp(0.0, 1.0);
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
        if cy > h as f32 * 0.52 && upper_frac < 0.28 {
            continue;
        }
        // Prefer anything with a real foothold in the top half.
        if upper_frac >= 0.18 || cy < h as f32 * 0.45 {
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
}

fn water_score(r: f32, g: f32, b: f32, y_norm: f32) -> f32 {
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    if b < r + 0.02 || b < g * 0.88 {
        return 0.0;
    }
    if !(0.04..=0.62).contains(&luma) {
        return 0.0;
    }
    let blue = ((b - r.max(g)) / 0.25).clamp(0.0, 1.0);
    let depth = (1.0 - (luma / 0.55).clamp(0.0, 1.0)).clamp(0.15, 1.0);
    // Prefer mid/lower frame — sky lives up top.
    let lower = ((y_norm - 0.28) / 0.55).clamp(0.0, 1.0);
    blue * depth * (0.25 + 0.75 * lower)
}

fn vegetation_score(r: f32, g: f32, b: f32) -> f32 {
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    if g < r * 1.02 || g < b * 1.02 {
        return 0.0;
    }
    if !(0.05..=0.75).contains(&luma) {
        return 0.0;
    }
    let green = ((g - r.max(b)) / 0.28).clamp(0.0, 1.0);
    let mid = (1.0 - ((luma - 0.35).abs() / 0.4)).clamp(0.2, 1.0);
    green * mid
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
        let thresh = (0.18 * (lum + 0.2)).max(0.05);
        let mut mask = vec![0.0f32; w * h];
        let mut stack = vec![seed_i];
        let mut visited = 0usize;
        while let Some(i) = stack.pop() {
            if mask[i] > 0.0 {
                continue;
            }
            let d = dist(i);
            if d > thresh {
                continue;
            }
            mask[i] = (1.0 - d / thresh).clamp(0.3, 1.0);
            visited += 1;
            if visited > w * h / 2 {
                break;
            }
            let (x, y) = (i % w, i / w);
            for (nx, ny) in [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ] {
                if nx < w && ny < h {
                    let j = ny * w + nx;
                    if mask[j] == 0.0 {
                        stack.push(j);
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
