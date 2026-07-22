# Phase 01 — Application Shell & Non-Destructive Editing Core

**Goal:** A Tauri app skeleton with a command layer between frontend and Rust, and the non-destructive edit model (an edit is a stack of parameters, never a mutation of pixels) that every later phase plugs into.

**Dependencies & licenses**
| Dep | Role | License | Verdict |
|---|---|---|---|
| `tauri` (v2) | app shell, IPC | Apache/MIT | 🟢 embed |
| `serde` / `serde_json` | edit-state serialization | Apache/MIT | 🟢 embed |
| `uuid` | edit/layer IDs | Apache/MIT | 🟢 embed |

**Reference (study only):** RapidRAW's non-destructive model + sidecar approach (AGPL — read, don't copy).

---

## Architecture

The whole product hinges on this idea: **the document is a description of edits, not a bitmap.** The pixel output is derived on demand by running the edit stack through the pipeline (Phase 04). This is what makes edits reversible, history free, and presets possible.

```rust
// crates/core/src/edit.rs
pub struct Document {
    pub id: Uuid,
    pub source: SourceRef,          // path + hash of the original file
    pub edits: EditStack,           // ordered, serializable parameters
    pub masks: Vec<Mask>,           // phase 09
    pub metadata: DocMetadata,      // phase 11
}

pub struct EditStack {
    pub global: GlobalAdjustments,  // exposure, contrast, wb, tone curve...
    pub ops: Vec<EditOp>,           // crop, denoise, effects, inpaint regions...
}

pub enum EditOp { Crop(CropParams), Denoise(DenoiseParams), Effect(EffectId, Params), /* ... */ }
```

**Persistence:** serialize `Document` to a **sidecar** file (`.editproj` JSON, or XMP-style) next to the original. Never write back to the source. This mirrors how Lightroom/darktable/RapidRAW keep originals pristine.

---

## Sub-phases

### 01.1 — Tauri skeleton
- Scaffold Tauri v2 project; pick frontend framework (React/Svelte — match what you have).
- Wire a single round-trip `invoke` command as a smoke test.
- Set up dev + release builds for your target OSes.

### 01.2 — Command layer contract
- Define the typed command surface between frontend and Rust (open file, request preview, apply edit, export).
- Standardize on `Result<T, AppError>` with a serializable error enum.
- Decide the preview transport: base64 in IPC (simple) vs a local asset/stream (faster for large images). Prototype both, measure.

### 01.3 — Non-destructive document model
- Implement `Document`, `EditStack`, `EditOp` in `crates/core`.
- Make every op `serde`-serializable and **versioned** (add a `schema_version` field now — you will change the format).
- Implement load/save of the sidecar.

### 01.4 — History (undo/redo)
- Model history as a stack of `EditStack` snapshots or a command/inverse-command log.
- Snapshots are cheap because they're *parameters*, not pixels — favor them for simplicity unless memory says otherwise.

### 01.5 — Presets
- A preset is a serialized `GlobalAdjustments` (+ selected ops) with the source-specific bits stripped.
- Implement export/import of presets; this falls out almost free from 01.3.

---

## Evaluate against your editor / RapidRAW
- Do you already have a parameter-based (non-destructive) model, or do you mutate pixels? If you mutate, this phase is the single biggest architectural upgrade available.
- Compare your sidecar/project format's forward-compatibility story against RapidRAW's.
- Benchmark preview round-trip latency (edit slider → updated preview) — this is the #1 felt-performance metric.

## Testing & acceptance criteria
- [ ] Open an image, apply a parameter change, see a preview update — round trip < 1 frame budget for a downscaled preview.
- [ ] Undo/redo across 50+ operations with no pixel drift (deterministic re-render).
- [ ] Save → close → reopen restores the exact edit state from sidecar.
- [ ] Schema version present; loading an older version doesn't crash (migration hook exists).
- [ ] Original file byte-for-byte untouched after a full edit+save cycle.

## Risks / gotchas
- **Format churn:** you *will* change the sidecar schema. Versioning from day one avoids a painful migration later.
- **IPC image transport:** shipping full-res buffers over IPC per keystroke will feel sluggish — always preview at display resolution, render full-res only on export.
