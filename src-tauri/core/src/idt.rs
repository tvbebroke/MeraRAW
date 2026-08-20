//! Camera / display transfer functions and input-device transforms.
//!
//! Published curves only. Primaries conversion uses chromaticity matrices
//! from `color.rs`. Used by the LUT shaper path and by video landing.
//! Working space after an IDT: linear Rec.2020, D65.

use crate::color::{self, mat_mul, mat_vec, Mat3, XYZ_TO_REC2020};

/// Integer ids match `lut.shaper` / `input.transfer` registry enums.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Transfer {
    /// Scene-linear (identity).
    Linear = 0,
    Srgb = 1,
    Rec709 = 2,
    Cineon = 3,
    Acescct = 4,
    LogC3 = 5,
    LogC4 = 6,
    SLog3 = 7,
    VLog = 8,
    Pq = 9,
    Hlg = 10,
}

impl Transfer {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Srgb,
            2 => Self::Rec709,
            3 => Self::Cineon,
            4 => Self::Acescct,
            5 => Self::LogC3,
            6 => Self::LogC4,
            7 => Self::SLog3,
            8 => Self::VLog,
            9 => Self::Pq,
            10 => Self::Hlg,
            _ => Self::Linear,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Srgb => "sRGB",
            Self::Rec709 => "Rec.709",
            Self::Cineon => "Cineon",
            Self::Acescct => "ACEScct",
            Self::LogC3 => "LogC3",
            Self::LogC4 => "LogC4",
            Self::SLog3 => "S-Log3",
            Self::VLog => "V-Log",
            Self::Pq => "PQ",
            Self::Hlg => "HLG",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Primaries {
    Rec2020 = 0,
    Rec709 = 1,
    DisplayP3 = 2,
    SGamut3Cine = 3,
    ArriWideGamut3 = 4,
    ArriWideGamut4 = 5,
    VGamut = 6,
}

impl Primaries {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Rec709,
            2 => Self::DisplayP3,
            3 => Self::SGamut3Cine,
            4 => Self::ArriWideGamut3,
            5 => Self::ArriWideGamut4,
            6 => Self::VGamut,
            _ => Self::Rec2020,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Rec2020 => "Rec.2020",
            Self::Rec709 => "Rec.709",
            Self::DisplayP3 => "Display P3",
            Self::SGamut3Cine => "S-Gamut3.Cine",
            Self::ArriWideGamut3 => "ARRI Wide Gamut 3",
            Self::ArriWideGamut4 => "ARRI Wide Gamut 4",
            Self::VGamut => "V-Gamut",
        }
    }

    /// RGB (this space, linear) → XYZ D65, computed from chromaticities.
    pub fn to_xyz(self) -> Mat3 {
        match self {
            Self::Rec2020 => color::REC2020_TO_XYZ,
            Self::Rec709 => color::SRGB_TO_XYZ,
            Self::DisplayP3 => color::DISPLAY_P3_TO_XYZ,
            Self::SGamut3Cine => color::rgb_to_xyz_from_xy(
                (0.766, 0.275),
                (0.225, 0.800),
                (0.089, -0.087),
                (0.3127, 0.3290),
            ),
            Self::ArriWideGamut3 => color::rgb_to_xyz_from_xy(
                (0.6840, 0.3130),
                (0.2210, 0.8480),
                (0.0861, -0.1020),
                (0.3127, 0.3290),
            ),
            Self::ArriWideGamut4 => color::rgb_to_xyz_from_xy(
                (0.7347, 0.2653),
                (0.1424, 0.8576),
                (0.0991, -0.0308),
                (0.3127, 0.3290),
            ),
            Self::VGamut => color::rgb_to_xyz_from_xy(
                (0.730, 0.280),
                (0.165, 0.840),
                (0.100, -0.030),
                (0.3127, 0.3290),
            ),
        }
    }

    pub fn from_xyz(self) -> Mat3 {
        color::mat_inverse(&self.to_xyz()).expect("primaries matrix invertible")
    }
}

