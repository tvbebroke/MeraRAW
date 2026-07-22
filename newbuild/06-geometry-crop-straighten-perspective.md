# Phase 06 — Geometry: Crop / Straighten / Perspective

**Goal:** Non-destructive geometric edits — crop, rotate/straighten, and perspective/keystone correction — expressed as parameters and applied as a GPU warp.

**Dependencies & licenses**
| Dep | Role | License | Verdict |
|---|---|---|---|
| `kornia-rs` | homographies, warping, geometry (optional) | Apache-2.0 | 🟢 embed freely (verify) |
| your WGSL | the actual sampling/warp pass | yours | 🟢 |

**Reference (study only):** `Koz-TV/KozPhotoEditor` is crop-focused (Tauri v2 + React) — good interactive crop UX reference.

---

## Architecture

Geometry is **non-destructive**: store a crop rect, rotation angle, and a perspective transform as parameters; apply them as a single warp pass in the pipeline (Phase 04). Never resample twice — compose rotation + perspective + crop into **one homography** and sample once to avoid cumulative softening.

```rust
// crates/geometry/src/lib.rs
pub struct GeometryOp {
    pub crop: Rect,            // in source coords
    pub angle_deg: f32,        // straighten
    pub perspective: Option<Homography>, // 3x3
    pub aspect_lock: Option<f32>,
}
impl GeometryOp { pub fn to_homography(&self) -> Homography { /* compose */ } }
```

---

## Sub-phases

### 06.1 — Crop (non-destructive)
- Store crop rect + aspect-ratio lock; render the crop as a viewport, not a pixel deletion.
- Interactive handles in the frontend; overlay rule-of-thirds/grid.

### 06.2 — Straighten / rotate
- Arbitrary-angle rotation with a straighten tool (drag a line to define horizontal/vertical).
- Auto-expand or auto-crop to avoid empty corners after rotation.

### 06.3 — Perspective / keystone
- Manual perspective handles (4-corner) → compute a homography.
- Optional: automatic vertical/horizontal correction (detect converging lines). `kornia-rs` provides the homography/warp math if you don't want to write it.

### 06.4 — Single-sample warp pass
- Compose crop + rotation + perspective into one 3×3 matrix; apply as one WGSL sampling pass with a good interpolation kernel (bicubic/Lanczos).
- Verify only **one** resample happens across all geometry ops.

---

## Evaluate against your editor / RapidRAW
- Do you compose geometry into a single resample, or stack them (each adding softness)? Single-sample is the quality win.
- Perspective correction present? If not, `kornia-rs` makes it cheap to add.
- Crop UX: compare handle feel/aspect presets to KozPhotoEditor.

## Testing & acceptance criteria
- [ ] Crop is fully reversible (change/undo restores full frame).
- [ ] Straighten a tilted horizon: a known-tilted test image reads level after correction.
- [ ] Perspective: a photographed rectangle becomes rectangular after 4-corner correction.
- [ ] Combined rotate+perspective+crop resamples **once** (verify no double-softening vs a single-op baseline).
- [ ] Aspect-ratio lock holds exact ratios (16:9, 3:2, 1:1).

## Risks / gotchas
- **Double resampling** softens the image — always compose to one matrix.
- Perspective warps can reveal empty regions; define fill/auto-crop behavior.
- Keep geometry in source coordinates so it survives edits to earlier stages.
