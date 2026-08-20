//! Segmenter — contract B2. On-device, offline, private: pixel masks come
//! from local models, never the cloud (the conductor split, spec 4.2).
//!
//! Primary: tract-onnx (pure Rust — no FFI/runtime download; the `ort`
//! CoreML path is a contained swap behind this trait). Subject model:
//! bundled u2netp. Sky: spectral heuristic (model upgrade slots in here).
//! Object-by-point: color-similarity region grow seeded at the point.

use crate::error::CoreError;
use crate::image::RgbF32Buf;
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

const MODEL_BYTES: &[u8] = include_bytes!("../models/u2netp.onnx");
const NET_SIZE: usize = 320;

fn model() -> Result<&'static TractModel, CoreError> {
    U2NETP
        .get_or_init(|| {
            let mut cursor = std::io::Cursor::new(MODEL_BYTES);
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
        })
        .as_ref()
        .map_err(|e| CoreError::Engine(format!("u2netp load: {e}")))
}

/// Display-ish gamma for model input (models train on encoded images).
fn enc(v: f32) -> f32 {
    v.clamp(0.0, 1.0).powf(1.0 / 2.2)
}

impl Segmenter for TractSegmenter {
    fn subject(&self, img: &RgbF32Buf) -> Result<Mask01, CoreError> {
        let plan = model()?;
        // letterbox-free squash resize to 320² (u2net convention)
        let mean = [0.485f32, 0.456, 0.406];
        let std = [0.229f32, 0.224, 0.225];
        let mut input = vec![0.0f32; 3 * NET_SIZE * NET_SIZE];
        for y in 0..NET_SIZE {
            let sy = (y * img.height / NET_SIZE).min(img.height - 1);
            for x in 0..NET_SIZE {
                let sx = (x * img.width / NET_SIZE).min(img.width - 1);
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
        // min-max normalize (rembg post-processing)
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

    /// Sky heuristic: bright + blue-ish + connected to the top edge.
    /// (A dedicated sky model slots in behind this trait later.)
    fn sky(&self, img: &RgbF32Buf) -> Result<Mask01, CoreError> {
        let (w, h) = (img.width, img.height);
        let mut score = vec![0.0f32; w * h];
        for i in 0..w * h {
            let r = img.data[i * 3];
            let g = img.data[i * 3 + 1];
            let b = img.data[i * 3 + 2];
            let luma = 0.2627 * r + 0.678 * g + 0.0593 * b;
            let blueness = (b - r).max(0.0) / (luma + 0.05);
            let bright = (luma / 0.5).clamp(0.0, 1.0);
            score[i] = ((blueness * 2.0).clamp(0.0, 1.0) * 0.6 + bright * 0.4).clamp(0.0, 1.0);
        }
        // connectivity: flood from top rows over high-score pixels
        let mut mask = vec![0.0f32; w * h];
        let mut stack: Vec<usize> = (0..w)
            .chain(w..2 * w.min(w * h))
            .filter(|i| score[*i] > 0.55)
            .collect();
        while let Some(i) = stack.pop() {
            if mask[i] > 0.0 {
                continue;
            }
            mask[i] = 1.0;
            let (x, y) = (i % w, i / w);
            for (nx, ny) in [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ] {
                if nx < w && ny < h {
                    let j = ny * w + nx;
                    if mask[j] == 0.0 && score[j] > 0.45 {
                        stack.push(j);
                    }
                }
            }
        }
        // soften by score so edges aren't binary
        for i in 0..w * h {
            mask[i] *= 0.5 + 0.5 * score[i];
        }
        Ok(Mask01 {
            width: w,
            height: h,
            data: mask,
        })
    }

    /// Object-by-point: color-similarity region grow seeded at the point.
    /// (Promptable SAM-style model slots in behind this trait later.)
    fn object(&self, img: &RgbF32Buf, point: (f32, f32)) -> Result<Mask01, CoreError> {
        let (w, h) = (img.width, img.height);
        let cx = ((point.0.clamp(0.0, 1.0) * w as f32) as usize).min(w - 1);
        let cy = ((point.1.clamp(0.0, 1.0) * h as f32) as usize).min(h - 1);
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
                break; // runaway grow guard
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

fn tr_err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Engine(format!("segmentation: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_scene() -> RgbF32Buf {
        // 64×64: blue bright top half (sky), dark red blob bottom-center
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
                // red blob
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

    #[test]
    fn sky_heuristic_finds_top_blue() {
        let img = synthetic_scene();
        let m = TractSegmenter.sky(&img).unwrap();
        let at = |x: usize, y: usize| m.data[y * m.width + x];
        assert!(at(32, 8) > 0.5, "sky top: {}", at(32, 8));
        assert!(at(32, 60) < 0.1, "ground must not be sky: {}", at(32, 60));
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
        // normalized output must span the range
        let hi = m.data.iter().cloned().fold(0.0f32, f32::max);
        assert!(hi > 0.9);
    }
}
