//! Per-channel + luma tone curves (control-point interpolation).
//! Live impl: `core/src/curve.rs` (baked LUT) + `core/src/graph/curve.wgsl`.
#![allow(dead_code, unused_variables)]

use crate::pipeline::RgbImage;

/// Evaluate a monotone curve of sorted (x, y) control points, linearly interpolated.
pub fn eval(points: &[(f32, f32)], x: f32) -> f32 {
    if points.is_empty() {
        return x;
    }
    if x <= points[0].0 {
        return points[0].1;
    }
    for w in points.windows(2) {
        let (x0, y0) = w[0];
        let (x1, y1) = w[1];
        if x <= x1 {
            let t = (x - x0) / (x1 - x0).max(1e-6);
            return y0 + (y1 - y0) * t;
        }
    }
    points[points.len() - 1].1
}

pub fn apply_luma(img: &mut RgbImage, points: &[(f32, f32)]) {
    for v in img.data.iter_mut() {
        *v = eval(points, *v);
    }
}
