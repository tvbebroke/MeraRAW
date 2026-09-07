//! Koschmieder / dark-channel dehaze. The GPU effects pass must match
//! `apply` + `estimate_dark_and_airlight` in `graph/effects.wgsl`.
//!
//! This is atmospheric veil removal, not midtone unsharp (that's Clarity).
//! Positive `amount` inverts `I = J t + A (1-t)`; negative applies it.

const LUMA: [f32; 3] = [0.2627, 0.6780, 0.0593];

fn min3(v: [f32; 3]) -> f32 {
    v[0].min(v[1]).min(v[2])
}

fn max3(v: [f32; 3]) -> f32 {
    v[0].max(v[1]).max(v[2])
}

fn luma(v: [f32; 3]) -> f32 {
    v[0] * LUMA[0] + v[1] * LUMA[1] + v[2] * LUMA[2]
}

fn neutralize_airlight(a: [f32; 3]) -> [f32; 3] {
    let y = luma(a);
    [
        (a[0] * 0.65 + y * 0.35).max(0.08),
        (a[1] * 0.65 + y * 0.35).max(0.08),
        (a[2] * 0.65 + y * 0.35).max(0.08),
    ]
}

/// Local dark channel and airlight from a neighborhood (He et al. style).
pub fn estimate_dark_and_airlight(samples: &[[f32; 3]]) -> (f32, [f32; 3]) {
    let mut dc = f32::MAX;
    let mut a_acc = [0.0f32; 3];
    let mut a_w = 0.0;
    let mut bright_max = [0.0f32; 3];
    let mut bright_dc = -1.0f32;
    for s in samples {
        let p = [s[0].max(0.0), s[1].max(0.0), s[2].max(0.0)];
        let mn = min3(p);
        let mx = max3(p);
        dc = dc.min(mn);
        let chroma = mx - mn;
        let t = ((chroma - 0.04) / 0.24).clamp(0.0, 1.0);
        let smooth = t * t * (3.0 - 2.0 * t);
        let w = mx * (1.0 - smooth);
        a_acc[0] += p[0] * w;
        a_acc[1] += p[1] * w;
        a_acc[2] += p[2] * w;
        a_w += w;
        if mn > bright_dc {
            bright_dc = mn;
            bright_max = p;
        }
    }
    let a = if a_w > 1e-4 {
        [a_acc[0] / a_w, a_acc[1] / a_w, a_acc[2] / a_w]
    } else {
        bright_max
    };
    (dc, a)
}

/// `amount` is −1..1 (registry slider / 100). Zero is identity.
pub fn apply(rgb: [f32; 3], dark: f32, atmos: [f32; 3], amount: f32) -> [f32; 3] {
    if amount.abs() < 1e-6 {
        return rgb;
    }
    let a = neutralize_airlight(atmos);
    let omega = amount.abs() * 0.92;
    let a_min = min3(a).max(0.08);
    let t_raw = 1.0 - omega * dark / a_min;
    let t0 = 0.18 + (0.10 - 0.18) * amount.abs();
    let t = t_raw.clamp(t0, 1.0);
    if amount > 0.0 {
        let mut out = [
            ((rgb[0] - a[0]) / t + a[0]).max(0.0),
            ((rgb[1] - a[1]) / t + a[1]).max(0.0),
            ((rgb[2] - a[2]) / t + a[2]).max(0.0),
        ];
        let y = luma(out);
        let sat = 1.0 + 0.20 * amount * (1.0 - t);
        out = [
            (y + (out[0] - y) * sat).max(0.0),
            (y + (out[1] - y) * sat).max(0.0),
            (y + (out[2] - y) * sat).max(0.0),
        ];
        out
    } else {
        let t_haze = t * 0.40 + (1.0 - 0.48 * omega) * 0.60;
        [
            (rgb[0] * t_haze + a[0] * (1.0 - t_haze)).max(0.0),
            (rgb[1] * t_haze + a[1] * (1.0 - t_haze)).max(0.0),
            (rgb[2] * t_haze + a[2] * (1.0 - t_haze)).max(0.0),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l1(a: [f32; 3], b: [f32; 3]) -> f32 {
        (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
    }

    fn chroma(c: [f32; 3]) -> f32 {
        max3(c) - min3(c)
    }

    #[test]
    fn zero_is_identity() {
        let rgb = [0.4, 0.2, 0.1];
        assert_eq!(apply(rgb, 0.1, [0.8, 0.8, 0.8], 0.0), rgb);
    }

    #[test]
    fn recovers_veiled_color_toward_the_scene() {
        let j = [0.45, 0.12, 0.06];
        let a = [0.78, 0.80, 0.84];
        let t = 0.40;
        let i = [
            j[0] * t + a[0] * (1.0 - t),
            j[1] * t + a[1] * (1.0 - t),
            j[2] * t + a[2] * (1.0 - t),
        ];
        let out = apply(i, min3(i), a, 0.85);
        assert!(
            l1(out, j) < l1(i, j),
            "dehaze should unwrap Koschmieder veil, not wander: i={i:?} out={out:?} j={j:?}"
        );
        assert!(
            chroma(out) > chroma(i) + 0.05,
            "recovered chroma should rise (haze is grey): i={} out={}",
            chroma(i),
            chroma(out)
        );
    }

    #[test]
    fn airlight_pixel_is_not_crushed() {
        let a = [0.72, 0.74, 0.76];
        let out = apply(a, min3(a), a, 0.8);
        assert!(
            l1(out, a) < 0.08,
            "pure airlight must stay near itself (unlike a contrast punch): {out:?}"
        );
    }

    #[test]
    fn negative_adds_veil() {
        let j = [0.40, 0.15, 0.08];
        let a = [0.75, 0.77, 0.80];
        let out = apply(j, min3(j), a, -0.7);
        assert!(
            chroma(out) < chroma(j) - 0.03,
            "negative dehaze should wash chroma: j={} out={}",
            chroma(j),
            chroma(out)
        );
        assert!(
            l1(out, a) < l1(j, a),
            "negative dehaze should move toward airlight"
        );
    }

    #[test]
    fn clear_saturated_pixel_barely_moves() {
        let rgb = [0.55, 0.04, 0.03];
        let out = apply(rgb, min3(rgb), [0.8, 0.8, 0.8], 0.7);
        assert!(
            l1(out, rgb) < 0.08,
            "no veil (dark channel ≈ 0) so dehaze must not act like Clarity: {out:?}"
        );
    }

    #[test]
    fn neighborhood_recovers_hazy_patch() {
        let j = [0.42, 0.14, 0.07];
        let a = [0.76, 0.78, 0.82];
        let t = 0.42;
        let mut samples = vec![a; 81];
        for y in 0..9 {
            for x in 0..9 {
                if (3..6).contains(&x) && (3..6).contains(&y) {
                    samples[y * 9 + x] = [
                        j[0] * t + a[0] * (1.0 - t),
                        j[1] * t + a[1] * (1.0 - t),
                        j[2] * t + a[2] * (1.0 - t),
                    ];
                }
            }
        }
        let (dark, atmos) = estimate_dark_and_airlight(&samples);
        let i = samples[9 * 4 + 4];
        let out = apply(i, dark, atmos, 0.9);
        assert!(chroma(out) > chroma(i) + 0.04);
        assert!(l1(out, j) < l1(i, j));
    }
}