/// Linear Rec.2020 → linear `dst` primaries.
pub fn rec2020_to(dst: Primaries) -> Mat3 {
    mat_mul(&dst.from_xyz(), &color::REC2020_TO_XYZ)
}

/// Linear `src` primaries → linear Rec.2020.
pub fn to_rec2020(src: Primaries) -> Mat3 {
    mat_mul(&XYZ_TO_REC2020, &src.to_xyz())
}

fn map3(rgb: [f32; 3], f: impl Fn(f32) -> f32) -> [f32; 3] {
    [f(rgb[0]), f(rgb[1]), f(rgb[2])]
}

/// Encode linear light → the named transfer (cube / log domain).
pub fn encode(t: Transfer, rgb: [f32; 3]) -> [f32; 3] {
    match t {
        Transfer::Linear => rgb,
        Transfer::Srgb => map3(rgb, srgb_oetf),
        Transfer::Rec709 => map3(rgb, rec709_oetf),
        Transfer::Cineon => map3(rgb, cineon_encode),
        Transfer::Acescct => map3(rgb, acescct_encode),
        Transfer::LogC3 => map3(rgb, logc3_encode),
        Transfer::LogC4 => map3(rgb, logc4_encode),
        Transfer::SLog3 => map3(rgb, slog3_encode),
        Transfer::VLog => map3(rgb, vlog_encode),
        Transfer::Pq => map3(rgb, pq_encode_scene),
        Transfer::Hlg => map3(rgb, hlg_oetf),
    }
}

/// Decode named transfer → linear light.
pub fn decode(t: Transfer, rgb: [f32; 3]) -> [f32; 3] {
    match t {
        Transfer::Linear => rgb,
        Transfer::Srgb => map3(rgb, srgb_eotf),
        Transfer::Rec709 => map3(rgb, rec709_eotf),
        Transfer::Cineon => map3(rgb, cineon_decode),
        Transfer::Acescct => map3(rgb, acescct_decode),
        Transfer::LogC3 => map3(rgb, logc3_decode),
        Transfer::LogC4 => map3(rgb, logc4_decode),
        Transfer::SLog3 => map3(rgb, slog3_decode),
        Transfer::VLog => map3(rgb, vlog_decode),
        Transfer::Pq => map3(rgb, pq_decode_scene),
        Transfer::Hlg => map3(rgb, hlg_eotf),
    }
}

/// Soft highlight compressor used by the sRGB/Rec.709 LUT path.
/// Identity on `[0, 1]`, C0-continuous, maps `(1, ∞)` → `(1, 2)` so ordering
/// is preserved before the cube-domain clamp. Combined with residual bypass
/// in `lut::apply_working` this does not collapse super-whites.
pub fn highlight_extend(x: f32) -> f32 {
    if x <= 1.0 {
        x
    } else {
        // f(1)=1, f'(1)=1 (left), f(∞)→2
        2.0 - 1.0 / x
    }
}

pub fn highlight_extend3(rgb: [f32; 3]) -> [f32; 3] {
    map3(rgb, highlight_extend)
}

// ---- sRGB (IEC 61966-2-1) — encode may exceed 1 (extended) ----

pub fn srgb_oetf(c: f32) -> f32 {
    if c <= 0.0 {
        return 0.0;
    }
    if c <= 0.0031308 {
        return 12.92 * c;
    }
    if c <= 1.0 {
        return 1.055 * c.powf(1.0 / 2.4) - 0.055;
    }
    // Linear extrapolation of the curve at 1 so super-whites stay ordered.
    let d = 1.055 * (1.0 / 2.4); // ≈ 0.43958
    1.0 + d * (c - 1.0)
}

pub fn srgb_eotf(c: f32) -> f32 {
    if c <= 0.0 {
        return 0.0;
    }
    if c <= 0.04045 {
        return c / 12.92;
    }
    if c <= 1.0 {
        return ((c + 0.055) / 1.055).powf(2.4);
    }
    let d: f32 = 1.055 * (1.0 / 2.4);
    1.0 + (c - 1.0) / d.max(1e-6)
}

// ---- Rec.709 OETF (ITU-R BT.709) ----

