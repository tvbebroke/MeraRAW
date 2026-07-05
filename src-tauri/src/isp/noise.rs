//! Luma/chroma denoise. Live equivalent: `core/src/graph/noise.wgsl`
//! (params: `detail.noise_luma`, `detail.noise_chroma`).
#![allow(dead_code, unused_variables)]

use crate::pipeline::RgbImage;

pub fn reduce(img: &mut RgbImage, luma: f32, chroma: f32) {
    // TODO — see noise.wgsl
}
