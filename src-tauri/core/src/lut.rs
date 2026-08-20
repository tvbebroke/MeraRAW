//! 3D LUT (`.cube`) loading + tagged cinematic apply (CPU oracle).
//!
//! The GPU node (`graph/lut.wgsl`) must match `CubeLut::apply_working`.
//! Cube samples are R-fastest, matching Adobe/IRIDAS `.cube` order.

use crate::color::{mat_vec, Mat3};
use crate::error::CoreError;
use crate::idt::{self, Primaries, Transfer};
use std::path::Path;

/// Hard cap on cube edge length. 65 is the largest common authoring size.
pub const MAX_SIZE: usize = 65;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum LutKind {
    DisplayLook = 0,
    LogCreative = 1,
    TechnicalIdt = 2,
    PrintShow = 3,
}

impl LutKind {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::LogCreative,
            2 => Self::TechnicalIdt,
            3 => Self::PrintShow,
            _ => Self::DisplayLook,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum LutInterp {
    Tetrahedral = 0,
    Trilinear = 1,
}

impl LutInterp {
    pub fn from_u32(v: u32) -> Self {
        if v == 1 {
            Self::Trilinear
        } else {
            Self::Tetrahedral
        }
    }
}

/// How a cube is interpreted against the linear Rec.2020 working image.
#[derive(Clone, Copy, Debug)]
pub struct LutParams {
    pub kind: LutKind,
    pub input_primaries: Primaries,
    pub output_primaries: Primaries,
    pub shaper: Transfer,
    pub interp: LutInterp,
    /// 0..1 linear-light mix.
    pub opacity: f32,
}

impl LutParams {
    /// Unknown user `.cube`: display-referred Rec.709, sRGB shaper, tetrahedral.
    pub fn display_rec709() -> Self {
        Self {
            kind: LutKind::DisplayLook,
            input_primaries: Primaries::Rec709,
            output_primaries: Primaries::Rec709,
            shaper: Transfer::Srgb,
            interp: LutInterp::Tetrahedral,
            opacity: 1.0,
        }
    }

    /// Identity-preserving: working primaries, linear shaper.
    pub fn working_linear() -> Self {
        Self {
            kind: LutKind::DisplayLook,
            input_primaries: Primaries::Rec2020,
            output_primaries: Primaries::Rec2020,
            shaper: Transfer::Linear,
            interp: LutInterp::Tetrahedral,
            opacity: 1.0,
        }
    }
}

/// A cubic 3D LUT: `size^3` RGB entries, **R varying fastest** (`.cube` order).
#[derive(Clone, Debug)]
pub struct CubeLut {
    pub size: usize,
    /// `size^3` entries, indexed `(b * size + g) * size + r`.
    pub data: Vec<[f32; 3]>,
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
    pub title: Option<String>,
}

impl CubeLut {
    /// Parse an Adobe/IRIDAS `.cube` file. 3D cubes are the creative LUT.
    /// A 1D table is promoted to a per-channel 3D shaper (same slot — not a
    /// second engine).
    pub fn load_cube(path: &Path) -> Result<CubeLut, CoreError> {
        let s = path.to_string_lossy();
        if let Some(id) = s.strip_prefix("bundled:") {
            return crate::look::generate_look_cube(id);
        }
        if let Some(stem) = s.strip_prefix("user:") {
            let path = crate::look::user_cube_path(stem)
                .ok_or_else(|| CoreError::InvalidOp(format!("unknown user look '{stem}'")))?;
            return Self::load_cube(&path);
        }
        const MAX_LUT_BYTES: u64 = 32 * 1024 * 1024;
        let meta =
            std::fs::metadata(path).map_err(|e| CoreError::InvalidOp(format!("read LUT: {e}")))?;
        if meta.len() > MAX_LUT_BYTES {
            return Err(CoreError::InvalidOp("LUT file too large".into()));
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| CoreError::InvalidOp(format!("read LUT: {e}")))?;
        Self::parse_cube(&text)
    }

