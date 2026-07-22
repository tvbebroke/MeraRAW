# Phase 08 — Denoise

**Goal:** Luminance and color noise reduction that preserves detail — via GPU shader, ML model, or both. This is a likely **differentiation** point (no mature permissive Rust crate exists, so everyone rolls their own).

**Dependencies & licenses**
| Dep | Role | License | Verdict |
|---|---|---|---|
| your WGSL | shader denoise | yours | 🟢 |
| `ort` (ONNX Runtime) | run ML denoise models | MIT/Apache | 🟢 embed freely (verify) |
| a denoise model | the weights | model-specific | ⚠️ check weights license |

**The honest gap:** `riccio8/DenoiseRs` is early; `sentinel1_denoise_rs` is satellite-specific. Neither is a drop-in. **How the good apps do it:** RapidRAW uses a **WGSL shader**; TheUnduster uses **ONNX models**. Study those approaches (don't copy AGPL code), implement your own.

---

## Architecture

Offer a **tiered** approach:
- **Fast (shader):** bilateral / guided / wavelet denoise as a pipeline pass — real-time, good for moderate noise.
- **Quality (ML):** an ONNX denoise model run via `ort` — slower, best for high-ISO. Run on demand (not per-slider).

Denoise belongs **early** in the pipeline (right after demosaic/linearization), before sharpening. Sharpening before denoise amplifies noise.

```rust
// crates/denoise/src/lib.rs
pub enum DenoiseMode { Shader(ShaderParams), Ml(MlParams) }
pub struct DenoiseParams { pub luminance: f32, pub color: f32, pub detail: f32, pub mode: DenoiseMode }
```

---

## Sub-phases

### 08.1 — Shader denoise (baseline)
- Implement a separable bilateral or guided filter as a WGSL pass with luminance/color/detail controls.
- Split luma vs chroma denoise (chroma noise tolerates stronger smoothing).

### 08.2 — Detail preservation
- Edge-aware weighting so flat areas smooth while edges stay crisp. Expose a detail/threshold control.

### 08.3 — ML denoise (quality tier)
- Integrate `ort`; run a denoise model (pick one with permissive **weights** — verify).
- Run as an explicit, cancelable job (not live) given cost; cache the result.

### 08.4 — Pipeline placement & interaction
- Place denoise before sharpening/clarity. Verify the two interact sanely (denoise then sharpen).

---

## Evaluate against your editor / RapidRAW
- Does your denoise preserve edges or smear detail? Compare on a high-ISO test shot vs a shader-based reference.
- Do you separate luma/chroma denoise? Chroma-only denoise fixes color blotches without softening.
- Is there an ML "quality" tier? That's where premium editors differentiate.

## Testing & acceptance criteria
- [ ] High-ISO test image: noise visibly reduced with edges/text still legible at 100%.
- [ ] Luma and color denoise are independently controllable.
- [ ] Shader denoise runs in the live preview budget; ML denoise runs as a cancelable job.
- [ ] Denoise sits before sharpening; enabling both doesn't re-amplify noise.
- [ ] Zero denoise = identity (no change when sliders at 0).

## Risks / gotchas
- **Model weights license** ≠ code license — verify you can ship the weights in a closed product.
- Over-smoothing = "plastic" look; always expose a detail control and test at 100%.
- ML denoise memory/latency on large images — tile if needed.
