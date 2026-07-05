//! Master tone curve orchestration (shadows/darks/lights/highlights + contrast).
//! Live params: `tone_curve.{shadows,darks,lights,highlights,contrast,points}`.
#![allow(dead_code, unused_variables)]

use crate::pipeline::RgbImage;

pub fn apply(img: &mut RgbImage) {
    // Identity placeholder — the real parametric tone curve is in core/src/curve.rs.
}
