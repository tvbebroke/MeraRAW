//! 3D LUT loading (`.cube`) + trilinear application.
//!
//! **This is the one stage with no counterpart in the shipping pipeline yet.**
//! It is implemented for real here (not stubbed) so it can be lifted straight
//! into a `core/src/graph/lut.wgsl` node + a `lut.*` registry entry.
#![allow(dead_code, unused_variables)]

use crate::pipeline::RgbImage;
use std::path::Path;

/// A cubic 3D LUT: `size^3` RGB entries, R varying fastest (`.cube` order).
pub struct CubeLut {
    pub size: usize,
    pub data: Vec<[f32; 3]>,
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
}

impl CubeLut {
    /// Parse an Adobe/IRIDAS `.cube` file (3D LUT required for `apply`).
    pub fn load_cube(path: &Path) -> Result<CubeLut, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut size = 0usize;
        let mut data: Vec<[f32; 3]> = Vec::new();
        let mut domain_min = [0.0f32; 3];
        let mut domain_max = [1.0f32; 3];

        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut it = line.split_whitespace();
            let tok = it.next().unwrap();
            match tok {
                "LUT_3D_SIZE" => {
                    size = it
                        .next()
                        .and_then(|s| s.parse().ok())
                        .ok_or("bad LUT_3D_SIZE")?;
                }
                "DOMAIN_MIN" => domain_min = parse3(&mut it)?,
                "DOMAIN_MAX" => domain_max = parse3(&mut it)?,
                "TITLE" | "LUT_1D_SIZE" => { /* ignored */ }
                _ => {
                    if let Ok(rgb) = parse3(&mut line.split_whitespace()) {
                        data.push(rgb);
                    }
                }
            }
        }

        if size == 0 || data.len() != size * size * size {
            return Err(format!(
                "not a 3D cube (size={size}, entries={})",
                data.len()
            ));
        }
        Ok(CubeLut { size, data, domain_min, domain_max })
    }

    #[inline]
    fn sample(&self, r: usize, g: usize, b: usize) -> [f32; 3] {
        self.data[(b * self.size + g) * self.size + r]
    }

    /// Trilinear lookup of a normalized RGB triple.
    pub fn apply_pixel(&self, rgb: [f32; 3]) -> [f32; 3] {
        let n = self.size - 1;
        let pos = [
            norm(rgb[0], self.domain_min[0], self.domain_max[0]) * n as f32,
            norm(rgb[1], self.domain_min[1], self.domain_max[1]) * n as f32,
            norm(rgb[2], self.domain_min[2], self.domain_max[2]) * n as f32,
        ];
        let (r0, g0, b0) = (pos[0].floor() as usize, pos[1].floor() as usize, pos[2].floor() as usize);
        let (r1, g1, b1) = ((r0 + 1).min(n), (g0 + 1).min(n), (b0 + 1).min(n));
        let (fr, fg, fb) = (pos[0] - r0 as f32, pos[1] - g0 as f32, pos[2] - b0 as f32);

        let mut out = [0.0f32; 3];
        for c in 0..3 {
            let c000 = self.sample(r0, g0, b0)[c];
            let c100 = self.sample(r1, g0, b0)[c];
            let c010 = self.sample(r0, g1, b0)[c];
            let c110 = self.sample(r1, g1, b0)[c];
            let c001 = self.sample(r0, g0, b1)[c];
            let c101 = self.sample(r1, g0, b1)[c];
            let c011 = self.sample(r0, g1, b1)[c];
            let c111 = self.sample(r1, g1, b1)[c];
            let c00 = c000 * (1.0 - fr) + c100 * fr;
            let c10 = c010 * (1.0 - fr) + c110 * fr;
            let c01 = c001 * (1.0 - fr) + c101 * fr;
            let c11 = c011 * (1.0 - fr) + c111 * fr;
            let c0 = c00 * (1.0 - fg) + c10 * fg;
            let c1 = c01 * (1.0 - fg) + c11 * fg;
            out[c] = c0 * (1.0 - fb) + c1 * fb;
        }
        out
    }

    /// Apply to a whole image at a given opacity (0..1).
    pub fn apply(&self, img: &mut RgbImage, opacity: f32) {
        let o = opacity.clamp(0.0, 1.0);
        for px in img.data.chunks_exact_mut(3) {
            let src = [px[0], px[1], px[2]];
            let dst = self.apply_pixel(src);
            px[0] = src[0] * (1.0 - o) + dst[0] * o;
            px[1] = src[1] * (1.0 - o) + dst[1] * o;
            px[2] = src[2] * (1.0 - o) + dst[2] * o;
        }
    }
}

#[inline]
fn norm(v: f32, lo: f32, hi: f32) -> f32 {
    ((v - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0)
}

fn parse3<'a, I: Iterator<Item = &'a str>>(it: &mut I) -> Result<[f32; 3], String> {
    let mut out = [0.0f32; 3];
    for o in out.iter_mut() {
        *o = it
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or("expected 3 floats")?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_lut_roundtrips() {
        // 2^3 identity cube: corners of the unit RGB cube.
        let mut data = Vec::new();
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    data.push([r as f32, g as f32, b as f32]);
                }
            }
        }
        let lut = CubeLut { size: 2, data, domain_min: [0.0; 3], domain_max: [1.0; 3] };
        let p = lut.apply_pixel([0.25, 0.5, 0.75]);
        assert!((p[0] - 0.25).abs() < 1e-5);
        assert!((p[1] - 0.5).abs() < 1e-5);
        assert!((p[2] - 0.75).abs() < 1e-5);
    }
}
