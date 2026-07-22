# Phase 10 — Inpainting / Generative Replace (Object Removal & Content-Aware Fill)

**Goal:** Remove objects, dust, and blemishes, and optionally do generative replace — by painting a region and filling it plausibly.

**Dependencies & licenses**
| Dep | Role | License | Verdict |
|---|---|---|---|
| `ort` (ONNX Runtime) | run inpainting models locally | MIT/Apache | 🟢 embed freely (verify) |
| **LaMa** | inpainting model (mask → filled) | code Apache-2.0 | ⚠️ **verify model weights** for closed redistribution |
| your code | mask UX, tiling, blending | yours | 🟢 |

**Reference (study only):** RapidRAW uses **LaMa** for content-aware fill / object removal; TheUnduster uses LaMa via **ONNX** for dust/scratch removal — the best reference for running LaMa locally in a Tauri/Rust app.

---

## Architecture

Inpainting = **user paints a mask (reuse Phase 09 brush) → model fills the masked region → blend result back**. Runs as an explicit, cancelable job (not live). Optionally add a **local ComfyUI/cloud backend** for generative replace (RapidRAW offers this as an optional service) — keep it strictly opt-in and off by default for a local-first product.

```rust
// crates/ml/src/inpaint.rs
pub struct InpaintJob { pub image: CoreImage, pub mask: MaskTexture, pub backend: InpaintBackend }
pub enum InpaintBackend { LocalLaMa(Session), LocalComfyUi(Url), /* Cloud(...) opt-in */ }
```

---

## Sub-phases

### 10.1 — Removal mask UX
- Reuse the Phase 09 brush to paint removal regions; show a live overlay. Support multiple regions per edit.

### 10.2 — Local LaMa inpainting (ONNX)
- Integrate `ort`; run LaMa on (image, mask). Handle large images via **tiling** around the masked region (don't run the whole image).
- Blend the filled patch back with feathered edges to avoid seams.

### 10.3 — Dust & scratch removal (auto)
- Optional: detect small defects automatically (heuristic or a small model, à la TheUnduster) and batch-remove. Great for film scans.

### 10.4 — Generative replace (optional, opt-in)
- Optional local ComfyUI backend for prompt-driven replace. Off by default; clearly separated from core (local-first) features. Consider the licensing/privacy implications of any cloud path.

### 10.5 — Non-destructive integration
- Store inpaint regions + results as an `EditOp` so removal is reversible and re-editable (Phase 01).

---

## Evaluate against your editor / RapidRAW
- Do you have object removal / content-aware fill? LaMa via ONNX is the same route RapidRAW/TheUnduster take.
- Is removal **non-destructive** (re-editable) or baked in? Non-destructive is the upgrade.
- Dust/scratch auto-removal for film scans — a differentiator if your audience shoots film.

## Testing & acceptance criteria
- [ ] Painting over an object and running fill removes it with a plausible, seamless result.
- [ ] Large-image inpaint uses tiling (verify it doesn't process/allocate the full image).
- [ ] Filled region blends without a visible patch boundary at 100%.
- [ ] Removal is stored as a reversible EditOp and survives save/reload.
- [ ] Job is cancelable and reports progress.

## Risks / gotchas
- **⚠️ Model weights license is the real gate** — LaMa code is Apache, but confirm the *checkpoint* you ship can be redistributed in a closed commercial product. This is the most likely legal snag in the whole build.
- Full-image inpainting is slow/memory-heavy — always tile around the mask.
- Any cloud generative path introduces privacy + licensing questions; keep it opt-in and disclosed.
