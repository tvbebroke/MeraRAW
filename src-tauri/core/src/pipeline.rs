//! Pipeline instrumentation — prove where pixels go wrong.
//!
//! Enable with `MERARAW_PIPELINE_TRACE=1`. Optional dump directory:
//! `MERARAW_PIPELINE_DUMP=/tmp/mera-pipe` writes a no-edit sRGB PNG of the
//! working buffer (passthrough present) for comparison against the source.

use crate::raw::ImageKind;

#[derive(Debug, Clone)]
pub struct RgbStats {
    pub n: usize,
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub mean: [f32; 3],
    /// Any-channel ≥ `hi` (default 0.995 linear or 254/255 encoded).
    pub clip_hi_pct: f32,
    /// All-channel ≤ `lo`.
    pub clip_lo_pct: f32,
}

impl RgbStats {
    pub fn from_rgb(rgb: &[f32], hi: f32, lo: f32) -> Self {
        let n = rgb.len() / 3;
        if n == 0 {
            return Self {
                n: 0,
                min: [0.0; 3],
                max: [0.0; 3],
                mean: [0.0; 3],
                clip_hi_pct: 0.0,
                clip_lo_pct: 0.0,
            };
        }
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        let mut acc = [0.0f64; 3];
        let mut hi_n = 0u64;
        let mut lo_n = 0u64;
        for px in rgb.chunks_exact(3) {
            for c in 0..3 {
                min[c] = min[c].min(px[c]);
                max[c] = max[c].max(px[c]);
                acc[c] += px[c] as f64;
            }
            if px[0] >= hi || px[1] >= hi || px[2] >= hi {
                hi_n += 1;
            }
            if px[0] <= lo && px[1] <= lo && px[2] <= lo {
                lo_n += 1;
            }
        }
        let nf = n as f32;
        Self {
            n,
            min,
            max,
            mean: [
                (acc[0] / n as f64) as f32,
                (acc[1] / n as f64) as f32,
                (acc[2] / n as f64) as f32,
            ],
            clip_hi_pct: 100.0 * hi_n as f32 / nf,
            clip_lo_pct: 100.0 * lo_n as f32 / nf,
        }
    }

    pub fn from_rgb8(rgb: &[u8]) -> Self {
        let lin: Vec<f32> = rgb.iter().map(|v| *v as f32 / 255.0).collect();
        Self::from_rgb(&lin, 254.0 / 255.0, 1.0 / 255.0)
    }
}

pub fn trace_enabled() -> bool {
    matches!(
        std::env::var("MERARAW_PIPELINE_TRACE").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

/// Log one pipeline stage. Cheap no-op unless `MERARAW_PIPELINE_TRACE=1`.
pub fn log_import(
    stage: &str,
    kind: ImageKind,
    format: &str,
    bit_depth: u8,
    color_label: &str,
    linear: bool,
    demosaic: &str,
    camera_profile: Option<&str>,
    stats: &RgbStats,
) {
    if !trace_enabled() {
        return;
    }
    tracing::info!(
        stage,
        kind = ?kind,
        format,
        bit_depth,
        input_color = color_label,
        linear,
        demosaic = if kind.allows_raw_only_stages() {
            demosaic
        } else {
            "SKIPPED"
        },
        camera_profile = camera_profile.unwrap_or(if kind.allows_raw_only_stages() {
            "none"
        } else {
            "SKIPPED"
        }),
        n = stats.n,
        min = ?stats.min,
        max = ?stats.max,
        mean = ?stats.mean,
        clip_hi_pct = stats.clip_hi_pct,
        clip_lo_pct = stats.clip_lo_pct,
        "pipeline-trace"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_stats_count_highlights() {
        let mut rgb = vec![0.2f32; 30]; // 10 pixels
        rgb[0] = 1.0;
        rgb[1] = 1.0;
        rgb[2] = 1.0;
        let s = RgbStats::from_rgb(&rgb, 0.995, 0.004);
        assert_eq!(s.n, 10);
        assert!((s.clip_hi_pct - 10.0).abs() < 0.01);
        assert_eq!(s.clip_lo_pct, 0.0);
    }
}
