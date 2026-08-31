//! DNG ProfileHueSatMap / ProfileLookTable — trilinear HSV delta LUTs.

use crate::color::{mat_vec, Mat3};

/// Linear Rec.2020 (D65) → linear ProPhoto (D50), precomputed.
const REC2020_TO_PROPHOTO: Mat3 = [
    [0.8351703, 0.0487892, 0.1159976],
    [0.0540198, 0.9289338, 0.0170577],
    [-0.0023388, 0.0363283, 0.9662183],
];

/// Linear ProPhoto (D50) → linear Rec.2020 (D65), precomputed.
const PROPHOTO_TO_REC2020: Mat3 = [
    [1.2006766, -0.0574642, -0.1431305],
    [-0.0699240, 1.0805933, -0.0106824],
    [0.0055354, -0.0407677, 1.0350180],
];

#[derive(Debug, Clone)]
pub struct HueSatMap {
    pub hue_div: u32,
    pub sat_div: u32,
    pub val_div: u32,
    /// Nested order: value → hue → sat; each entry is (hue_shift_deg, sat_scale, val_scale).
    pub deltas: Vec<[f32; 3]>,
}

impl HueSatMap {
    pub fn is_valid(&self) -> bool {
        let Some(n) = self
            .hue_div
            .checked_mul(self.sat_div)
            .and_then(|v| v.checked_mul(self.val_div))
        else {
            return false;
        };
        self.hue_div >= 1
            && self.sat_div >= 2
            && self.val_div >= 1
            && self.hue_div <= 360
            && self.sat_div <= 256
            && self.val_div <= 256
            && self.deltas.len() == n as usize
    }

    fn idx(&self, h: u32, s: u32, v: u32) -> usize {
        // Dims are capped in is_valid / DCP parse; compute in usize.
        (v as usize) * (self.hue_div as usize) * (self.sat_div as usize)
            + (h as usize) * (self.sat_div as usize)
            + (s as usize)
    }

    fn delta_at(&self, h: u32, s: u32, v: u32) -> [f32; 3] {
        self.deltas[self.idx(h, s, v)]
    }

    fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    }

    /// Trilinear sample; HSV inputs normalized to [0, 1] per DNG spec.
    pub fn sample(&self, h: f32, s: f32, v: f32) -> [f32; 3] {
        if !self.is_valid() {
            return [0.0, 1.0, 1.0];
        }
        let hd = self.hue_div as f32;
        let sd = self.sat_div as f32;
        let vd = self.val_div as f32;

        let mut hf = (h.fract().abs() + if h < 0.0 { 1.0 } else { 0.0 }) * hd;
        if hf >= hd {
            hf -= hd;
        }
        let sf = s.clamp(0.0, 1.0) * (sd - 1.0);
        let vf = if self.val_div <= 1 {
            0.0
        } else {
            v.clamp(0.0, 1.0) * (vd - 1.0)
        };

        let h0 = hf.floor() as u32 % self.hue_div;
        let h1 = (h0 + 1) % self.hue_div;
        let ht = hf - hf.floor();

        let s0 = sf.floor() as u32;
        let s1 = (s0 + 1).min(self.sat_div - 1);
        let st = sf - sf.floor();

        let v0 = vf.floor() as u32;
        let v1 = if self.val_div <= 1 {
            0
        } else {
            (v0 + 1).min(self.val_div - 1)
        };
        let vt = vf - vf.floor();

        let c000 = self.delta_at(h0, s0, v0);
        let c100 = self.delta_at(h1, s0, v0);
        let c010 = self.delta_at(h0, s1, v0);
        let c110 = self.delta_at(h1, s1, v0);
        let c001 = self.delta_at(h0, s0, v1);
        let c101 = self.delta_at(h1, s0, v1);
        let c011 = self.delta_at(h0, s1, v1);
        let c111 = self.delta_at(h1, s1, v1);

        let x00 = Self::lerp3(c000, c100, ht);
        let x10 = Self::lerp3(c010, c110, ht);
        let x01 = Self::lerp3(c001, c101, ht);
        let x11 = Self::lerp3(c011, c111, ht);
        let y0 = Self::lerp3(x00, x10, st);
        let y1 = Self::lerp3(x01, x11, st);
        Self::lerp3(y0, y1, vt)
    }

    /// Apply HSV delta table to linear ProPhoto RGB.
    pub fn apply_prophoto(&self, rgb: [f32; 3]) -> [f32; 3] {
        let hsv = rgb_to_hsv(rgb);
        let [dh, ss, vs] = self.sample(hsv[0], hsv[1], hsv[2]);
        let mut h = hsv[0] + dh / 360.0;
        h -= h.floor();
        let s = (hsv[1] * ss).clamp(0.0, 1.0);
        // Keep V unclamped so highlight headroom survives (matches GPU path).
        let v = (hsv[2] * vs).max(0.0);
        hsv_to_rgb([h, s, v])
    }
}