pub fn rec709_oetf(c: f32) -> f32 {
    if c < 0.0 {
        return 0.0;
    }
    if c < 0.018 {
        4.5 * c
    } else {
        1.099 * c.powf(0.45) - 0.099
    }
}

pub fn rec709_eotf(c: f32) -> f32 {
    if c < 0.0 {
        return 0.0;
    }
    if c < 0.081 {
        c / 4.5
    } else {
        ((c + 0.099) / 1.099).powf(1.0 / 0.45)
    }
}

// ---- Cineon printing density (Kodak / OCIO default) ----
// lin = (10^((1023x − 685)/300) − 0.0108) / (1 − 0.0108)

pub fn cineon_decode(x: f32) -> f32 {
    let black = 0.0108;
    ((10.0f32).powf((1023.0 * x - 685.0) / 300.0) - black) / (1.0 - black)
}

pub fn cineon_encode(lin: f32) -> f32 {
    let black = 0.0108;
    let y = lin.max(0.0) * (1.0 - black) + black;
    (300.0 * y.log10() + 685.0) / 1023.0
}

// ---- ACEScct (Academy S-2016-001) ----

pub fn acescct_encode(lin: f32) -> f32 {
    let x = lin.max(0.0);
    if x <= 0.0078125 {
        10.5402377416545 * x + 0.0729055341958355
    } else {
        (x.log2() + 9.72) / 17.52
    }
}

pub fn acescct_decode(x: f32) -> f32 {
    if x <= 0.155251141552511 {
        (x - 0.0729055341958355) / 10.5402377416545
    } else {
        2.0f32.powf(x * 17.52 - 9.72)
    }
}

// ---- ARRI LogC3 (EI 800, 32-bit float parameters from ARRI) ----

pub fn logc3_encode(lin: f32) -> f32 {
    const CUT: f32 = 0.010_591;
    const A: f32 = 5.555_556;
    const B: f32 = 0.052_272;
    const C: f32 = 0.247_190;
    const D: f32 = 0.385_537;
    const E: f32 = 5.367_655;
    const F: f32 = 0.092_809;
    if lin > CUT {
        C * (A * lin + B).log10() + D
    } else {
        E * lin + F
    }
}

pub fn logc3_decode(x: f32) -> f32 {
    const CUT: f32 = 0.010_591;
    const A: f32 = 5.555_556;
    const B: f32 = 0.052_272;
    const C: f32 = 0.247_190;
    const D: f32 = 0.385_537;
    const E: f32 = 5.367_655;
    const F: f32 = 0.092_809;
    let cut_enc = logc3_encode(CUT);
    if x > cut_enc {
        (10.0f32.powf((x - D) / C) - B) / A
    } else {
        (x - F) / E
    }
}

// ---- ARRI LogC4 (ARRI LogC4 specification, 2022) ----
// colour-science / ARRI: a, b, c, s, t as published.

fn logc4_constants() -> (f32, f32, f32, f32, f32) {
    let a = ((1u32 << 18) as f32 - 16.0) / 117.45;
    let b = (1023.0 - 95.0) / 1023.0;
    let c = 95.0 / 1023.0;
    let s = (7.0 * std::f32::consts::LN_2 * 2.0f32.powf(7.0 - 14.0 * c / b)) / (a * b);
    let t = (2.0f32.powf(14.0 * (-c / b) + 6.0) - 64.0) / a;
    (a, b, c, s, t)
}

pub fn logc4_encode(lin: f32) -> f32 {
    let (a, b, c, s, t) = logc4_constants();
    if lin < t {
        lin * s + c
    } else {
        ((lin * a + 64.0).log2() - 6.0) / 14.0 * b + c
    }
}

pub fn logc4_decode(x: f32) -> f32 {
    let (a, b, c, s, t) = logc4_constants();
    if x < t * s + c {
        (x - c) / s
    } else {
        (2.0f32.powf((x - c) / b * 14.0 + 6.0) - 64.0) / a
    }
}

// ---- Sony S-Log3 ----