    pub fn parse_cube(text: &str) -> Result<CubeLut, CoreError> {
        let mut size = 0usize;
        let mut size_1d = 0usize;
        let mut data: Vec<[f32; 3]> = Vec::new();
        let mut domain_min = [0.0f32; 3];
        let mut domain_max = [1.0f32; 3];
        let mut title = None::<String>;

        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut it = line.split_whitespace();
            let tok = match it.next() {
                Some(t) => t,
                None => continue,
            };
            match tok {
                "LUT_3D_SIZE" => {
                    size = it
                        .next()
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| CoreError::InvalidOp("bad LUT_3D_SIZE".into()))?;
                    if !(2..=MAX_SIZE).contains(&size) {
                        return Err(CoreError::InvalidOp(format!(
                            "LUT_3D_SIZE {size} out of range 2..={MAX_SIZE}"
                        )));
                    }
                }
                "LUT_1D_SIZE" => {
                    size_1d = it
                        .next()
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| CoreError::InvalidOp("bad LUT_1D_SIZE".into()))?;
                    if !(2..=4096).contains(&size_1d) {
                        return Err(CoreError::InvalidOp(format!(
                            "LUT_1D_SIZE {size_1d} out of range 2..=4096"
                        )));
                    }
                }
                "DOMAIN_MIN" => domain_min = parse3(&mut it)?,
                "DOMAIN_MAX" => domain_max = parse3(&mut it)?,
                "TITLE" => {
                    let rest = line.trim_start_matches("TITLE").trim();
                    let t = rest.trim_matches('"').trim();
                    if !t.is_empty() {
                        title = Some(t.to_string());
                    }
                }
                _ => {
                    if let Ok(rgb) = parse3(&mut line.split_whitespace()) {
                        data.push(rgb);
                    }
                }
            }
        }

        if size == 0 {
            if size_1d >= 2 {
                if data.len() != size_1d {
                    return Err(CoreError::InvalidOp(format!(
                        "1D cube has {} entries, expected {size_1d}",
                        data.len()
                    )));
                }
                return Ok(promote_1d_shaper(data, domain_min, domain_max, title));
            }
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
            title,
        })
    }

    /// Flatten to interleaved `r,g,b,…` for the storage buffer.
    pub fn flatten(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.data.len() * 3);
        for px in &self.data {
            out.extend_from_slice(px);
        }
        out
    }

    #[inline]
    pub fn at(&self, r: usize, g: usize, b: usize) -> [f32; 3] {
        self.data[(b * self.size + g) * self.size + r]
    }

    /// Cube-domain sample (encoded RGB already mapped into DOMAIN). Tetrahedral.
    pub fn apply_pixel(&self, rgb: [f32; 3]) -> [f32; 3] {
        self.sample_tetrahedral(rgb)
    }

    pub fn sample_trilinear(&self, rgb: [f32; 3]) -> [f32; 3] {
        let (i0, i1, f) = self.cell(rgb);
        let mut out = [0.0f32; 3];
        for c in 0..3 {
            let c00 = self.at(i0[0], i0[1], i0[2])[c] * (1.0 - f[0])
                + self.at(i1[0], i0[1], i0[2])[c] * f[0];
            let c10 = self.at(i0[0], i1[1], i0[2])[c] * (1.0 - f[0])
                + self.at(i1[0], i1[1], i0[2])[c] * f[0];
            let c01 = self.at(i0[0], i0[1], i1[2])[c] * (1.0 - f[0])
                + self.at(i1[0], i0[1], i1[2])[c] * f[0];
            let c11 = self.at(i0[0], i1[1], i1[2])[c] * (1.0 - f[0])
                + self.at(i1[0], i1[1], i1[2])[c] * f[0];
            let c0 = c00 * (1.0 - f[1]) + c10 * f[1];
            let c1 = c01 * (1.0 - f[1]) + c11 * f[1];
            out[c] = c0 * (1.0 - f[2]) + c1 * f[2];
        }
        out
    }

    /// Standard 6-tetrahedra interpolation (same partition as OCIO / Resolve).
    pub fn sample_tetrahedral(&self, rgb: [f32; 3]) -> [f32; 3] {
        let (i0, i1, f) = self.cell(rgb);
        let (fx, fy, fz) = (f[0], f[1], f[2]);
        let c000 = self.at(i0[0], i0[1], i0[2]);
        let c100 = self.at(i1[0], i0[1], i0[2]);
        let c010 = self.at(i0[0], i1[1], i0[2]);
        let c001 = self.at(i0[0], i0[1], i1[2]);
        let c110 = self.at(i1[0], i1[1], i0[2]);
        let c101 = self.at(i1[0], i0[1], i1[2]);
        let c011 = self.at(i0[0], i1[1], i1[2]);
        let c111 = self.at(i1[0], i1[1], i1[2]);
        let mut out = [0.0f32; 3];
        for c in 0..3 {
            out[c] = if fx >= fy {
                if fy >= fz {
                    (1.0 - fx) * c000[c] + (fx - fy) * c100[c] + (fy - fz) * c110[c] + fz * c111[c]
                } else if fx >= fz {
                    (1.0 - fx) * c000[c] + (fx - fz) * c100[c] + (fz - fy) * c101[c] + fy * c111[c]
                } else {
                    (1.0 - fz) * c000[c] + (fz - fx) * c001[c] + (fx - fy) * c101[c] + fy * c111[c]
                }
            } else if fz >= fy {
                (1.0 - fz) * c000[c] + (fz - fy) * c001[c] + (fy - fx) * c011[c] + fx * c111[c]
            } else if fz >= fx {
                (1.0 - fy) * c000[c] + (fy - fz) * c010[c] + (fz - fx) * c011[c] + fx * c111[c]
            } else {
                (1.0 - fy) * c000[c] + (fy - fx) * c010[c] + (fx - fz) * c110[c] + fz * c111[c]
            };
        }
        out
    }

    fn cell(&self, rgb: [f32; 3]) -> ([usize; 3], [usize; 3], [f32; 3]) {
        let n = (self.size - 1) as f32;
        let mut i0 = [0usize; 3];
        let mut i1 = [0usize; 3];
        let mut f = [0.0f32; 3];
        for c in 0..3 {
            let t = norm(rgb[c], self.domain_min[c], self.domain_max[c]) * n;
            let lo = t.floor().clamp(0.0, n) as usize;
            let hi = (lo + 1).min(self.size - 1);
            i0[c] = lo;
            i1[c] = hi;
            f[c] = t - lo as f32;
        }
        (i0, i1, f)
    }

    /// Authoritative working-space apply: linear Rec.2020 in, linear Rec.2020 out.
    pub fn apply_working(&self, rgb_2020: [f32; 3], p: &LutParams) -> [f32; 3] {
        let amount = p.opacity.clamp(0.0, 1.0);
        if amount <= 0.0 {
            return rgb_2020;
        }
        let m_in: Mat3 = idt::rec2020_to(p.input_primaries);
        let m_out: Mat3 = idt::to_rec2020(p.output_primaries);
        let lin_in = mat_vec(&m_in, rgb_2020);

        let display_shaper = matches!(
            p.shaper,
            Transfer::Linear | Transfer::Srgb | Transfer::Rec709
        );
        let (encoded, residual) = if display_shaper {
            let sdr = [
                lin_in[0].clamp(0.0, 1.0),
                lin_in[1].clamp(0.0, 1.0),
                lin_in[2].clamp(0.0, 1.0),
            ];
            let hi = [lin_in[0] - sdr[0], lin_in[1] - sdr[1], lin_in[2] - sdr[2]];
            (idt::encode(p.shaper, sdr), hi)
        } else {
            (
                idt::encode(
                    p.shaper,
                    [lin_in[0].max(0.0), lin_in[1].max(0.0), lin_in[2].max(0.0)],
                ),
                [0.0; 3],
            )
        };

        let looked = match p.interp {
            LutInterp::Tetrahedral => self.sample_tetrahedral(encoded),
            LutInterp::Trilinear => self.sample_trilinear(encoded),
        };
        let mut decoded = idt::decode(p.shaper, looked);
        decoded[0] += residual[0];
        decoded[1] += residual[1];
        decoded[2] += residual[2];
        let lin_work = mat_vec(&m_out, decoded);
        [
            rgb_2020[0] * (1.0 - amount) + lin_work[0] * amount,
            rgb_2020[1] * (1.0 - amount) + lin_work[1] * amount,
            rgb_2020[2] * (1.0 - amount) + lin_work[2] * amount,
        ]
    }

    /// Matrices uploaded to the LUT shader (row-major, padded to vec4).
    pub fn shader_mats(p: &LutParams) -> (Mat3, Mat3) {
        (
            idt::rec2020_to(p.input_primaries),
            idt::to_rec2020(p.output_primaries),
        )
    }
}

