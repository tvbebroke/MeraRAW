//! Working/output color-space matrices (sRGB, Rec.2020, ProPhoto, Display P3).
//! Live impl: `core/src/color.rs` (Rec.2020 working space; Oklab for grading).
#![allow(dead_code, unused_variables)]

use crate::color::xyz::mul3;
use crate::pipeline::RgbImage;

#[derive(Clone, Copy)]
pub enum WorkingSpace {
    Srgb,
    Rec2020,
    ProPhoto,
    DisplayP3,
}

/// XYZ(D65) -> Rec.2020 linear.
pub const XYZ_TO_REC2020: [[f32; 3]; 3] = [
    [1.71665, -0.35567, -0.25337],
    [-0.66668, 1.61648, 0.01577],
    [0.01764, -0.04277, 0.94210],
];

/// XYZ(D65) -> sRGB linear.
pub const XYZ_TO_SRGB: [[f32; 3]; 3] = [
    [3.24097, -1.53738, -0.49861],
    [-0.96924, 1.87597, 0.04156],
    [0.05563, -0.20398, 1.05697],
];

pub fn xyz_to_working(img: &mut RgbImage, space: WorkingSpace) {
    let m = match space {
        WorkingSpace::Rec2020 => XYZ_TO_REC2020,
        // TODO ProPhoto / Display P3 — see core/src/color.rs.
        _ => XYZ_TO_SRGB,
    };
    for px in img.data.chunks_exact_mut(3) {
        let o = mul3(m, [px[0], px[1], px[2]]);
        px.copy_from_slice(&o);
    }
}

pub fn working_to_output(img: &mut RgbImage, space: WorkingSpace) {
    // Placeholder — real gamut conversions live in core/src/color.rs.
}
