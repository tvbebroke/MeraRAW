//! Transfer functions (gamma / sRGB OETF-EOTF).
//! Live tone handling: `core/src/curve.rs` + `core/src/graph/curve.wgsl`.
#![allow(dead_code)]

pub fn srgb_oetf(c: f32) -> f32 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

pub fn srgb_eotf(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn gamma(c: f32, g: f32) -> f32 {
    c.max(0.0).powf(1.0 / g)
}
