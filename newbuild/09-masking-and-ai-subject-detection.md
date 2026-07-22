# Phase 09 — Masking & AI Subject Detection

**Goal:** Local adjustments through masks — radial, linear, brush, luminance/color range, and **AI subject/sky selection** — that gate any pipeline pass to a region.

**Dependencies & licenses**
| Dep | Role | License | Verdict |
|---|---|---|---|
| your WGSL | mask generation + application | yours | 🟢 |
| `ort` (ONNX Runtime) | run segmentation models | MIT/Apache | 🟢 embed freely (verify) |
| **SAM 2** (Segment Anything 2) | AI subject detection | **Apache-2.0** | 🟢 model license is permissive-friendly (verify weights) |

**Reference (study only):** RapidRAW uses **SAM 2** for AI subject detection and a multi-mask system (AGPL app — study the UX/architecture, use SAM 2 directly yourself).

---

## Architecture

A **mask is a single-channel float texture** (0..1). Any pass (Phase 04/07/08) takes an optional mask and blends its effect by the mask value. Masks compose (add/subtract/intersect).

```rust
// crates/core/src/mask.rs (referenced by pipeline)
pub enum MaskKind {
    Radial(RadialParams), Linear(LinearParams), Brush(BrushStrokes),
    LumaRange(Range), ColorRange(ColorSel), AiSubject(SubjectSel),
}
pub struct Mask { pub kind: MaskKind, pub invert: bool, pub feather: f32, pub opacity: f32 }
pub struct LocalAdjustment { pub mask: Mask, pub adjustments: GlobalAdjustments }
```

Local adjustments are just `GlobalAdjustments` gated by a `Mask` — so masking **reuses every pass you already built**.

---

## Sub-phases

### 09.1 — Mask primitives
- Radial (ellipse + feather), linear (gradient), brush (with in/out + size/flow/feather). Render each to a mask texture.

### 09.2 — Range masks
- Luminance-range and color-range selection (auto masks). Combine with primitives (e.g., brush ∩ luma range).

### 09.3 — Mask compositing
- Add/subtract/intersect multiple masks per adjustment; invert; global opacity/feather.

### 09.4 — AI subject/sky (SAM 2)
- Integrate `ort`; run **SAM 2** for click-to-select subject and sky/background segmentation.
- Convert model output to a feathered mask texture; let users refine with brush (09.1).

### 09.5 — Local adjustment application
- Wire `LocalAdjustment` into the pipeline: each masked pass blends by mask value. Support multiple local adjustments stacked.

---

## Evaluate against your editor / RapidRAW
- Which mask types do you have vs RapidRAW (radial/linear/brush/range/AI)? List gaps.
- **AI masking:** do you have click-to-select subject / one-tap sky? SAM 2 is Apache-licensed and is the same tool RapidRAW uses — a strong, legally-clean addition.
- Can masks be combined (intersect a brush with a luma range)? That's the pro-grade capability.

## Testing & acceptance criteria
- [ ] Each mask type gates an adjustment to the correct region with smooth feathering.
- [ ] Masks compose (add/subtract/intersect) and invert correctly.
- [ ] SAM 2 subject select produces a usable mask on varied images; brush refine works.
- [ ] Multiple stacked local adjustments render correctly and stay in the preview budget.
- [ ] Masks are non-destructive and serialize into the Document (Phase 01).

## Risks / gotchas
- **Verify SAM 2 weights** license terms for redistribution in a closed app (code is Apache; confirm the checkpoint you ship).
- ONNX model size/load time — lazy-load and cache the session.
- Feathering in linear vs display space affects blend look — be consistent.
- Preview cost with many masks — this is exactly where RapidRAW optimizes; cache mask textures.
