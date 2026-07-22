# Photo Editor — Master Buildout Roadmap

**Source:** derived from `rust-photo-editor-repo-deep-dive.md`.
**Target stack:** Rust + Tauri (v2) + web frontend, GPU pipeline via WGPU/WGSL.
**Context:** closed, for-profit product. Every phase respects the legal tiers below.

> ⚠️ **Not legal advice.** License data is carried from the deep dive; re-verify with `cargo tree` and each crate's crates.io page before release. Consult an IP attorney for anything in the 🟡/🔴/💰 tiers.

---

## The legal frame (applies to every phase)

- **Learn** (read code, reimplement yourself) — safe for *any* repo, including AGPL. This is your default.
- **Embed** (link the crate) — governed by license tier.
- **Copy** (paste source) — governed strictly by that file's license; no-license repo = all rights reserved.

**Tiers:** 🟢 permissive = embed freely · 🟡 LGPL = embed with linking obligations · 🔴 GPL/AGPL = do not embed in closed software · 💰 = legal only if you buy a commercial license.

---

## Phase sequence & dependencies

Build in this order; each phase assumes the ones above it exist. Phases marked ⟂ can be built in parallel once the core (01–04) is done.

| # | Phase | Core deps | License | Blocking? |
|---|---|---|---|---|
| 01 | Application Shell & Non-Destructive Core | Tauri, serde | 🟢 | Foundation |
| 02 | Codec Layer (decode/encode) | image, zune | 🟢 | Foundation |
| 03 | RAW Decoding Pipeline | rawloader/rawler *or* own *or* zenraw💰 | 🟡/💰 | Core (RAW editors) |
| 04 | GPU Processing Pipeline & Color Mgmt | wgpu, own WGSL | 🟢 | Core |
| 05 | Resize & Export ⟂ | fast_image_resize, image | 🟢 | After 02/04 |
| 06 | Geometry: Crop/Straighten/Perspective ⟂ | kornia-rs, own | 🟢 | After 04 |
| 07 | Filters & Effects ⟂ | own (photon as ref) | 🟢 | After 04 |
| 08 | Denoise ⟂ | own WGSL / ONNX | 🟢 | After 04 |
| 09 | Masking & AI Subject Detection ⟂ | ort (ONNX), SAM 2 model | 🟢 | After 04 |
| 10 | Inpainting / Generative Replace ⟂ | ort (ONNX), LaMa model | 🟢/⚠️ | After 09 |
| 11 | Import, Metadata & Interop ⟂ | kamadak-exif, parsers | 🟢/⚠️ | After 02 |
| 12 | Histogram & HUD ⟂ | frontend canvas | 🟢 | After 04 |

---

## Suggested repo/module layout (monorepo, multi-crate)

```
photo-editor/
├─ crates/
│  ├─ core/          # image buffers, pixel formats, color types (shared)
│  ├─ codec/         # phase 02: decode/encode
│  ├─ raw/           # phase 03: RAW decode + demosaic
│  ├─ pipeline/      # phase 04: GPU pipeline, color mgmt, stage graph
│  ├─ geometry/      # phase 06: crop/straighten/perspective
│  ├─ effects/       # phase 07: filters
│  ├─ denoise/       # phase 08
│  ├─ ml/            # phases 09/10: ONNX runtime + masking + inpainting
│  ├─ metadata/      # phase 11: exif/icc/interop
│  └─ export/        # phase 05: resize + write
├─ src-tauri/        # phase 01: Tauri backend, command layer
└─ frontend/         # phase 01/12: UI, histogram, HUD
```

Keep each crate as a **separate library with a clean public API** even inside the monorepo — this is the "monorepo, multi-library" sweet spot and it also makes it trivial to quarantine any 🟡 LGPL crate behind a boundary if you choose to embed one.

---

## How to use these files

Each phase file is self-contained and agent-executable: goal, dependency + license verdict, architecture, numbered sub-phases with tasks, key Rust interfaces, an **evaluate-against-your-editor** hook, testing/acceptance criteria, and risks. Hand any single file to a coding agent and it should be able to execute the phase.

**Global definition of done:** all acceptance criteria met per phase + `cargo tree` license audit clean + attribution screen lists required MIT/Apache/BSD notices.