/// PrintShow LUTs already include a display transform — skip AgX/Neutral.
pub fn present_look_for(
    kind: crate::raw::ImageKind,
    user_look: u32,
    doc: &crate::doc::EditDoc,
) -> u32 {
    if user_look == 4 {
        return crate::raw::effective_display_look(kind, user_look);
    }
    let opacity = crate::registry::effective_f32(doc, "lut", "opacity");
    let p = params_from_doc(doc, opacity);
    if p.kind == LutKind::PrintShow && p.opacity > 0.0 {
        3
    } else {
        crate::raw::effective_display_look(kind, user_look)
    }
}

#[inline]
fn norm(v: f32, lo: f32, hi: f32) -> f32 {
    ((v - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0)
}

/// Promote a 1D `.cube` (per-channel shaper) into a 3D cube so it occupies the
/// single LUT slot. Independent R/G/B mapping — not a second creative LUT.
fn promote_1d_shaper(
    table: Vec<[f32; 3]>,
    domain_min: [f32; 3],
    domain_max: [f32; 3],
    title: Option<String>,
) -> CubeLut {
    const SIZE: usize = 17;
    let n = (SIZE - 1) as f32;
    let mut data = Vec::with_capacity(SIZE * SIZE * SIZE);
    for b in 0..SIZE {
        for g in 0..SIZE {
            for r in 0..SIZE {
                let rr = sample_1d(&table, r as f32 / n);
                let gg = sample_1d(&table, g as f32 / n);
                let bb = sample_1d(&table, b as f32 / n);
                data.push([rr[0], gg[1], bb[2]]);
            }
        }
    }
    CubeLut {
        size: SIZE,
        data,
        domain_min,
        domain_max,
        title,
    }
}

fn sample_1d(table: &[[f32; 3]], t: f32) -> [f32; 3] {
    let n = table.len();
    if n == 0 {
        return [t, t, t];
    }
    if n == 1 {
        return table[0];
    }
    let x = t.clamp(0.0, 1.0) * (n - 1) as f32;
    let i0 = x.floor() as usize;
    let i1 = (i0 + 1).min(n - 1);
    let f = x - i0 as f32;
    let a = table[i0];
    let b = table[i1];
    [
        a[0] + (b[0] - a[0]) * f,
        a[1] + (b[1] - a[1]) * f,
        a[2] + (b[2] - a[2]) * f,
    ]
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

pub fn identity_cube_text(size: usize) -> String {
    let mut s = format!("TITLE \"identity\"\nLUT_3D_SIZE {size}\n");
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

/// Params from an EditDoc (registry f32 enums).
pub fn params_from_doc(doc: &crate::doc::EditDoc, opacity_100: f32) -> LutParams {
    use crate::registry::effective_f32 as eff;
    let enabled = eff(doc, "lut", "enabled");
    let opacity = if enabled < 0.5 {
        0.0
    } else {
        (opacity_100 / 100.0).clamp(0.0, 1.0)
    };
    LutParams {
        kind: LutKind::from_u32(eff(doc, "lut", "kind").round() as u32),
        input_primaries: Primaries::from_u32(eff(doc, "lut", "input_primaries").round() as u32),
        output_primaries: Primaries::from_u32(eff(doc, "lut", "output_primaries").round() as u32),
        shaper: Transfer::from_u32(eff(doc, "lut", "shaper").round() as u32),
        interp: LutInterp::from_u32(eff(doc, "lut", "interpolation").round() as u32),
        opacity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(size: usize) -> CubeLut {
        CubeLut::parse_cube(&identity_cube_text(size)).unwrap()
    }

    #[test]
    fn parses_and_interpolates_identity_trilinear() {
        let lut = identity(2);
        let p = lut.sample_trilinear([0.25, 0.5, 0.75]);
        for (got, want) in p.iter().zip([0.25, 0.5, 0.75]) {
            assert!((got - want).abs() < 1e-5, "{p:?}");
        }
    }

    #[test]
    fn tetrahedral_identity_2_and_33() {
        for size in [2usize, 17, 33] {
            let lut = identity(size);
            for &rgb in &[
                [0.0, 0.0, 0.0],
                [1.0, 1.0, 1.0],
                [0.5, 0.5, 0.5],
                [0.2, 0.4, 0.8],
                [0.9, 0.1, 0.3],
            ] {
                let t = lut.sample_tetrahedral(rgb);
                for c in 0..3 {
                    assert!((t[c] - rgb[c]).abs() < 2e-5, "size={size} {rgb:?} → {t:?}");
                }
            }
        }
    }

    #[test]
    fn tetrahedral_matches_trilinear_on_identity() {
        let lut = identity(9);
        for i in 0..20 {
            let rgb = [i as f32 / 19.0, 0.3, 0.7];
            let a = lut.sample_tetrahedral(rgb);
            let b = lut.sample_trilinear(rgb);
            for c in 0..3 {
                assert!((a[c] - b[c]).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn rejects_1d_and_bad_counts() {
        assert!(CubeLut::parse_cube("LUT_1D_SIZE 4\n").is_err());
        assert!(CubeLut::parse_cube("LUT_3D_SIZE 2\n0 0 0\n").is_err());
    }

    #[test]
    fn one_d_identity_promotes_to_working_passthrough() {
        let lut = CubeLut::parse_cube("TITLE \"shaper\"\nLUT_1D_SIZE 2\n0 0 0\n1 1 1\n").unwrap();
        assert_eq!(lut.size, 17);
        let p = LutParams::working_linear();
        for &rgb in &[[0.0, 0.0, 0.0], [0.18, 0.18, 0.18], [1.0, 0.5, 0.2]] {
            let out = lut.apply_working(rgb, &p);
            for c in 0..3 {
                assert!((out[c] - rgb[c]).abs() < 2e-3, "{rgb:?} → {out:?}");
            }
        }
    }

    #[test]
    fn identity_cube_text_is_working_passthrough() {
        let lut = CubeLut::parse_cube(&identity_cube_text(9)).unwrap();
        let p = LutParams::working_linear();
        for &rgb in &[
            [0.0, 0.0, 0.0],
            [0.18, 0.18, 0.18],
            [1.0, 1.0, 1.0],
            [0.4, 0.2, 0.1],
        ] {
            let out = lut.apply_working(rgb, &p);
            for c in 0..3 {
                assert!((out[c] - rgb[c]).abs() < 2e-4, "{rgb:?} → {out:?}");
            }
        }
    }

    #[test]
    fn flatten_layout_matches_index() {
        let lut = identity(3);
        let flat = lut.flatten();
        assert_eq!(flat.len(), 3 * 3 * 3 * 3);
        let idx = 5 * 3;
        assert_eq!(&flat[idx..idx + 3], &lut.at(2, 1, 0));
    }

    #[test]
    fn working_identity_cube_is_identity_on_sdr_and_highlights() {
        let lut = identity(5);
        let p = LutParams::working_linear();
        for &rgb in &[
            [0.0, 0.0, 0.0],
            [0.18, 0.18, 0.18],
            [1.0, 1.0, 1.0],
            [1.25, 1.25, 1.25],
            [2.0, 1.5, 1.1],
            [0.4, 0.2, 0.1],
        ] {
            let out = lut.apply_working(rgb, &p);
            for c in 0..3 {
                assert!((out[c] - rgb[c]).abs() < 2e-4, "{rgb:?} → {out:?}");
            }
        }
    }

    #[test]
    fn display_rec709_identity_preserves_neutrals_and_highlights() {
        let lut = identity(9);
        let p = LutParams::display_rec709();
        for &g in &[0.0, 0.18, 0.5, 1.0, 1.5, 2.5] {
            let rgb = [g, g, g];
            let out = lut.apply_working(rgb, &p);
            for c in 0..3 {
                assert!((out[c] - g).abs() < 3e-4, "grey {g} → {out:?}");
            }
        }
    }

    #[test]
    fn opacity_zero_is_identity() {
        let lut = identity(3);
        let mut p = LutParams::display_rec709();
        p.opacity = 0.0;
        let rgb = [0.3, 0.4, 0.5];
        let out = lut.apply_working(rgb, &p);
        assert_eq!(out, rgb);
    }

    #[test]
    fn opacity_mix_is_linear_not_encoded() {
        // A cube that maps everything to black in the encoded domain.
        let mut s = String::from("LUT_3D_SIZE 2\n");
        for _ in 0..8 {
            s.push_str("0 0 0\n");
        }
        let lut = CubeLut::parse_cube(&s).unwrap();
        let mut p = LutParams::working_linear();
        p.opacity = 0.5;
        let rgb = [0.25, 0.25, 0.25];
        let out = lut.apply_working(rgb, &p);
        // Linear mix with black: 0.125. Encoded mix of sRGB(0.25) would differ.
        for c in 0..3 {
            assert!((out[c] - 0.125).abs() < 2e-3, "{out:?}");
        }
    }

    #[test]
    fn highlights_stay_ordered_through_display_lut() {
        let lut = identity(5);
        let p = LutParams::display_rec709();
        let a = lut.apply_working([1.0, 1.0, 1.0], &p)[0];
        let b = lut.apply_working([1.25, 1.25, 1.25], &p)[0];
        let c = lut.apply_working([2.0, 2.0, 2.0], &p)[0];
        let d = lut.apply_working([4.0, 4.0, 4.0], &p)[0];
        assert!(a < b && b < c && c < d, "{a} {b} {c} {d}");
        assert!((a - 1.0).abs() < 3e-4);
    }

    #[test]
    fn present_look_print_show_forces_passthrough() {
        let mut doc = crate::doc::EditDoc::new("/t.ARW");
        doc.set("lut", "kind", crate::doc::ParamValue::F32(3.0));
        doc.set("lut", "enabled", crate::doc::ParamValue::F32(1.0));
        doc.set("lut", "opacity", crate::doc::ParamValue::F32(100.0));
        let look = present_look_for(crate::raw::ImageKind::Raw, 2, &doc);
        assert_eq!(look, 3);
        doc.set("lut", "opacity", crate::doc::ParamValue::F32(0.0));
        let look = present_look_for(crate::raw::ImageKind::Raw, 2, &doc);
        assert_eq!(look, 2);
    }

    #[test]
    fn known_channel_swap_moves_pixels() {
        let size = 5usize;
        let n = (size - 1) as f32;
        let mut s = format!("LUT_3D_SIZE {size}\n");
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    s.push_str(&format!(
                        "{} {} {}\n",
                        b as f32 / n,
                        g as f32 / n,
                        r as f32 / n
                    ));
                }
            }
        }
        let lut = CubeLut::parse_cube(&s).unwrap();
        let p = LutParams::working_linear();
        let out = lut.apply_working([0.2, 0.4, 0.8], &p);
        assert!((out[0] - 0.8).abs() < 0.02);
        assert!((out[2] - 0.2).abs() < 0.02);
    }
}
