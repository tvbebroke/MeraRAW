//! Orders the grading stages, mirroring the grade node in
//! `core/src/graph/render.rs`.
#![allow(dead_code, unused_variables)]

use crate::pipeline::{GradeParams, RgbImage};

pub fn apply(img: &mut RgbImage, params: &GradeParams) {
    crate::grading::saturation::apply(img, 0.0);
    // wheels / hsl / split-tone / vibrance chain here — see grade.wgsl for order.
}
