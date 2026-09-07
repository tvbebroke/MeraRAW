//! On-device human face boxes (UltraFace RFB-320, MIT — Linzaer).
//! Wildlife eyes / animal heads stay in `segment` — this model is people only.

use crate::error::CoreError;
use crate::image::RgbF32Buf;
use std::sync::OnceLock;
use tract_onnx::prelude::*;

type TractModel = std::sync::Arc<TypedSimplePlan>;

const MODEL_BYTES: &[u8] = include_bytes!("../models/ultraface-rfb320.onnx");
const NET_W: usize = 320;
const NET_H: usize = 240;
const CENTER_VAR: f32 = 0.1;
const SIZE_VAR: f32 = 0.2;
const SCORE_THRESH: f32 = 0.62;
const NMS_IOU: f32 = 0.3;

#[derive(Debug, Clone, Copy)]
pub struct FaceBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub score: f32,
}

impl FaceBox {
    pub fn width(self) -> f32 {
        (self.x1 - self.x0).max(0.0)
    }
    pub fn height(self) -> f32 {
        (self.y1 - self.y0).max(0.0)
    }
    pub fn cx(self) -> f32 {
        (self.x0 + self.x1) * 0.5
    }
    pub fn cy(self) -> f32 {
        (self.y0 + self.y1) * 0.5
    }
}

static MODEL: OnceLock<Result<TractModel, String>> = OnceLock::new();
static PRIORS: OnceLock<Vec<[f32; 4]>> = OnceLock::new();

fn priors() -> &'static [[f32; 4]] {
    PRIORS.get_or_init(generate_priors)
}

fn generate_priors() -> Vec<[f32; 4]> {
    let feature_w = [40usize, 20, 10, 5];
    let feature_h = [30usize, 15, 8, 4];
    let shrinkage = [8.0f32, 16.0, 32.0, 64.0];
    let min_boxes: [&[f32]; 4] = [
        &[10.0, 16.0, 24.0],
        &[32.0, 48.0],
        &[64.0, 96.0],
        &[128.0, 192.0, 256.0],
    ];
    let mut out = Vec::with_capacity(4420);
    for index in 0..feature_w.len() {
        let scale_w = NET_W as f32 / shrinkage[index];
        let scale_h = NET_H as f32 / shrinkage[index];
        for j in 0..feature_h[index] {
            for i in 0..feature_w[index] {
                let x_center = (i as f32 + 0.5) / scale_w;
                let y_center = (j as f32 + 0.5) / scale_h;
                for &min_box in min_boxes[index] {
                    out.push([
                        x_center.clamp(0.0, 1.0),
                        y_center.clamp(0.0, 1.0),
                        (min_box / NET_W as f32).clamp(0.0, 1.0),
                        (min_box / NET_H as f32).clamp(0.0, 1.0),
                    ]);
                }
            }
        }
    }
    out
}

fn compile(bytes: &[u8]) -> Result<TractModel, String> {
    let mut cursor = std::io::Cursor::new(bytes);
    tract_onnx::onnx()
        .model_for_read(&mut cursor)
        .and_then(|m| {
            m.with_input_fact(
                0,
                InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 3, NET_H, NET_W)),
            )
        })
        .and_then(|m| m.into_optimized())
        .and_then(|m| m.into_runnable())
        .map_err(|e| e.to_string())
}

fn model() -> Result<&'static TractModel, CoreError> {
    MODEL
        .get_or_init(|| compile(MODEL_BYTES))
        .as_ref()
        .map_err(|e| CoreError::Engine(format!("ultraface load: {e}")))
}

fn enc(v: f32) -> f32 {
    v.clamp(0.0, 1.0).powf(1.0 / 2.2)
}

/// Detect people faces. Empty on load/inference failure — callers fall back.
pub fn detect_faces(img: &RgbF32Buf) -> Vec<FaceBox> {
    match detect_faces_inner(img) {
        Ok(v) => v,
        Err(_) => Vec::new(),
    }
}

