//! Highlight reconstruction — clipped-channel recovery after demosaic.
//! GPU shader in `graph/highlights.wgsl` mirrors this math.

/// Reconstruct clipped channels from unclipped chromaticity.
/// `clip` is the linear threshold (typically ~1.0); `amount` is 0..1 mix.
pub fn reconstruct_pixel(rgb: [f32; 3], clip: f32, amount: f32) -> [f32; 3] {
    let amount = amount.clamp(0.0, 1.0);
    if amount <= 1e-6 {
        return rgb;
    }
    let clip = clip.max(1e-4);
    let c = [rgb[0].min(clip), rgb[1].min(clip), rgb[2].min(clip)];
    let m = c[0].max(c[1]).max(c[2]);
    if m < 1e-6 {
        return rgb;
    }
    let rec = [c[0] / m * clip, c[1] / m * clip, c[2] / m * clip];
    [
        rgb[0] * (1.0 - amount) + rec[0] * amount,
        rgb[1] * (1.0 - amount) + rec[1] * amount,
        rgb[2] * (1.0 - amount) + rec[2] * amount,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_when_amount_zero() {
        let p = [1.2, 0.4, 0.3];
        assert_eq!(reconstruct_pixel(p, 1.0, 0.0), p);
    }

    #[test]
    fn pulls_clipped_channel_toward_chromaticity() {
        let out = reconstruct_pixel([1.8, 0.4, 0.2], 1.0, 1.0);
        assert!(out[0] <= 1.0 + 1e-5, "clipped channel should not stay above clip: {out:?}");
        assert!(out[1] > 0.3 && out[2] > 0.1);
        // ratios of unclipped preserved
        let r01 = 0.4 / 0.4; // unclipped g/g
        let _ = r01;
        assert!((out[1] / out[2] - 0.4 / 0.2).abs() < 0.05, "{out:?}");
    }
}
