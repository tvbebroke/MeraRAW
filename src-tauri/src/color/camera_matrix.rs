//! Camera-native RGB -> CIE XYZ via the DNG color matrix.
//! Live impl: `core/src/graph/color_matrix.wgsl` + `core/src/profile/dcp.rs`.
#![allow(dead_code, unused_variables)]

use crate::pipeline::RgbImage;

pub fn camera_to_xyz(img: &mut RgbImage, m: [[f32; 3]; 3]) {
    for px in img.data.chunks_exact_mut(3) {
        let (r, g, b) = (px[0], px[1], px[2]);
        px[0] = m[0][0] * r + m[0][1] * g + m[0][2] * b;
        px[1] = m[1][0] * r + m[1][1] * g + m[1][2] * b;
        px[2] = m[2][0] * r + m[2][1] * g + m[2][2] * b;
    }
}
