//! Per-channel WB multipliers. Params: `white_balance.{temp,tint}` ->
//! `core/src/graph/calibration.wgsl`.
#![allow(dead_code, unused_variables)]

use crate::pipeline::RgbImage;

pub fn apply_white_balance(img: &mut RgbImage, gains: [f32; 3]) {
    for px in img.data.chunks_exact_mut(3) {
        px[0] *= gains[0];
        px[1] *= gains[1];
        px[2] *= gains[2];
    }
}