pub fn rgb_to_hsv(rgb: [f32; 3]) -> [f32; 3] {
    let r = rgb[0].max(0.0);
    let g = rgb[1].max(0.0);
    let b = rgb[2].max(0.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let v = max;
    if max <= 1e-10 {
        return [0.0, 0.0, 0.0];
    }
    let delta = max - min;
    let s = delta / max;
    if delta <= 1e-10 {
        return [0.0, 0.0, v];
    }
    let mut h_deg = if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    if h_deg < 0.0 {
        h_deg += 360.0;
    }
    [h_deg / 360.0, s, v]
}

pub fn hsv_to_rgb(hsv: [f32; 3]) -> [f32; 3] {
    let mut h_deg = (hsv[0].fract().abs()) * 360.0;
    if hsv[0] < 0.0 {
        h_deg = 360.0 - h_deg;
    }
    let s = hsv[1].clamp(0.0, 1.0);
    let v = hsv[2].max(0.0);
    if s <= 1e-10 {
        return [v, v, v];
    }
    let c = v * s;
    let hp = h_deg / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let m = v - c;
    let (rp, gp, bp) = if hp < 1.0 {
        (c, x, 0.0)
    } else if hp < 2.0 {
        (x, c, 0.0)
    } else if hp < 3.0 {
        (0.0, c, x)
    } else if hp < 4.0 {
        (0.0, x, c)
    } else if hp < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    [rp + m, gp + m, bp + m]
}

/// Apply dual-illuminant hue/sat maps in ProPhoto, then convert back to Rec.2020.
pub fn apply_hue_sat_maps(
    rec2020: [f32; 3],
    map1: Option<&HueSatMap>,
    map2: Option<&HueSatMap>,
    cct: f32,
    t1: f32,
    t2: f32,
) -> [f32; 3] {
    let map1 = match map1 {
        Some(m) if m.is_valid() => m,
        _ => return rec2020,
    };
    let pro = mat_vec(&REC2020_TO_PROPHOTO, rec2020);
    let pro = if let Some(m2) = map2.filter(|m| m.is_valid()) {
        let w = cct_weight(cct, t1, t2);
        let hsv = rgb_to_hsv(pro);
        let d1 = map1.sample(hsv[0], hsv[1], hsv[2]);
        let d2 = m2.sample(hsv[0], hsv[1], hsv[2]);
        let dh = w * d1[0] + (1.0 - w) * d2[0];
        let ss = w * d1[1] + (1.0 - w) * d2[1];
        let vs = w * d1[2] + (1.0 - w) * d2[2];
        let mut h = hsv[0] + dh / 360.0;
        h -= h.floor();
        let s = (hsv[1] * ss).clamp(0.0, 1.0);
        let v = (hsv[2] * vs).max(0.0);
        hsv_to_rgb([h, s, v])
    } else {
        map1.apply_prophoto(pro)
    };
    mat_vec(&PROPHOTO_TO_REC2020, pro)
}

/// Apply a single look table (same format as HueSatMap) after the hue/sat map pass.
pub fn apply_look_table(rec2020: [f32; 3], table: &HueSatMap) -> [f32; 3] {
    if !table.is_valid() {
        return rec2020;
    }
    let pro = mat_vec(&REC2020_TO_PROPHOTO, rec2020);
    let pro = table.apply_prophoto(pro);
    mat_vec(&PROPHOTO_TO_REC2020, pro)
}

/// Pull Fuji DCP cyan-leaning blues toward Affinity/LR.
///
/// Root issue: HueSatMap leaves B-dominant cyans at sat≈1 with R≈0; after
/// Rec.2020→sRGB they stay neon cyan. Affinity's bluish-green is ~hue 192°
/// at sat ~0.25 with real red; blue sky is ~227° at sat ~0.75.
pub fn correct_blue_cyan_cast(rgb: [f32; 3]) -> [f32; 3] {
    let b = rgb[2];
    if b <= 1e-6 || b < rgb[0] || b < rgb[1] * 0.98 {
        return rgb;
    }
    let hsv = rgb_to_hsv(rgb);
    let h = hsv[0] * 360.0;
    let s = hsv[1];
    let v = hsv[2];
    if s < 0.20 || v < 0.02 || !(175.0..=225.0).contains(&h) {
        return rgb;
    }

    let gb = rgb[1] / b;
    let rb = rgb[0] / b;
    let luma = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];

    if gb > 0.70 {
        // Bluish-green / cyan — Affinity keeps hue ~190–205° but much lower sat.
        let crush = ((0.35 - rb).max(0.0) / 0.35).clamp(0.0, 1.0);
        let strength = crush * ((s - 0.25) / 0.75).clamp(0.0, 1.0);
        if strength < 0.05 {
            return rgb;
        }
        let target_h = 198.0f32;
        let nh = (h + (target_h - h) * strength * 0.35).rem_euclid(360.0);
        // Pull sat down hard so R survives sRGB gamut map (Affinity ~0.25).
        let ns = (s * (1.0 - 0.70 * strength)).clamp(0.15, 1.0);
        let mut out = hsv_to_rgb([nh / 360.0, ns, v]);
        // Soft mix toward luma for gamut headroom, then nudge blue.
        let desat = 0.35 * strength;
        out = [
            out[0] * (1.0 - desat) + luma * desat,
            out[1] * (1.0 - desat) + luma * desat,
            out[2] * (1.0 - desat) + luma * desat,
        ];
        out[0] = out[0].max(luma * 0.55 * strength);
        out[2] = (out[2] + 0.04 * strength * b).max(out[0]);
        return [out[0].max(0.0), out[1].max(0.0), out[2].max(0.0)];
    }

    // Blue sky / primary blue — Affinity ~227°, R/B ≈ 0.22, sat moderated.
    // Keep this mild: aggressive G crush washed Nikon AW1 / Sony A7III blues.
    let hue_pull = ((225.0 - h) / 45.0).clamp(0.0, 1.0);
    let red_pull = ((0.18 - rb).max(0.0) / 0.18).clamp(0.0, 1.0);
    let strength = (hue_pull * 0.25 + red_pull * 0.45).clamp(0.0, 0.55);
    if strength < 0.06 {
        return rgb;
    }
    let target_h = 227.0f32;
    let nh = (h + (target_h - h) * strength * 0.65).rem_euclid(360.0);
    let ns = (s * (1.0 - 0.10 * strength)).clamp(0.0, 1.0);
    let mut out = hsv_to_rgb([nh / 360.0, ns, v]);
    let want_r = b * 0.20;
    if out[0] < want_r {
        out[0] += (want_r - out[0]) * strength * 0.7;
    }
    out[1] *= 1.0 - 0.08 * strength;
    [out[0].max(0.0), out[1].max(0.0), out[2].max(0.0)]
}