fn detect_faces_inner(img: &RgbF32Buf) -> Result<Vec<FaceBox>, CoreError> {
    if img.width < 128 || img.height < 128 {
        return Ok(Vec::new());
    }
    let plan = model()?;
    let mut input = vec![0.0f32; 3 * NET_W * NET_H];
    for y in 0..NET_H {
        let sy = (y * img.height / NET_H).min(img.height.saturating_sub(1));
        for x in 0..NET_W {
            let sx = (x * img.width / NET_W).min(img.width.saturating_sub(1));
            let i = (sy * img.width + sx) * 3;
            for c in 0..3 {
                let byte = enc(img.data[i + c]) * 255.0;
                input[c * NET_W * NET_H + y * NET_W + x] = (byte - 127.0) / 128.0;
            }
        }
    }
    let tensor = Tensor::from_shape(&[1, 3, NET_H, NET_W], &input)
        .map_err(|e| CoreError::Engine(format!("ultraface tensor: {e}")))?;
    let result = plan
        .run(tvec!(tensor.into()))
        .map_err(|e| CoreError::Engine(format!("ultraface run: {e}")))?;

    let mut scores: Option<Vec<f32>> = None;
    let mut boxes: Option<Vec<f32>> = None;
    for t in result.iter() {
        let view = t.view();
        let shape = view.shape();
        let slice = view
            .as_slice::<f32>()
            .map_err(|e| CoreError::Engine(format!("ultraface out: {e}")))?;
        if shape.len() >= 3 && shape[2] == 2 {
            scores = Some(slice.to_vec());
        } else if shape.len() >= 3 && shape[2] == 4 {
            boxes = Some(slice.to_vec());
        }
    }
    let scores = scores.ok_or_else(|| CoreError::Engine("ultraface: no scores".into()))?;
    let boxes = boxes.ok_or_else(|| CoreError::Engine("ultraface: no boxes".into()))?;
    Ok(decode_detections(&scores, &boxes))
}

fn decode_detections(scores: &[f32], boxes: &[f32]) -> Vec<FaceBox> {
    let priors = priors();
    let n = priors.len().min(scores.len() / 2).min(boxes.len() / 4);
    let mut cands = Vec::new();
    for i in 0..n {
        let face_p = scores[i * 2 + 1];
        if face_p < SCORE_THRESH {
            continue;
        }
        let p = priors[i];
        let lx = boxes[i * 4];
        let ly = boxes[i * 4 + 1];
        let lw = boxes[i * 4 + 2];
        let lh = boxes[i * 4 + 3];
        let cx = lx * CENTER_VAR * p[2] + p[0];
        let cy = ly * CENTER_VAR * p[3] + p[1];
        let w = (lw * SIZE_VAR).exp() * p[2];
        let h = (lh * SIZE_VAR).exp() * p[3];
        let x0 = (cx - w * 0.5).clamp(0.0, 1.0);
        let y0 = (cy - h * 0.5).clamp(0.0, 1.0);
        let x1 = (cx + w * 0.5).clamp(0.0, 1.0);
        let y1 = (cy + h * 0.5).clamp(0.0, 1.0);
        if x1 - x0 < 0.012 || y1 - y0 < 0.012 {
            continue;
        }
        cands.push(FaceBox {
            x0,
            y0,
            x1,
            y1,
            score: face_p,
        });
    }
    hard_nms(cands)
}

fn iou(a: FaceBox, b: FaceBox) -> f32 {
    let x0 = a.x0.max(b.x0);
    let y0 = a.y0.max(b.y0);
    let x1 = a.x1.min(b.x1);
    let y1 = a.y1.min(b.y1);
    let inter = (x1 - x0).max(0.0) * (y1 - y0).max(0.0);
    let union = a.width() * a.height() + b.width() * b.height() - inter;
    if union <= 1e-6 {
        0.0
    } else {
        inter / union
    }
}

fn hard_nms(mut boxes: Vec<FaceBox>) -> Vec<FaceBox> {
    boxes.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let mut keep = Vec::new();
    'outer: for b in boxes {
        for k in &keep {
            if iou(b, *k) > NMS_IOU {
                continue 'outer;
            }
        }
        keep.push(b);
        if keep.len() >= 12 {
            break;
        }
    }
    keep
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ultraface_prior_count() {
        assert_eq!(generate_priors().len(), 4420);
    }

    #[test]
    fn detect_faces_empty_on_tiny_image() {
        let img = RgbF32Buf {
            width: 4,
            height: 4,
            data: vec![0.2; 4 * 4 * 3],
        };
        assert!(detect_faces(&img).is_empty());
    }
}
