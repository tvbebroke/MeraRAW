//! Color validation harness seed (test strategy §2). Pinned-commit
//! regression: a known ARW must land within frozen stat bands. Drift =
//! silent color bug → red build. Skipped when the fixture is absent.
//! Slow (full decode) → run with: cargo test --release -- --ignored

use meratech_core::raw::{Decoder, RawlerDecoder};
use std::path::Path;

const FIXTURE: &str = "/Users/kaimai/Desktop/test-claude-raw/DSC07585.ARW";

/// Frozen 2026-06-12 from the first verified render (graduation portrait,
/// open shade). Mean linear-Rec.2020 per channel. Re-bless deliberately.
const PINNED_MEAN: [f32; 3] = [0.12857, 0.12813, 0.12513];
const MEAN_TOL_REL: f32 = 0.05; // ±5%

#[test]
#[ignore = "needs local fixture + full decode; run --ignored --release"]
fn pinned_arw_color_regression() {
    if !Path::new(FIXTURE).exists() {
        eprintln!("fixture missing, skipping");
        return;
    }
    let dec = RawlerDecoder::default();
    let img = dec.decode(Path::new(FIXTURE)).expect("decode");

    assert_eq!(
        (img.working.width, img.working.height),
        (4000, 6000),
        "dims changed — orientation regression?"
    );
    assert_eq!(img.meta.orientation, "Rotate270");

    let n = (img.working.width * img.working.height) as f64;
    let mut mean = [0.0f64; 3];
    let mut negatives = 0usize;
    let mut over1 = 0usize;
    for px in img.working.data.chunks_exact(3) {
        for c in 0..3 {
            mean[c] += px[c] as f64;
        }
        if px.iter().any(|v| *v < 0.0) {
            negatives += 1;
        }
        if px.iter().any(|v| *v > 1.0) {
            over1 += 1;
        }
    }
    let mean = mean.map(|v| (v / n) as f32);

    for c in 0..3 {
        let rel = (mean[c] - PINNED_MEAN[c]).abs() / PINNED_MEAN[c];
        assert!(
            rel < MEAN_TOL_REL,
            "channel {c} drifted: got {} want {} (rel {rel:.4})",
            mean[c],
            PINNED_MEAN[c]
        );
    }
    assert_eq!(negatives, 0, "negative values leaked into working buffer");
    assert!(over1 > 0, "headroom lost — highlights clipped at decode");
    // neutral-scene cast check (this frame is near-neutral overall)
    let rg = mean[0] / mean[1];
    let bg = mean[2] / mean[1];
    assert!((0.95..=1.06).contains(&rg), "R/G cast: {rg}");
    assert!((0.92..=1.04).contains(&bg), "B/G cast: {bg}");
}