pub fn slog3_encode(lin: f32) -> f32 {
    if lin >= 0.011_250_00 {
        (420.0 + ((lin + 0.01) / 0.19).log10() * 261.5) / 1023.0
    } else {
        (lin * (171.210_294_692_9 - 95.0) / 0.011_250_00 + 95.0) / 1023.0
    }
}

pub fn slog3_decode(x: f32) -> f32 {
    let cut = 171.210_294_692_9 / 1023.0;
    if x >= cut {
        0.19 * 10.0f32.powf((x * 1023.0 - 420.0) / 261.5) - 0.01
    } else {
        (x * 1023.0 - 95.0) * 0.011_250_00 / (171.210_294_692_9 - 95.0)
    }
}

// ---- Panasonic V-Log ----

pub fn vlog_encode(lin: f32) -> f32 {
    const CUT: f32 = 0.01;
    const B: f32 = 0.00873;
    const C: f32 = 0.241_514;
    const D: f32 = 0.598_206;
    if lin < CUT {
        5.6 * lin + 0.125
    } else {
        C * (lin + B).log10() + D
    }
}

pub fn vlog_decode(x: f32) -> f32 {
    const CUT: f32 = 0.01;
    const B: f32 = 0.00873;
    const C: f32 = 0.241_514;
    const D: f32 = 0.598_206;
    if x < vlog_encode(CUT) {
        (x - 0.125) / 5.6
    } else {
        10.0f32.powf((x - D) / C) - B
    }
}

// ---- PQ (ST.2084). Scene mapping: 100 nits ↔ linear 1.0 ----

const PQ_M1: f32 = 2610.0 / 16384.0;
const PQ_M2: f32 = 2523.0 / 32.0;
const PQ_C1: f32 = 3424.0 / 4096.0;
const PQ_C2: f32 = 2413.0 / 128.0;
const PQ_C3: f32 = 2392.0 / 128.0;
const PQ_SCENE_NITS: f32 = 100.0;

pub fn pq_encode_nits(nits: f32) -> f32 {
    let y = (nits / 10_000.0).max(0.0);
    let ym = y.powf(PQ_M1);
    ((PQ_C1 + PQ_C2 * ym) / (1.0 + PQ_C3 * ym)).powf(PQ_M2)
}

pub fn pq_decode_nits(e: f32) -> f32 {
    let ep = e.max(0.0).powf(1.0 / PQ_M2);
    let num = (ep - PQ_C1).max(0.0);
    let den = (PQ_C2 - PQ_C3 * ep).max(1e-10);
    (num / den).powf(1.0 / PQ_M1) * 10_000.0
}

pub fn pq_encode_scene(lin: f32) -> f32 {
    pq_encode_nits(lin.max(0.0) * PQ_SCENE_NITS)
}

pub fn pq_decode_scene(e: f32) -> f32 {
    pq_decode_nits(e) / PQ_SCENE_NITS
}

// ---- HLG (ITU-R BT.2100) scene-referred OETF / inverse ----

pub fn hlg_oetf(lin: f32) -> f32 {
    const A: f32 = 0.178_832_77;
    const B: f32 = 0.284_668_92;
    const C: f32 = 0.559_910_73;
    if lin <= 1.0 / 12.0 {
        (3.0 * lin.max(0.0)).sqrt()
    } else {
        A * (12.0 * lin - B).ln() + C
    }
}

pub fn hlg_eotf(e: f32) -> f32 {
    const A: f32 = 0.178_832_77;
    const B: f32 = 0.284_668_92;
    const C: f32 = 0.559_910_73;
    if e <= 0.5 {
        (e * e) / 3.0
    } else {
        (((e - C) / A).exp() + B) / 12.0
    }
}

/// Full IDT: encoded camera RGB → linear Rec.2020.
pub fn idt_to_rec2020(rgb_enc: [f32; 3], transfer: Transfer, primaries: Primaries) -> [f32; 3] {
    let lin_cam = decode(transfer, rgb_enc);
    mat_vec(&to_rec2020(primaries), lin_cam)
}