pub fn cct_weight(cct: f32, t1: f32, t2: f32) -> f32 {
    if (t1 - t2).abs() < 1.0 {
        return 1.0;
    }
    let cct = cct.clamp(t1.min(t2), t1.max(t2));
    let (lo, hi) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
    (1.0 / cct - 1.0 / hi) / (1.0 / lo - 1.0 / hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_map_is_noop() {
        let map = HueSatMap {
            hue_div: 2,
            sat_div: 2,
            val_div: 1,
            deltas: vec![[0.0, 1.0, 1.0]; 4],
        };
        let rgb = [0.4, 0.2, 0.1];
        let out = map.apply_prophoto(rgb);
        assert!((out[0] - rgb[0]).abs() < 0.02);
    }

    #[test]
    fn fixes_fuji_cyan_patch() {
        // High-G/B cyan (bluish-green) — restore red, stay cyan-green.
        let cyan = [0.0, 0.42, 0.48];
        let out = correct_blue_cyan_cast(cyan);
        assert!(out[0] > cyan[0], "red should lift on cyan patch");

        // Blue-sky style: low R, mid G — Affinity lands ~227°.
        let sky = [0.0, 0.18, 0.55];
        let out = correct_blue_cyan_cast(sky);
        let h = rgb_to_hsv(out)[0] * 360.0;
        assert!(h > 215.0, "sky hue {h} still cyan");
        assert!(out[0] > sky[0], "sky red should lift");
    }
}
