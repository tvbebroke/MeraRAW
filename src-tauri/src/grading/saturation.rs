//! Global saturation about Rec.2020 luma. Live equivalent:
//! `color_grade.global_chroma` in grade.wgsl.
#![allow(dead_code, unused_variables)]

use crate::pipeline::RgbImage;

const LUMA: [f32; 3] = [0.2627, 0.6780, 0.0593]; // Rec.2020

pub fn apply(img: &mut RgbImage, amount: f32) {
    let k = 1.0 + amount;
    for px in img.data.chunks_exact_mut(3) {
        let l = LUMA[0] * px[0] + LUMA[1] * px[1] + LUMA[2] * px[2];
        px[0] = l + (px[0] - l) * k;
        px[1] = l + (px[1] - l) * k;
        px[2] = l + (px[2] - l) * k;
    }
}
