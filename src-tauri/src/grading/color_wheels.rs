//! Lift / Gamma / Gain (shadows / midtones / highlights) color wheels.
//! Live impl: `core/src/graph/grade.wgsl`
//! (params: `color_grade.{shadows,midtones,highlights}_{hue,sat,lum}`).
#![allow(dead_code, unused_variables)]

use crate::pipeline::RgbImage;

#[derive(Default, Clone, Copy)]
pub struct Wheel {
    pub hue: f32,
    pub sat: f32,
    pub lum: f32,
}

pub fn apply(img: &mut RgbImage, shadows: Wheel, midtones: Wheel, highlights: Wheel) {
    // The real 3-model implementation lives in grade.wgsl; CPU mirror stub.
}
