//! Exposure in stops (linear multiply by 2^stops). Param: `exposure.stops`.
#![allow(dead_code, unused_variables)]

use crate::pipeline::RgbImage;

pub fn apply(img: &mut RgbImage, stops: f32) {
    let k = 2f32.powf(stops);
    for v in img.data.iter_mut() {
        *v *= k;
    }
}