/// Inverse (for tests): linear Rec.2020 → encoded camera RGB.
pub fn rec2020_to_idt(rgb_lin: [f32; 3], transfer: Transfer, primaries: Primaries) -> [f32; 3] {
    let lin_cam = mat_vec(&rec2020_to(primaries), rgb_lin);
    encode(transfer, lin_cam)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, eps: f32) {
        assert!((a - b).abs() < eps, "{a} vs {b} (eps {eps})");
    }

    fn roundtrip(t: Transfer, lin: f32) {
        let e = encode(t, [lin, lin, lin])[0];
        let d = decode(t, [e, e, e])[0];
        close(d, lin, 2e-4);
    }

    #[test]
    fn srgb_identity_on_unit_interval() {
        for &x in &[0.0, 0.001, 0.18, 0.5, 1.0] {
            roundtrip(Transfer::Srgb, x);
        }
        // Super-white encode stays ordered.
        let a = srgb_oetf(1.0);
        let b = srgb_oetf(1.25);
        let c = srgb_oetf(2.0);
        assert!(a < b && b < c, "{a} {b} {c}");
        close(srgb_eotf(srgb_oetf(1.25)), 1.25, 2e-4);
    }

    #[test]
    fn slog3_mid_grey() {
        // Sony: 18% grey → code 420/1023.
        let e = slog3_encode(0.18);
        close(e, 420.0 / 1023.0, 1e-5);
        close(slog3_decode(e), 0.18, 1e-5);
    }

    #[test]
    fn logc3_roundtrip_and_cut() {
        for &x in &[0.0, 0.005, 0.18, 1.0, 4.0] {
            roundtrip(Transfer::LogC3, x);
        }
    }

    #[test]
    fn logc4_roundtrip() {
        for &x in &[0.0, 0.001, 0.18, 1.0, 8.0] {
            let e = logc4_encode(x);
            let d = logc4_decode(e);
            close(d, x, 4e-4);
        }
    }

    #[test]
    fn acescct_roundtrip() {
        for &x in &[0.001, 0.18, 1.0, 4.0] {
            roundtrip(Transfer::Acescct, x);
        }
    }

    #[test]
    fn vlog_roundtrip() {
        for &x in &[0.0, 0.005, 0.18, 1.0] {
            roundtrip(Transfer::VLog, x);
        }
    }

    #[test]
    fn cineon_roundtrip() {
        for &x in &[0.01, 0.18, 1.0] {
            roundtrip(Transfer::Cineon, x);
        }
    }

    #[test]
    fn hlg_roundtrip() {
        for &x in &[0.0, 0.05, 0.26, 1.0] {
            roundtrip(Transfer::Hlg, x);
        }
    }

    #[test]
    fn pq_100_nits_is_scene_one() {
        close(pq_decode_scene(pq_encode_scene(1.0)), 1.0, 2e-4);
        close(pq_decode_nits(pq_encode_nits(100.0)), 100.0, 0.05);
    }

    #[test]
    fn rec709_rec2020_neutral_axis() {
        let m = rec2020_to(Primaries::Rec709);
        let g = mat_vec(&m, [0.2, 0.2, 0.2]);
        close(g[0], g[1], 1e-4);
        close(g[1], g[2], 1e-4);
        let back = mat_vec(&to_rec2020(Primaries::Rec709), g);
        close(back[0], 0.2, 1e-4);
    }

    #[test]
    fn highlight_extend_identity_then_ordered() {
        close(highlight_extend(0.5), 0.5, 1e-6);
        close(highlight_extend(1.0), 1.0, 1e-6);
        let a = highlight_extend(1.1);
        let b = highlight_extend(1.5);
        let c = highlight_extend(4.0);
        assert!(a > 1.0 && a < b && b < c && c < 2.0);
    }

    #[test]
    fn idt_identity_rec709() {
        // Encoded Rec.709 18% grey-ish mid (sRGB ~0.461) lands near linear 0.18
        // after 709 decode; primaries Rec.709 → Rec.2020 keeps neutrals.
        let enc = [srgb_oetf(0.18); 3];
        let lin = idt_to_rec2020(enc, Transfer::Srgb, Primaries::Rec709);
        close(lin[0], 0.18, 2e-4);
        close(lin[0], lin[1], 1e-5);
    }
}
