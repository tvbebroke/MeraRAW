//! Contrast around a mid-gray pivot. Live equivalent folds into the core tone curve.
#![allow(dead_code, unused_variables)]

use crate::pipeline::RgbImage;

pub fn apply(img: &mut RgbImage, amount: f32, pivot: f32) {
    let k = 1.0 + amount;
    for v in img.data.iter_mut() {
        *v = (*v - pivot) * k + pivot;
    }
}
