//! Per-band Hue/Sat/Lum. Live impl: `core/src/graph/hsl.wgsl`
//! (params: `hsl.{band}.{hue,sat,lum}`).
#![allow(dead_code, unused_variables)]

use crate::pipeline::RgbImage;

#[derive(Clone, Copy)]
pub enum Band {
    Red,
    Orange,
    Yellow,
    Green,
    Aqua,
    Blue,
    Purple,
    Magenta,
}

pub fn apply(img: &mut RgbImage, band: Band, hue: f32, sat: f32, lum: f32) {
    // TODO — see hsl.wgsl
}
