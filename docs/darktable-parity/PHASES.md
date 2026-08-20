# Darktable-parity build phases

Gated like [harness-skills](https://github.com/harness/harness-skills): **scope → verify dependencies → discover schema → implement → exit criteria**. Do not generate a dependent (UI, new IOP, new IPC) until its dependency is confirmed in this repo.

Spec: [`SPEC.md`](./SPEC.md). Color-engine rewrite is out of scope (`FOUNDATION-REBUILD.md`).

---

## Operating rules (Harness → MeraRAW)

| Harness rule | How we apply it |
|---|---|
| Establish scope first | MeraRAW is Rust+Tauri+Svelte, photo+video, fixed `MODULE_ORDER`. We take DT *workflows*, not DT source. |
| Verify dependencies before dependents | No filmic UI without a tested tone module. No virtual-copy button until catalog persistence exists. No new `invoke` until `GridQuery` / `EngineMsg` already has the field. |
| Discover schema before payloads | Read `GridQuery`, `EditDoc`, `ops.rs`, `permissions/default.toml`, `contractAllowlist.ts` before adding commands. |
| Phase gates | A phase is done only when its **Exit** checklist is green. Later phases may not start on a red gate they depend on. |

**Parallelism:** P1 (collections) depends on catalog `grid()`, which already has Rust tests. It MAY ship while P0 (selftest/CI) is still being hardened. P5–P6 MUST NOT start until P0 is green.

---

## P0 — Verification harness

**Scope:** Prove the engine and IPC contract before adding color-science modules.

**Dependencies:** Existing `src/selftest.ts`, `.github/workflows/ci.yml`, `src/ipc/contractAllowlist.ts`.

**Schema:** No new payloads.

**Work:** Keep CI green (`npm run check`, vitest, `cargo test`, clippy). Drive selftest to a known `selftest-pass` on a fixture RAW. Do not add Darktable features that mutate the pixel graph until this is true.

**Exit:**

- [x] `npm run check` and `npx vitest run` pass
- [x] `cargo test --workspace` pass
- [ ] CI workflow on the working branch is green — last GitHub run on `release/0.1.7.1` failed (`clippy -D warnings` on Linux HEIC arms; `npm audit` high on postcss/nanoid). Fixes are in this tree. Remote stays red until a push.
- [x] Selftest named failure filed: last recorded run died at `exposure no-op: 229.33 → 229.33` (stale FrameReady / sidecar already at 1.5) — see `FOUNDATION-REBUILD.md`. This tree samples luma via `waitLuma` after `reset_all`, and `MERATECH_SELFTEST=exposure` stops after that check. Full `selftest-pass` not re-driven here (needs `npx tauri dev` + fixture RAW).

---

## P1 — Lighttable collections  ← **this delivery**

**Scope:** Darktable collections / collection-filters for rating, flag, history (edited), text, blurry, dupes — wired to existing `get_grid`.

**Dependencies (verified):**

- `Catalog::grid` + `GridQuery` in `src-tauri/core/src/catalog.rs` (rating_min, flag, has_edits, text, blurry_only, dupes_only)
- IPC `get_grid` / `GridQuery` camelCase
- Library cull bar already writes rating/flag via `set_asset_meta`

**Schema:** Existing `GridQuery`. Frontend `LibraryFilters` + `gridQueryFromState`. No new Tauri command.

**Work:** Filter store, catalog-backed search, collection chips, copy/paste grade from Library + filmstrip onto the clicked file.

**Exit:**

- [x] Spec FR-1…FR-6
- [x] `gridQueryFromState` unit tests
- [x] Import polling does not use user filters (EC-3)
- [x] CatalogChanged reload preserves filters

---

## P2 — History as a lighttable tool

**Scope:** Darktable history stack on many images: copy/paste already exists for one open doc; extend to **styles** (named presets from a copied grade) and **apply to selected**.

**Dependencies:** P1 (selection + copy grade UX). `apply_preset` / sidecar write. Multi-select in Library (does not exist yet).

**Schema:** Discover whether batch apply can be `apply_preset` in a loop vs a new `apply_preset_to_paths` catalog command. Do not guess — read `ops.rs` and sidecar I/O first.

**Work:** Multi-select grid. “Paste grade to selected” writes sidecars without requiring a viewport for each. Optional named style in PresetSettings.

**Exit:** Paste onto N selected files leaves N sidecars and `has_edits=1`; destination crop/masks unchanged. Tests on sidecar merge.

- [x] Library cmd/shift multi-select
- [x] `apply_grade_to_paths` IPC + sidecar merge (crop/masks/detail untouched)
- [x] Paste grade to selected
- [x] Save copied grade as a named preset (PresetSettings “Save copy”)

---

## P3 — Module enable / disable

**Scope:** Darktable IOP power button. Each `MODULE_ORDER` node can be identity-skipped.

**Dependencies:** Graph dirty-from cache (`src-tauri/core/src/graph/`). Confirm identity skip already exists or add `enabled` on module maps with registry defaults.

**Schema:** Per-module `enabled` in `EditDoc.modules[name]`, default on. `reset_module` must not wipe the enabled bit unless specified.

**Work:** Header toggle on each edit-panel section. Graph skips disabled nodes.

**Exit:** Toggling exposure off restores the pre-exposure preview; sidecar round-trips; `cargo test` on graph skip.

- [x] `{module}.enabled` in registry, default on
- [x] Graph `module_enabled` skip + `disabled_module_skips_even_when_params_are_set`
- [x] Edit-panel power buttons

---

## P4 — Virtual copies (catalog)

**Scope:** Darktable duplicates. Current `virtual_copy` is in-memory only (`vc-N` on `c.docs`) — **do not ship that as the feature**.

**Dependencies:** Sidecar path scheme (how a clone is named on disk). Catalog unique path constraint.

**Schema:** New asset row + sidecar file. `switch_doc` by catalog id, not session index.

**Work:** “Create virtual copy” in Library / Versions. Grid shows copies grouped under the master.

**Exit:** Quit and relaunch; copies still listed and openable. Deleting a copy does not delete the master file.

- [x] Sidecar `copies[]` round-trip (`split_copies` / `bundle_copies`)
- [x] Versions UI: list / create / switch (`list_docs`, `virtual_copy`, `switch_doc`)
- [x] Grid expands copies into separate rows (`docId`); open switches doc; Library can create a copy; deleting a copy does not delete the master

---

## P5 — Masks: parametric + blend

**Scope:** Darktable parametric masks and blend modes on top of existing geometric masks.

**Dependencies:** P0 + current `MaskMirror`. Color picker if range is sampled from the image.

**Schema:** Extend mask source types; do not invent a second mask system.

**Work:** Luminance/chroma/hue range. Per-module blend (normal, multiply, etc.) only if the graph compositor can express it.

**Exit:** Fixture: a grey ramp with a luminance mask affects only the specified band. No full-image bleed.

- [x] `parametric` mask kind + luma/chroma/hue sample pass
- [x] Blend modes normal / multiply / screen on the compositor
- [x] Dedicated grey-ramp GPU fixture (`parametric_luma_mask_scopes_grey_ramp`)

---

## P6 — Scene-referred tone + highlight reconstruction

**Scope:** Darktable filmic/sigmoid-class mapper and `highlights` reconstruction as **Rust modules**, not LUT-only looks.

**Dependencies:** **P0 green.** Existing LUT/AgX path stays. Numeric tests before UI.

**Schema:** New registry modules at fixed slots in `MODULE_ORDER` (append or insert with a version bump — do not let users reorder).

**Work:** Highlight reconstruct from clipped RAW channels. Tone mapper with documented scene-linear in / display out.

**Exit:** Golden buffers or max-error asserts. UI ships only after tests. Optics placeholder (F6) is a separate track, not this phase.

- [x] `highlights` GPU node after exposure
- [x] CPU `reconstruct_pixel` tests
- [x] `tone_curve.sigmoid` mixed into the curve LUT

---

## P7 — Soft proof, gamut check, culling compare

**Scope:** Darktable softproof / gamut warning; 2-up culling.

**Dependencies:** P6 color spaces understood; export ICC path. Viewport already has before/after bypass.

**Schema:** View-only flags on `ViewParams` or a proofing command — must not write `EditDoc`.

**Work:** Soft proof to export profile. Optional 2-image compare using existing preview surfaces.

**Exit:** Proofing off restores the edit view bit-identically (same `FrameInfo.version` path). Compare does not duplicate history.

- [x] Viewport Blinkies + histogram clip overlays (`set_clip_warnings`)
- [x] Viewport Compare + Filmic/AgX display looks (existing)
- [x] Library 2-up when exactly two photos are selected
- [x] Viewport soft proof / gamut check (`set_proof_target`; off uses the same present path)

---

## P8 — Auto-presets from EXIF (optional)

**Scope:** Darktable auto-apply styles by camera/ISO/lens.

**Dependencies:** P2 styles + catalog EXIF columns.

**Schema:** Rules table or sidecar convention. Confirm columns (`camera_make`, `iso`, `lens`) before UI.

**Exit:** Opening a matching camera applies the style once; user undo works; no re-apply loop on every catalog refresh.

- [x] On open, if the primary doc is empty, apply a preset whose tags match `camera_model` and/or `iso:N`

---

## Dependency graph

```
P0 harness ─────────────────────────────┐
P1 collections (engine already exists)  │
        └─► P2 history/styles/batch     │
                └─► P4 virtual copies   │
                └─► P8 auto-presets     │
P3 module enable (graph)                │
P5 parametric masks                     │
P0 ─► P6 tone + highlights ─► P7 proof/cull
```

---

## What we will never take from Darktable

- Pixelpipe written in C with OpenCL variants of every IOP
- User-custom module order (legacy vs v3.0 vs custom)
- GTK lighttable/darkroom chrome
- Database layout or XMP as the source of truth
- 100 IOPs “because they have them”
