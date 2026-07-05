//! 3D LUT (`.cube`) loading + GPU-upload packing.
//!
//! The parsed cube is cached in the engine (keyed by file path, like a DCP
//! profile) and threaded into the render graph, where `graph/lut.wgsl` samples
//! it with trilinear interpolation. Kept deliberately small: parse + validate +
//! flatten to the exact layout the shader indexes.

use crate::error::CoreError;
use std::path::Path;

/// Hard cap on cube edge length. 65 is the largest common authoring size;
/// bounding it keeps the GPU storage buffer a fixed, modest allocation.
pub const MAX_SIZE: usize = 65;

/// A cubic 3D LUT: `size^3` RGB entries, **R varying fastest** (`.cube` order).
#[derive(Clone, Debug)]
pub struct CubeLut {
    pub size: usize,
    /// `size^3` entries, indexed `(b * size + g) * size + r`.
    pub data: Vec<[f32; 3]>,
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
}

impl CubeLut {
    /// Parse an Adobe/IRIDAS `.cube` file. Only 3D LUTs are supported (1D LUTs
    /// are rejected — they belong in the tone-curve node).
    pub fn load_cube(path: &Path) -> Result<CubeLut, CoreError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| CoreError::InvalidOp(format!("read LUT: {e}")))?;
        Self::parse_cube(&text)
    }

    pub fn parse_cube(text: &str) -> Result<CubeLut, CoreError> {
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
                        .ok_or_else(|| CoreError::InvalidOp("bad LUT_3D_SIZE".into()))?;
                    if size < 2 || size > MAX_SIZE {
                        return Err(CoreError::InvalidOp(format!(
                            "LUT_3D_SIZE {size} out of range 2..={MAX_SIZE}"
                        )));
                    }
                }
                "LUT_1D_SIZE" => {
                    return Err(CoreError::InvalidOp(
                        "1D .cube LUTs are not supported".into(),
                    ));
                }
                "DOMAIN_MIN" => domain_min = parse3(&mut it)?,
                "DOMAIN_MAX" => domain_max = parse3(&mut it)?,
                "TITLE" => { /* ignored */ }
                _ => {
                    // A data row is three floats; anything else is ignored.
                    if let Ok(rgb) = parse3(&mut line.split_whitespace()) {
                        data.push(rgb);
                    }
                }
            }
        }

        if size == 0 {
            return Err(CoreError::InvalidOp("no LUT_3D_SIZE in .cube".into()));
        }
        if data.len() != size * size * size {
            return Err(CoreError::InvalidOp(format!(
                "cube has {} entries, expected {}",
                data.len(),
                size * size * size
            )));
        }
        Ok(CubeLut {
            size,
            data,
            domain_min,
            domain_max,
        })
    }

    /// Flatten to interleaved `r,g,b,r,g,b,…` for the storage buffer. Index in
    /// the shader as `lut[((b*size + g)*size + r) * 3 + c]`.
    pub fn flatten(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.data.len() * 3);
        for px in &self.data {
            out.extend_from_slice(px);
        }
        out
    }

    #[inline]
    fn at(&self, r: usize, g: usize, b: usize) -> [f32; 3] {
        self.data[(b * self.size + g) * self.size + r]
    }

    /// CPU reference trilinear lookup (used by tests + parity with the shader).
    pub fn apply_pixel(&self, rgb: [f32; 3]) -> [f32; 3] {
        let n = self.size - 1;
        let pos = [
            norm(rgb[0], self.domain_min[0], self.domain_max[0]) * n as f32,
            norm(rgb[1], self.domain_min[1], self.domain_max[1]) * n as f32,
            norm(rgb[2], self.domain_min[2], self.domain_max[2]) * n as f32,
        ];
        let (r0, g0, b0) = (
            pos[0].floor() as usize,
            pos[1].floor() as usize,
            pos[2].floor() as usize,
        );
        let (r1, g1, b1) = ((r0 + 1).min(n), (g0 + 1).min(n), (b0 + 1).min(n));
        let (fr, fg, fb) = (pos[0] - r0 as f32, pos[1] - g0 as f32, pos[2] - b0 as f32);
        let mut out = [0.0f32; 3];
        for c in 0..3 {
            let c00 = self.at(r0, g0, b0)[c] * (1.0 - fr) + self.at(r1, g0, b0)[c] * fr;
            let c10 = self.at(r0, g1, b0)[c] * (1.0 - fr) + self.at(r1, g1, b0)[c] * fr;
            let c01 = self.at(r0, g0, b1)[c] * (1.0 - fr) + self.at(r1, g0, b1)[c] * fr;
            let c11 = self.at(r0, g1, b1)[c] * (1.0 - fr) + self.at(r1, g1, b1)[c] * fr;
            let c0 = c00 * (1.0 - fg) + c10 * fg;
            let c1 = c01 * (1.0 - fg) + c11 * fg;
            out[c] = c0 * (1.0 - fb) + c1 * fb;
        }
        out
    }
}

#[inline]
fn norm(v: f32, lo: f32, hi: f32) -> f32 {
    ((v - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0)
}

fn parse3<'a, I: Iterator<Item = &'a str>>(it: &mut I) -> Result<[f32; 3], CoreError> {
    let mut out = [0.0f32; 3];
    for o in out.iter_mut() {
        *o = it
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| CoreError::InvalidOp("expected 3 floats".into()))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_cube(size: usize) -> String {
        let mut s = format!("LUT_3D_SIZE {size}\n");
        let n = (size - 1) as f32;
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    s.push_str(&format!(
                        "{} {} {}\n",
                        r as f32 / n,
                        g as f32 / n,
                        b as f32 / n
                    ));
                }
            }
        }
        s
    }

    #[test]
    fn parses_and_interpolates_identity() {
        let lut = CubeLut::parse_cube(&identity_cube(2)).unwrap();
        assert_eq!(lut.size, 2);
        let p = lut.apply_pixel([0.25, 0.5, 0.75]);
        for (got, want) in p.iter().zip([0.25, 0.5, 0.75]) {
            assert!((got - want).abs() < 1e-5, "{p:?}");
        }
    }

    #[test]
    fn rejects_1d_and_bad_counts() {
        assert!(CubeLut::parse_cube("LUT_1D_SIZE 4\n").is_err());
        assert!(CubeLut::parse_cube("LUT_3D_SIZE 2\n0 0 0\n").is_err());
    }

    #[test]
    fn flatten_layout_matches_index() {
        let lut = CubeLut::parse_cube(&identity_cube(3)).unwrap();
        let flat = lut.flatten();
        assert_eq!(flat.len(), 3 * 3 * 3 * 3);
        // entry (r=2,g=1,b=0) -> index (0*3+1)*3+2 = 5
        let idx = 5 * 3;
        assert_eq!(&flat[idx..idx + 3], &lut.at(2, 1, 0));
    }
}
