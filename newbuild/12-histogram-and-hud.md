# Phase 12 — Histogram & HUD

**Goal:** Real-time histogram, clipping warnings, and on-canvas HUD overlays (info, focus/exposure aids, before/after). In a Tauri app this is mostly **frontend**, fed by data computed in Rust.

**Dependencies & licenses**
| Dep | Role | License | Verdict |
|---|---|---|---|
| your Rust (compute) | histogram/stats from the rendered image | yours | 🟢 |
| frontend canvas / charting | draw the histogram + overlays | your choice (MIT-ish) | 🟢 |

**Reality check:** there's **no Rust "HUD/histogram" crate** worth embedding — in Tauri apps this lives in the web layer. Compute stats in Rust (fast, has the pixels), draw with canvas/WebGL/a chart lib in the frontend. (If you were doing a native-Rust GUI like `egui` instead of Tauri, it has histogram widgets — but that's a different architecture.)

---

## Architecture

Rust computes a compact stats payload from the **rendered preview** (post-pipeline, display space) each render; the frontend draws it. Keep the payload small (256-bin × channels) so IPC stays cheap.

```rust
// crates/pipeline/src/stats.rs
pub struct FrameStats {
    pub hist_r: [u32; 256], pub hist_g: [u32; 256], pub hist_b: [u32; 256], pub hist_luma: [u32; 256],
    pub clip_shadows: f32, pub clip_highlights: f32, // fraction of pixels clipped
}
pub fn compute_stats(rendered: &GpuImage) -> FrameStats { /* reduce on GPU or CPU */ }
```

Prefer a **GPU reduction** for the histogram (compute shader) so it's free-ish per frame; fall back to CPU for small previews.

---

## Sub-phases

### 12.1 — Stats compute
- Compute per-channel + luma histograms and clipping fractions from the rendered preview. GPU compute-shader reduction preferred; CPU acceptable at preview resolution.

### 12.2 — Histogram UI
- Draw RGB + luma histogram in the frontend (canvas). Live-update as edits change. Log/linear toggle.

### 12.3 — Clipping warnings
- Highlight/shadow clipping overlays on the canvas (blinkies), driven by `clip_*` values + a per-pixel clip mask.

### 12.4 — Canvas HUD
- Before/after split view, zoom/pan HUD, EXIF/info overlay, optional focus-peaking/exposure aids. Loupe/100% view.

### 12.5 — Performance
- Throttle stats to render cadence; don't recompute on every mouse move. Reuse the rendered texture you already have.

---

## Evaluate against your editor / RapidRAW
- Live histogram + clipping blinkies present and real-time? These are table-stakes pro features.
- Is the histogram computed from the **final rendered** image (correct) or the raw input (misleading)? It must reflect what the user sees.
- Before/after and 100% loupe — common gaps worth checking.

## Testing & acceptance criteria
- [ ] Histogram matches the rendered image (a known gradient produces the expected distribution).
- [ ] Histogram updates live within the preview budget (no lag on slider drag).
- [ ] Clipping overlays correctly mark blown highlights / crushed shadows.
- [ ] Before/after and 100% loupe work without re-decoding.
- [ ] Stats compute doesn't tank frame rate (throttled to render cadence).

## Risks / gotchas
- Computing the histogram from the wrong stage (pre-pipeline) misleads users — always use the final rendered output.
- Per-frame CPU histogram on full-res is wasteful — use the display-res preview or a GPU reduction.
- Keep the IPC payload tiny; don't ship pixel buffers to the frontend just to draw a histogram.
