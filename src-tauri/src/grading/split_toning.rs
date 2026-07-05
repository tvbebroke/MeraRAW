//! Two-color split toning (shadow tint + highlight tint + balance).
//! Subsumed by the 3-way wheels in the shipping build; kept as a discrete control.
#![allow(dead_code, unused_variables)]

use crate::pipeline::RgbImage;

pub struct SplitTone {
    pub shadow_hue: f32,
    pub shadow_sat: f32,
    pub highlight_hue: f32,
    pub highlight_sat: f32,
    pub balance: f32,
}

pub fn apply(img: &mut RgbImage, params: &SplitTone) {
    // TODO
}
