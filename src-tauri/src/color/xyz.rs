//! CIE XYZ helpers + reference white points. Live impl: `core/src/color.rs`.
#![allow(dead_code)]

pub const D65: [f32; 3] = [0.95047, 1.0, 1.08883];
pub const D50: [f32; 3] = [0.96422, 1.0, 0.82521];

/// 3x3 matrix * vec3.
pub fn mul3(m: [[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}
