# 07 — Implementation Roadmap

Phased plan with milestones, crates, testing, and acceptance criteria.

---

## Phase 0 — Foundations (1–2 weeks)

- [ ] `denoise` module scaffolding, `DenoiseSettings` in sidecar schema (versioned,
      forward-compatible defaults).
- [ ] Pipeline slots P0–P3 wired into the render graph (no-op passes first).
- [ ] Test-asset kit: raw ISO ladders (same scene, ISO 100→25600) from ≥3 cameras
      (Bayer ×2, X-Trans ×1) — shoot or pull from raw.pixls.us; NIND subset download
      script; synthetic Poisson–Gaussian noise injector for ground-truth tests.
- [ ] Metrics harness (`denoise-bench` bin): PSNR, SSIM, MS-SSIM vs. ground truth;
      timing per pass; runs on the asset kit and outputs a markdown report.

**Exit:** no-op passes cost <0.5 ms; bench harness produces a baseline report.

## Phase 1 — Raw-domain hygiene (1 week)

- [ ] P0 hot/dead pixel compute shader + toggle.
- [ ] P2 conditional impulse median.

**Exit:** acceptance criteria in doc 04 (P0/P2) green; zero regression on clean images
(SSIM ≥ 0.999 with features on, threshold 0).

## Phase 2 — Noise profiling + VST (1–2 weeks)

- [ ] EXIF-seeded heuristic (camera/ISO → rough (a,b) table; start with generic
      per-ISO curve, refine over time).
- [ ] Image-based (a,b) estimation (flat-region variance/mean fit, robust regression)
      on a CPU thumbnail.
- [ ] VST forward + exact-unbiased inverse as generated 1D LUTs; Y0U0V0 conversion
      shaders; round-trip test (identity at strength 0).
- [ ] (Stretch) darktable noise-profile import for cameras present in their DB.

**Exit:** estimated σ within ±20% of measured σ on the ISO ladder; round-trip ≤ 1e-4.

## Phase 3 — Wavelet engine (2–3 weeks) ← first user-visible release

- [ ] À-trous decompose/shrink/reconstruct chain (5 luma / 6 chroma levels).
- [ ] Simple sliders (Luminance / Color + Auto) mapped to fixed curves.
- [ ] Chroma coarse-level mean-shrink (mottling / Smoothness behavior).
- [ ] Preview-scale σ handling; CPU fallback for export.
- [ ] Panel v1 (Manual section only).

**Exit:** ≥ RawTherapee-default quality on bench set (SSIM within 1%, blind A/B panel of
5 images judged equal-or-better); ≤ 15 ms full-res 24 MP wavelet path on RTX 3060-class.

## Phase 4 — NLM + detail recovery + advanced UI (2–3 weeks)

- [ ] NLM compute shader (shared-memory tiles, scattering); engine selector.
- [ ] Residual DCT recovery + Detail slider; texture-mask Conservative/Aggressive.
- [ ] Advanced disclosure: strength, curves, NLM params, impulse, mode.
- [ ] Local-adjustment integration (Luminance/Color inside masks).

**Exit:** "Luminance 100 + Detail 60" workflow reproduces RT's flagship result; NLM ≤
40 ms full-res; all controls in sidecar round-trip.

## Phase 5 — AI track v1 (3–4 weeks)

- [x] ONNX inference — **deviation: tract-onnx (already bundled for u2netp), not
      `ort`** — fixed 576² padded tiles through one compiled plan; encode/decode
      gamma wrap; classical stand-in only when no model file is installed.
      (`ort`/EP matrix stays open as a perf upgrade.)
- [ ] nind-denoise UNet → ONNX conversion (fp16, dynamic dims); license review;
      model downloader with resume/sha256 (registry exists; drop the weight file
      at `<app support>/models/denoise/nind-utnet-v2.onnx` or point
      `MERARAW_DENOISE_MODELS_DIR` at it).
- [x] Tiler (512², 64 px overlap, cosine merge) + job manager (single-flight,
      progress events over the engine event channel, cancel, worker full-res
      re-decode).
- [x] Amount blend + disk cache (amount baked at job time, cached per 5%
      bucket; result replaces the working master so preview *and* export
      consume it — "real-time" GPU blend of a full-strength base stays open).
- [x] Panel AI section (progress/done/error events live; uncheck = re-decode
      reset). Batch "denoise selected" still open.

**Exit:** ISO 6400 bench images: AI beats classical track by ≥ 1.5 dB PSNR and wins blind
A/B; 24 MP job ≤ 30 s on mid GPU with correct ETA ±30%; cancel leaves app consistent;
seam test image shows no tile artifacts.

## Phase 6 — Polish & flagship experiments (ongoing)

- [ ] Auto-ISO import defaults; sharpening/demosaic hints.
- [ ] Split/diff preview.
- [ ] Phase B model (noise-map-conditioned) training + eval.
- [ ] Phase C joint demosaic+denoise prototype (Bayer first).
- [ ] candle/burn wgpu inference experiment (drop native ORT dep if viable).

---

## Crate list

| Crate | Use |
|---|---|
| `wgpu` (existing) | all classical passes |
| `rawler` (existing) | mosaic access, CFA layout, black/white levels, EXIF |
| `ort` | ONNX Runtime inference |
| `ndarray` | tensor staging CPU-side |
| `half` | f16 buffers/cache |
| `zstd` | cache compression |
| `rayon` | CPU fallbacks, profiling estimation |
| `sha2`, `reqwest`/`tauri-plugin-http` | model downloads |
| `serde`/`serde_json` (existing) | settings, registry |
| `image` + `dng`-writer (existing) | bench harness I/O |
| dev: `criterion`, `approx`, `insta` | benches, numeric tests, snapshot reports |

## Test strategy

1. **Numeric unit tests** — VST round-trip, wavelet perfect reconstruction, median
   sorting network, DCT orthogonality, tile merge windows sum to 1. CPU vs GPU parity
   (tolerance 1e-3).
2. **Ground-truth benchmarks** — synthetic noise on clean images (exact PSNR/SSIM) +
   NIND real pairs (MS-SSIM); tracked over time in CI (report artifact, fail on >2%
   regression).
3. **Golden-image snapshots** — fixed settings on fixed raws → hash-compared outputs
   (with a tolerance-aware perceptual diff, since GPU drivers vary).
4. **Performance gates** — criterion timings per pass with budget assertions (doc 03
   §5 budget table).
5. **Fuzz/edge** — 1×1 to 10000×2 images, all-black, all-clipped, NaN inputs from
   damaged raws, mid-job cancel storm, VRAM-pressure tile shrink path.
6. **Blind A/B protocol** — 5 testers, 10 images, MeraRAW vs RT/dt/LR exports at
   matched intent; ≥ "no worse" required before each phase ships.

## Risks

| Risk | Mitigation |
|---|---|
| NLM too slow on low-end GPUs | scattering + preview-scale + wavelet default; NLM opt-in |
| VST bias artifacts in deep shadows | exact unbiased inverse + bias-correction LUT term; dark-frame tests |
| ORT native binary size/platform pain | on-demand runtime download; candle fallback track |
| nind model license unclear for bundling | download-on-use with attribution now; retrain own weights (Phase B) |
| X-Trans everywhere (demosaic, models) | Bayer-first for Phase C; RGB model covers X-Trans meanwhile |
| Cache disk bloat (LR's DNG lesson) | LRU cap + regenerable-by-design cache |
