//! Colorimetry: matrices + camera→working-space math.
//! Working space everywhere: linear Rec.2020, scene-referred, D65.
//! Constants per color-science-reference.md §1–§5 (standard, high confidence).

use rawler::imgop::xyz::Illuminant;

pub type Mat3 = [[f32; 3]; 3];

/// Rec.2020 (linear) → XYZ (D65). reference §2.
pub const REC2020_TO_XYZ: Mat3 = [
    [0.6369580, 0.1446169, 0.1688810],
    [0.2627002, 0.6779981, 0.0593017],
    [0.0000000, 0.0280727, 1.0609851],
];

/// XYZ (D65) → Rec.2020 (linear). reference §2.
pub const XYZ_TO_REC2020: Mat3 = [
    [1.7166512, -0.3556708, -0.2533663],
    [-0.6666844, 1.6164812, 0.0157685],
    [0.0176399, -0.0427706, 0.9421031],
];

/// sRGB (linear) → XYZ (D65). reference §2.
pub const SRGB_TO_XYZ: Mat3 = [
    [0.4123908, 0.3575843, 0.1804808],
    [0.2126390, 0.7151687, 0.0721923],
    [0.0193308, 0.1191948, 0.9505322],
];

/// XYZ (D65) → sRGB (linear). reference §2.
pub const XYZ_TO_SRGB: Mat3 = [
    [3.2409699, -1.5373832, -0.4986108],
    [-0.9692436, 1.8759675, 0.0415551],
    [0.0556301, -0.2039770, 1.0569715],
];

/// Display P3 (linear) → XYZ (D65). CSS Color 4 / Apple P3.
pub const DISPLAY_P3_TO_XYZ: Mat3 = [
    [0.4865709, 0.2656677, 0.1982173],
    [0.2289746, 0.6917385, 0.0792869],
    [0.0000000, 0.0451134, 1.0439444],
];

/// Build an RGB→XYZ matrix from chromaticities (ITU-R BT.709 style).
/// `r,g,b,w` are (x, y) pairs. White is typically D65 (0.3127, 0.3290).
pub fn rgb_to_xyz_from_xy(r: (f32, f32), g: (f32, f32), b: (f32, f32), w: (f32, f32)) -> Mat3 {
    let xy_to_xyz = |x: f32, y: f32| -> [f32; 3] {
        if y.abs() < 1e-8 {
            return [0.0, 0.0, 0.0];
        }
        [x / y, 1.0, (1.0 - x - y) / y]
    };
    let mut xr = xy_to_xyz(r.0, r.1);
    let mut xg = xy_to_xyz(g.0, g.1);
    let mut xb = xy_to_xyz(b.0, b.1);
    let wxyz = xy_to_xyz(w.0, w.1);
    // X = [Xr Xg Xb]; S = X^{-1} W
    let x = [
        [xr[0], xg[0], xb[0]],
        [xr[1], xg[1], xb[1]],
        [xr[2], xg[2], xb[2]],
    ];
    let inv = mat_inverse(&x).expect("primaries XY invertible");
    let s = mat_vec(&inv, wxyz);
    for i in 0..3 {
        xr[i] *= s[0];
        xg[i] *= s[1];
        xb[i] *= s[2];
    }
    [
        [xr[0], xg[0], xb[0]],
        [xr[1], xg[1], xb[1]],
        [xr[2], xg[2], xb[2]],
    ]
}

