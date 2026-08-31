# Meratech RAW Editor

Built from the design package at `~/Desktop/meratech-design-package/` (all 8 phases). Tauri 2 + Rust core + native wgpu (Metal) + React/TS.

## Status — phases P0–P7 built + tested

| Phase | What | Verified by |
|---|---|---|
| P0 | Tauri shell, engine actor, `frame://` transport | headless engine tests + in-app frame self-report |
| P1 | RAW decode → linear Rec.2020 scene-referred working texture | decode of real ARW/DNG, pinned color-regression test |
| P2 | EditDoc, param registry, ops + guard-wall, GPU render graph (cache-the-chain) | 60+ unit tests, GPU cache/identity tests, in-app selftest |
| P3 | 8 develop modules in fixed scene-referred order (Oklab grade/HSL, film-like curve) | GPU hue-preservation/band-isolation tests, skin eyeball on graduation ARW |
| P4 | Masks: radial/linear/brush + on-device u2netp subject segmentation (tract), joint-bilateral edge refine, scoped stacks | GPU mask tests, in-app inference (~0.7s release) |
| P5 | Catalog: disposable SQLite index, sidecar truth, import + thumbs + phash/blur culling | rebuild-from-DB-delete test, in-app import flow |
| P6 | Claude assistant: registry-derived tools, guard-wall reuse, look-again loop | mock-executor selftest (live API needs `ANTHROPIC_API_KEY`) |
| P7 | Tiled full-res export, output transforms (sRGB/P3/AdobeRGB + ICC embed), presets, perf instrumentation | export_check example, all-phase selftest |

## Run

```bash
npm install
npx tauri dev                      # dev app
cd src-tauri && cargo test -p meratech-core   # headless suite (~70 tests)
```

Self-test rig (drives every phase through the real UI command path; grep the log for `selftest-pass`):

```bash
MERATECH_DATA_DIR=/tmp/meratech-data \
MERATECH_ASSISTANT_MOCK=1 \
MERATECH_OPEN="/path/to/file.ARW" \
MERATECH_SELFTEST=1 npx tauri dev
```

Headless visual checks (write PNGs/JPEGs to /tmp):

```bash
cd src-tauri
cargo run -p meratech-core --release --example decode_check  -- <raw>   # P1 color
cargo run -p meratech-core --release --example render_check  -- <raw>   # P3 full edit
cargo run -p meratech-core --release --example mask_check    -- <raw>   # P4 subject mask
cargo run -p meratech-core --release --example export_check  -- <raw>   # P7 ICC export
```

Diagnostic-only examples (import probe, DCP index, etc.): see [docs/dev-tools.md](docs/dev-tools.md).

Assistant: set `ANTHROPIC_API_KEY` (and optionally `ANTHROPIC_MODEL`) — or `MERATECH_ASSISTANT_MOCK=1` for the offline canned loop.

## Architecture spine (the four frozen contracts)

- **EditDoc** (`core/src/doc.rs`) — versioned state document, default-omitting, unknown-field-preserving; sidecar `<name>.mrt.json` is canonical.
- **Param registry** (`core/src/registry.rs`) — single source of param paths/ranges/defaults; drives guard-wall, UI generation, serialization, assistant tool schemas.
- **Ops + guard-wall** (`core/src/ops.rs`) — the ONLY write path to the doc; UI and Claude use identical ops; undo = doc-state capture.
- **Working texture** — linear Rec.2020 scene-referred RGBA16F, orientation baked, headroom preserved.

Render graph (`core/src/graph/`): extract (view region @ viewport res) → exposure → white_balance → calibration → noise → color_grade → hsl → tone_curve → sharpen → mask stage (scoped stacks + blend) → present (pinned neutral view transform). Cache-the-chain re-runs only from the first dirty stage. Export runs the same chain tiled at full res, applies bundled **DCP camera look** per tile, then the delivery transform.

Sidecar priority: `<name>.mrt.json` (canonical) → Adobe `<name>.xmp` (basic Lightroom sliders) → fresh doc.

## Pinned deviations from the specs (all noted at decision time)

- Decoder = pure-Rust **rawler** (LibRaw licensing/FFI risk R1); `Decoder` trait keeps the swap contained. EXIF-orientation fallback added.
- Perceptual space = **Oklab**, not Yrg/JzAzBz (reference §9 forbids from-memory constants; same constant-hue class, constants verified).
- Sky segmentation = on-device U²-Net skyseg (MIT, xiongzhu666); object-by-point = region-grow.
- Undo via doc snapshots (docs ~KB) instead of op inverses.
- Catalog search = indexed LIKE (FTS5 swap-in ready); collections deferred.
- ICC embed implemented for JPEG (system ColorSync profiles); PNG/TIFF written in-space without embedded profile.
- Adobe XMP sidecar import = basic develop sliders only (exposure, WB, tone, sharpen); full Lightroom stacks not supported.
- Full-res settle render: interactive chain runs at viewport res (per-pixel modules are resolution-exact; spatial noise/sharpen approximate at fit zoom — exact at 1:1 and in export).
