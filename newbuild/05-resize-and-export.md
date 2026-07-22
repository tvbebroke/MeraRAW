# Phase 05 — Resize & Export

**Goal:** High-quality, fast output: render full-res, resize correctly, embed the right metadata/ICC, and write the chosen format.

**Dependencies & licenses**
| Dep | Role | License | Verdict |
|---|---|---|---|
| `fast_image_resize` | SIMD resize for export/thumbnails | MIT/Apache | 🟢 embed freely (verify) |
| `image` | encode (from Phase 02) | Apache/MIT | 🟢 embed freely |
| `zune-*` | faster encode where it wins | MIT/Apache/Zlib | 🟢 embed freely (verify) |

---

## Architecture

Export = **render full-res through Phase 04 → resize (if requested) → attach metadata/ICC → encode**. The subtle quality point: **resize in linear light**, then convert to display space, then encode. Resizing in gamma space darkens/haloes edges.

```rust
// crates/export/src/lib.rs
pub struct ExportJob {
    pub doc: Document,
    pub target: ExportTarget,       // format, quality, bit depth
    pub resize: Option<ResizeSpec>, // long-edge px / exact dims + algorithm
    pub color: ColorOutput,         // sRGB / Display-P3 / embed ICC
    pub metadata: MetadataPolicy,   // strip / preserve / preserve-minus-GPS
}
```

---

## Sub-phases

### 05.1 — Full-res render path
- Reuse Phase 04 pipeline at native resolution (not the preview downscale).
- Confirm the full-res result matches the preview (same passes, same params).

### 05.2 — Resize
- Integrate `fast_image_resize`: feed source buffer + target dims + algorithm (Lanczos3 default, Bilinear for speed).
- **Do the resize in linear light.** Verify downscaled detail isn't darkened (the classic gamma-resize bug).

### 05.3 — Metadata & ICC on export
- Embed the output ICC profile (sRGB/Display-P3) so other apps render correctly.
- Apply `MetadataPolicy`: preserve EXIF (Phase 11), optionally strip GPS/personal fields, write copyright/software tags.

### 05.4 — Format targets & options
- JPEG (quality, chroma subsampling), PNG (bit depth), TIFF (16-bit, compression), WebP (quality/lossless). Optionally AVIF/JXL later.
- Benchmark encode speed `image` vs `zune`; route to the faster per format.

### 05.5 — Batch export
- Queue multiple docs; parallelize with a worker pool (`rayon` or async tasks). Progress reporting to the frontend.

---

## Evaluate against your editor / RapidRAW
- **Resize quality:** does yours resize in linear light with a good filter? Compare a 4000px→1000px downscale against `fast_image_resize` Lanczos3 for sharpness/halos.
- **Export speed:** time a full-res export end-to-end vs your current path.
- **Metadata fidelity:** does your export preserve EXIF/ICC, or silently drop it? Dropping ICC is a common "why do my colors look wrong elsewhere" bug.

## Testing & acceptance criteria
- [ ] Full-res export matches preview (scaled) — no surprise differences.
- [ ] Downscale is linear-light correct (verified against a gradient/detail chart).
- [ ] Exported file carries the intended ICC; opens with correct colors in a browser and a second app.
- [ ] EXIF preserved per policy; GPS strip option actually removes GPS.
- [ ] Batch export of N files completes with correct per-file progress and no memory blow-up.

## Risks / gotchas
- **Gamma-space resize** — the single most common export quality bug. Resize linear.
- **Missing ICC on export** makes wide-gamut edits look wrong everywhere else.
- Peak memory on batch: stream/limit concurrency so you don't hold many full-res float buffers at once.