pub fn mat_mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut out = [[0.0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

pub fn mat_vec(m: &Mat3, v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

pub fn mat_inverse(m: &Mat3) -> Option<Mat3> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv_det = 1.0 / det;
    let mut out = [[0.0f32; 3]; 3];
    out[0][0] = (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * inv_det;
    out[0][1] = (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv_det;
    out[0][2] = (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv_det;
    out[1][0] = (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * inv_det;
    out[1][1] = (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv_det;
    out[1][2] = (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv_det;
    out[2][0] = (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * inv_det;
    out[2][1] = (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv_det;
    out[2][2] = (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv_det;
    Some(out)
}

// ---- Oklab (Björn Ottosson, public reference implementation) ----
// Used for the perceptual modules (color_grade slot 5, hsl slot 6).
// NOTE: design docs name Kirk Yrg / JzAzBz; reference §9 forbids keying
// those constants from memory. Oklab is the same class (hue-uniform
// perceptual space) with constants verified against the published source.

/// XYZ (D65) → LMS (Oklab M1).
pub const OKLAB_M1: Mat3 = [
    [0.8189330101, 0.3618667424, -0.1288597137],
    [0.0329845436, 0.9293118715, 0.0361456387],
    [0.0482003018, 0.2643662691, 0.6338517070],
];

/// LMS' (cbrt) → Lab (Oklab M2).
pub const OKLAB_M2: Mat3 = [
    [0.2104542553, 0.7936177850, -0.0040720468],
    [1.9779984951, -2.4285922050, 0.4505937099],
    [0.0259040371, 0.7827717662, -0.8086757660],
];

/// Matrices for shader-side Rec.2020 ↔ Oklab round trips, computed (not
/// hand-keyed) from the pinned constants.
pub struct OklabMats {
    pub rec2020_to_lms: Mat3,
    pub lms_to_rec2020: Mat3,
    pub m2: Mat3,
    pub m2_inv: Mat3,
}

pub fn oklab_mats() -> OklabMats {
    let rec2020_to_lms = mat_mul(&OKLAB_M1, &REC2020_TO_XYZ);
    OklabMats {
        lms_to_rec2020: mat_inverse(&rec2020_to_lms).expect("invertible"),
        rec2020_to_lms,
        m2: OKLAB_M2,
        m2_inv: mat_inverse(&OKLAB_M2).expect("invertible"),
    }
}

fn cbrt_signed(v: f32) -> f32 {
    v.signum() * v.abs().cbrt()
}

/// CPU mirror of the shader path (tests + future CPU fallback).
pub fn rec2020_to_oklab(rgb: [f32; 3]) -> [f32; 3] {
    let m = oklab_mats();
    let lms = mat_vec(&m.rec2020_to_lms, rgb);
    let lms_p = [
        cbrt_signed(lms[0]),
        cbrt_signed(lms[1]),
        cbrt_signed(lms[2]),
    ];
    mat_vec(&m.m2, lms_p)
}

pub fn oklab_to_rec2020(lab: [f32; 3]) -> [f32; 3] {
    let m = oklab_mats();
    let lms_p = mat_vec(&m.m2_inv, lab);
    let lms = [lms_p[0].powi(3), lms_p[1].powi(3), lms_p[2].powi(3)];
    mat_vec(&m.lms_to_rec2020, lms)
}

/// Bradford cone matrix (XYZ→LMS). reference §4.
pub const BRADFORD: Mat3 = [
    [0.8951000, 0.2664000, -0.1614000],
    [-0.7502000, 1.7135000, 0.0367000],
    [0.0389000, -0.0685000, 1.0296000],
];

/// Planckian locus chromaticity x(T), y(x) — Kang et al. 2002 cubic
/// approximation, valid ~1667K..25000K.
pub fn planckian_xy(cct: f32, tint: f32) -> (f32, f32) {
    let t = cct.clamp(1667.0, 25000.0) as f64;
    let x = if t <= 4000.0 {
        -0.2661239e9 / (t * t * t) - 0.2343589e6 / (t * t) + 0.8776956e3 / t + 0.179910
    } else {
        -3.0258469e9 / (t * t * t) + 2.1070379e6 / (t * t) + 0.2226347e3 / t + 0.240390
    };
    let x2 = x * x;
    let x3 = x2 * x;
    let y = if t <= 2222.0 {
        -1.1063814 * x3 - 1.34811020 * x2 + 2.18555832 * x - 0.20219683
    } else if t <= 4000.0 {
        -0.9549476 * x3 - 1.37418593 * x2 + 2.09137015 * x - 0.16748867
    } else {
        3.0817580 * x3 - 5.87338670 * x2 + 3.75112997 * x - 0.37001483
    };
    // tint: perpendicular-ish green↔magenta offset, pinned scale
    (x as f32, y as f32 + tint * 0.0004)
}

fn xy_to_xyz(x: f32, y: f32) -> [f32; 3] {
    if y.abs() < 1e-6 {
        return [0.95047, 1.0, 1.08883];
    }
    [x / y, 1.0, (1.0 - x - y) / y]
}

/// Bradford chromatic adaptation: XYZ matrix mapping colors seen under
/// `src` white to their appearance under `dst` white. reference §4.
pub fn bradford_adapt(src_white_xyz: [f32; 3], dst_white_xyz: [f32; 3]) -> Mat3 {
    let lms_src = mat_vec(&BRADFORD, src_white_xyz);
    let lms_dst = mat_vec(&BRADFORD, dst_white_xyz);
    let mut d = [[0.0f32; 3]; 3];
    for i in 0..3 {
        d[i][i] = if lms_src[i].abs() > 1e-9 {
            lms_dst[i] / lms_src[i]
        } else {
            1.0
        };
    }
    let inv_b = mat_inverse(&BRADFORD).expect("bradford invertible");
    mat_mul(&inv_b, &mat_mul(&d, &BRADFORD))
}

/// White-balance matrix in linear Rec.2020 for the WB module (P2/P3).
/// Semantics: slider temp == as-shot CCT and tint == 0 → identity.
/// Raising temp renders warmer (adaptation from the slider white toward
/// the as-shot white).
pub fn wb_matrix_rec2020(temp: f32, tint: f32, as_shot_cct: f32) -> Mat3 {
    let src = xy_to_xyz_pair(planckian_xy(temp, tint));
    let dst = xy_to_xyz_pair(planckian_xy(as_shot_cct, 0.0));
    let adapt = bradford_adapt(src, dst);
    mat_mul(&XYZ_TO_REC2020, &mat_mul(&adapt, &REC2020_TO_XYZ))
}

fn xy_to_xyz_pair(p: (f32, f32)) -> [f32; 3] {
    xy_to_xyz(p.0, p.1)
}

/// WB eyedropper solve: find (temp, tint) so the WB module neutralizes a
/// sampled cast `c` (linear Rec.2020 of a should-be-neutral patch).
/// Coordinate descent: temp balances R/B, tint balances G vs (R+B)/2 —
/// both relations are monotonic over the slider ranges.
pub fn solve_wb_for_neutral(c: [f32; 3], as_shot_cct: f32) -> (f32, f32) {
    let eval = |temp: f32, tint: f32| -> [f32; 3] {
        let m = wb_matrix_rec2020(temp, tint, as_shot_cct);
        let out = mat_vec(&m, c);
        let g = out[1].max(1e-6);
        [out[0] / g, 1.0, out[2] / g]
    };
    let mut temp = as_shot_cct;
    let mut tint = 0.0f32;
    for _ in 0..3 {
        // temp: bisection on r-b over log range
        let (mut lo, mut hi) = (2000.0f32, 25000.0f32);
        // raising temp raises R (warmer): r-b increases with temp
        for _ in 0..24 {
            let mid = (lo.ln() + hi.ln()).exp_div2();
            let o = eval(mid, tint);
            if o[0] - o[2] < 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        temp = (lo.ln() + hi.ln()).exp_div2();
        // tint: bisection on g vs (r+b)/2; positive tint pushes magenta (R+B up)
        let (mut tlo, mut thi) = (-150.0f32, 150.0f32);
        for _ in 0..20 {
            let mid = (tlo + thi) * 0.5;
            let o = eval(temp, mid);
            let rb = (o[0] + o[2]) * 0.5;
            if rb < 1.0 {
                tlo = mid;
            } else {
                thi = mid;
            }
        }
        tint = (tlo + thi) * 0.5;
    }
    (temp.clamp(2000.0, 50000.0), tint.clamp(-150.0, 150.0))
}

trait ExpDiv2 {
    fn exp_div2(self) -> f32;
}
impl ExpDiv2 for f32 {
    fn exp_div2(self) -> f32 {
        (self * 0.5).exp()
    }
}

pub fn mat_is_identity(m: &Mat3, eps: f32) -> bool {
    for i in 0..3 {
        for j in 0..3 {
            let want = if i == j { 1.0 } else { 0.0 };
            if (m[i][j] - want).abs() > eps {
                return false;
            }
        }
    }
    true
}

/// Nominal CCT for DNG calibration illuminants (DNG spec / EXIF LightSource).
pub fn illuminant_cct(ill: &Illuminant) -> f32 {
    match ill {
        Illuminant::A => 2856.0,
        Illuminant::B => 4874.0,
        Illuminant::C => 6774.0,
        Illuminant::D50 => 5003.0,
        Illuminant::D55 => 5503.0,
        Illuminant::D65 => 6504.0,
        Illuminant::D75 => 7504.0,
        Illuminant::Tungsten | Illuminant::IsoStudioTungsten => 3200.0,
        Illuminant::Daylight | Illuminant::FineWeather => 5500.0,
        Illuminant::CloudyWeather => 6000.0,
        Illuminant::Shade => 7500.0,
        Illuminant::Flash => 5500.0,
        Illuminant::Fluorescent
        | Illuminant::DaylightFluorescent
        | Illuminant::DaylightWhiteFluorescent
        | Illuminant::CoolWhiteFluorescent
        | Illuminant::WhiteFluorescent => 4200.0,
        Illuminant::Unknown => 5500.0,
    }
}

/// McCamy approximation: chromaticity (x, y) → CCT in Kelvin.
pub fn mccamy_cct(x: f32, y: f32) -> f32 {
    let n = (x - 0.3320) / (0.1858 - y);
    449.0 * n.powi(3) + 3525.0 * n * n + 6823.3 * n + 5520.33
}

/// A camera's colorimetric calibration: per-illuminant XYZ→camera matrices.
pub struct CameraCalibration {
    /// (CCT of calibration illuminant, xyz→cam 3x3), sorted by CCT ascending.
    pub calibrations: Vec<(f32, Mat3)>,
}

impl CameraCalibration {
    pub fn from_rawler(color_matrix: &std::collections::HashMap<Illuminant, Vec<f32>>) -> Self {
        let mut calibrations: Vec<(f32, Mat3)> = color_matrix
            .iter()
            .filter(|(_, m)| m.len() >= 9)
            .map(|(ill, m)| {
                let mat = [[m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]];
                (illuminant_cct(ill), mat)
            })
            .collect();
        calibrations.sort_by(|a, b| a.0.total_cmp(&b.0));
        Self { calibrations }
    }

    /// Fallback when `color_matrix` is empty — common on older CR2/NEF where
    /// rawler only populates the legacy `xyz_to_cam` table.
    pub fn from_xyz_to_cam(xyz_to_cam: &[[f32; 3]; 4]) -> Self {
        let mat = [
            [xyz_to_cam[0][0], xyz_to_cam[0][1], xyz_to_cam[0][2]],
            [xyz_to_cam[1][0], xyz_to_cam[1][1], xyz_to_cam[1][2]],
            [xyz_to_cam[2][0], xyz_to_cam[2][1], xyz_to_cam[2][2]],
        ];
        Self {
            calibrations: vec![(6504.0, mat)],
        }
    }

    /// Prefer dual-illuminant `color_matrix`; fall back to legacy `xyz_to_cam`.
    pub fn from_raw(raw: &rawler::RawImage) -> Self {
        let cal = Self::from_rawler(&raw.color_matrix);
        if cal.calibrations.is_empty() {
            Self::from_xyz_to_cam(&raw.xyz_to_cam)
        } else {
            cal
        }
    }

    /// Interpolate xyz→cam by inverse-CCT weighting between the two
    /// calibration illuminants (DNG dual-illuminant model, reference §5.4).
    pub fn xyz_to_cam_at(&self, cct: f32) -> Option<Mat3> {
        match self.calibrations.len() {
            0 => None,
            1 => Some(self.calibrations[0].1),
            _ => {
                let (t1, m1) = &self.calibrations[0]; // low CCT (e.g. A)
                let (t2, m2) = self.calibrations.last().unwrap(); // high CCT (e.g. D65)
                let cct = cct.clamp(*t1, *t2);
                // w = (1/T − 1/T2)/(1/T1 − 1/T2); w weights the low-CCT matrix
                let w = (1.0 / cct - 1.0 / t2) / (1.0 / t1 - 1.0 / t2);
                let mut out = [[0.0f32; 3]; 3];
                for i in 0..3 {
                    for j in 0..3 {
                        out[i][j] = w * m1[i][j] + (1.0 - w) * m2[i][j];
                    }
                }
                Some(out)
            }
        }
    }

    /// Estimate the as-shot CCT from camera WB multipliers by iterating:
    /// neutral_cam = 1/wb → XYZ via inv(M(T)) → xy → McCamy CCT → repeat.
    pub fn estimate_cct(&self, wb: &[f32; 4]) -> f32 {
        let neutral_cam = [
            if wb[0] > 0.0 { 1.0 / wb[0] } else { 1.0 },
            if wb[1] > 0.0 { 1.0 / wb[1] } else { 1.0 },
            if wb[2] > 0.0 { 1.0 / wb[2] } else { 1.0 },
        ];
        let mut cct = 6504.0f32;
        for _ in 0..6 {
            let Some(m) = self.xyz_to_cam_at(cct) else {
                return cct;
            };
            let Some(inv) = mat_inverse(&m) else {
                return cct;
            };
            let xyz = mat_vec(&inv, neutral_cam);
            let sum = xyz[0] + xyz[1] + xyz[2];
            if sum <= 0.0 {
                return cct;
            }
            let (x, y) = (xyz[0] / sum, xyz[1] / sum);
            let next = mccamy_cct(x, y).clamp(1500.0, 25000.0);
            if (next - cct).abs() < 1.0 {
                return next;
            }
            cct = next;
        }
        cct
    }

    /// Build the camera→Rec.2020 matrix for WB-multiplied camera RGB
    /// (dcraw-style row normalization so WB'd neutral → neutral):
    ///   rgb2cam = rownorm(xyz2cam(T) · REC2020_TO_XYZ); cam2rgb = inv(rgb2cam)
    pub fn cam_to_rec2020(&self, wb: &[f32; 4]) -> Option<Mat3> {
        let cct = self.estimate_cct(wb);
        let xyz2cam = self.xyz_to_cam_at(cct)?;
        let mut rgb2cam = mat_mul(&xyz2cam, &REC2020_TO_XYZ);
        for row in rgb2cam.iter_mut() {
            let sum: f32 = row.iter().sum();
            if sum.abs() > 1e-9 {
                for v in row.iter_mut() {
                    *v /= sum;
                }
            }
        }
        mat_inverse(&rgb2cam)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn from_xyz_to_cam_fallback() {
        let xyz = [
            [0.7, 0.1, 0.1],
            [0.2, 0.9, 0.1],
            [0.1, 0.1, 0.8],
            [0.0, 0.0, 0.0],
        ];
        let cal = CameraCalibration::from_xyz_to_cam(&xyz);
        assert_eq!(cal.calibrations.len(), 1);
        assert!(cal.cam_to_rec2020(&[1.0, 1.0, 1.0, 1.0]).is_some());
    }

    #[test]
    fn rec2020_matrices_are_inverses() {
        let prod = mat_mul(&REC2020_TO_XYZ, &XYZ_TO_REC2020);
        for i in 0..3 {
            for j in 0..3 {
                let want = if i == j { 1.0 } else { 0.0 };
                assert!(
                    approx(prod[i][j], want, 1e-4),
                    "prod[{i}][{j}]={}",
                    prod[i][j]
                );
            }
        }
    }

    #[test]
    fn rec2020_white_maps_to_d65() {
        // (1,1,1) in Rec.2020 → D65 white XYZ (0.9505, 1.0, 1.0891)
        let xyz = mat_vec(&REC2020_TO_XYZ, [1.0, 1.0, 1.0]);
        assert!(approx(xyz[0], 0.95047, 2e-3));
        assert!(approx(xyz[1], 1.0, 2e-3));
        assert!(approx(xyz[2], 1.08883, 2e-3));
    }

    #[test]
    fn mat_inverse_round_trips() {
        let m: Mat3 = [[0.5, 0.2, 0.1], [0.1, 0.9, 0.0], [0.05, 0.1, 1.2]];
        let inv = mat_inverse(&m).unwrap();
        let prod = mat_mul(&m, &inv);
        for i in 0..3 {
            for j in 0..3 {
                let want = if i == j { 1.0 } else { 0.0 };
                assert!(approx(prod[i][j], want, 1e-5));
            }
        }
    }

    #[test]
    fn mccamy_d65_chromaticity_near_6500k() {
        let cct = mccamy_cct(0.3127, 0.3290);
        assert!((cct - 6504.0).abs() < 60.0, "cct={cct}");
    }

    #[test]
    fn oklab_white_is_l1_chroma0() {
        let lab = rec2020_to_oklab([1.0, 1.0, 1.0]);
        assert!((lab[0] - 1.0).abs() < 0.01, "L={}", lab[0]);
        assert!(lab[1].abs() < 0.005 && lab[2].abs() < 0.005, "{lab:?}");
    }

    #[test]
    fn oklab_round_trips() {
        for rgb in [
            [0.18, 0.18, 0.18],
            [0.6, 0.2, 0.1],
            [0.05, 0.4, 0.7],
            [1.5, 1.2, 0.9],
        ] {
            let back = oklab_to_rec2020(rec2020_to_oklab(rgb));
            for c in 0..3 {
                assert!((back[c] - rgb[c]).abs() < 2e-3, "{rgb:?} → {back:?}");
            }
        }
    }

    #[test]
    fn oklab_hue_angles_sane() {
        // red-ish should land near 25–40°, blue-ish near 240–280°
        let red = rec2020_to_oklab([0.8, 0.05, 0.05]);
        let h_red = red[2].atan2(red[1]).to_degrees().rem_euclid(360.0);
        assert!((15.0..60.0).contains(&h_red), "red hue {h_red}");
        let blue = rec2020_to_oklab([0.05, 0.1, 0.8]);
        let h_blue = blue[2].atan2(blue[1]).to_degrees().rem_euclid(360.0);
        assert!((230.0..290.0).contains(&h_blue), "blue hue {h_blue}");
    }

    #[test]
    fn planckian_d65_near_d65_chromaticity() {
        let (x, y) = planckian_xy(6504.0, 0.0);
        // Planckian locus sits slightly off the daylight locus; loose band
        assert!((x - 0.3127).abs() < 0.01, "x={x}");
        assert!((y - 0.3290).abs() < 0.015, "y={y}");
    }

    #[test]
    fn wb_identity_at_as_shot() {
        let m = wb_matrix_rec2020(5200.0, 0.0, 5200.0);
        assert!(mat_is_identity(&m, 1e-4), "{m:?}");
    }

    #[test]
    fn wb_warmer_boosts_red_cuts_blue() {
        let m = wb_matrix_rec2020(8000.0, 0.0, 5200.0);
        let out = mat_vec(&m, [1.0, 1.0, 1.0]);
        assert!(out[0] > 1.02, "R should rise, got {}", out[0]);
        assert!(out[2] < 0.98, "B should fall, got {}", out[2]);
    }

    #[test]
    fn wb_tint_positive_pushes_magenta() {
        // positive tint (toward magenta) should raise R+B relative to G
        let m = wb_matrix_rec2020(5200.0, 50.0, 5200.0);
        let out = mat_vec(&m, [1.0, 1.0, 1.0]);
        assert!(out[1] < (out[0] + out[2]) * 0.5, "{out:?}");
    }

    #[test]
    fn wb_solver_neutralizes_synthetic_cast() {
        // make a cast: what a gray looks like when the scene was 4000K but
        // camera assumed 5200K → cast = inverse of the 4000K correction
        let m = wb_matrix_rec2020(4000.0, 20.0, 5200.0);
        let inv = mat_inverse(&m).unwrap();
        let cast = mat_vec(&inv, [0.5, 0.5, 0.5]);
        let (temp, tint) = solve_wb_for_neutral(cast, 5200.0);
        let fixed = mat_vec(&wb_matrix_rec2020(temp, tint, 5200.0), cast);
        let g = fixed[1];
        assert!(
            (fixed[0] / g - 1.0).abs() < 0.01,
            "{fixed:?} temp={temp} tint={tint}"
        );
        assert!((fixed[2] / g - 1.0).abs() < 0.01, "{fixed:?}");
    }

    #[test]
    fn bradford_identity_for_same_white() {
        let w = [0.95047, 1.0, 1.08883];
        assert!(mat_is_identity(&bradford_adapt(w, w), 1e-5));
    }

    #[test]
    fn neutral_camera_pixel_lands_neutral_in_rec2020() {
        // Synthetic identity-ish camera: xyz2cam = XYZ_TO_REC2020-like.
        // With wb that neutralizes, a wb'd (1,1,1) must map to equal channels.
        let mut cm = std::collections::HashMap::new();
        let flat: Vec<f32> = XYZ_TO_REC2020.iter().flatten().copied().collect();
        cm.insert(Illuminant::D65, flat);
        let cal = CameraCalibration::from_rawler(&cm);
        let m = cal.cam_to_rec2020(&[2.0, 1.0, 1.5, f32::NAN]).unwrap();
        let out = mat_vec(&m, [1.0, 1.0, 1.0]);
        assert!(
            approx(out[0], out[1], 1e-3) && approx(out[1], out[2], 1e-3),
            "{out:?}"
        );
    }
}
