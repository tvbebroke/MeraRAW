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
}

impl Segmenter for TractSegmenter {
    fn subject(&self, img: &RgbF32Buf) -> Result<Mask01, CoreError> {
        let mut mask = run_u2net(subject_model()?, img)?;
        refine_saliency_with_image(&mut mask.data, mask.width, mask.height, img);
        Ok(mask)
    }

    /// Sky via dedicated U²-Net weights (MIT — xiongzhu666 Sky-Segmentation).
    fn sky(&self, img: &RgbF32Buf) -> Result<Mask01, CoreError> {
        run_u2net(sky_model()?, img)
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
