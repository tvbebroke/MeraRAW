# 03 — Architecture

How the denoise feature integrates with MeraRAW's Rust + Tauri + wgpu stack.

---

## 1. High-level design

Two engines behind one panel:

```
                        ┌────────────────────────────┐
                        │  Denoise Panel (React/TS)   │
                        │  mode: Off | Classical | AI │
                        └──────────┬─────────────────┘
                                   │ Tauri commands / events
        ┌──────────────────────────┴───────────────────────────┐
        │                    Rust core (tauri backend)          │
        │                                                       │
        │  denoise::profile     noise model (a,b) estimation    │
        │  denoise::classical   orchestrates GPU passes         │
        │  denoise::ai          ONNX session, tiling, cache     │
        │  denoise::cache       denoised-buffer cache & sidecar │
        └───────┬───────────────────────────────┬───────────────┘
                │ wgpu compute/render passes     │ ort / candle
        ┌───────┴────────┐              ┌────────┴─────────┐
        │  WGSL shaders  │              │  UNet model(s)   │
        │  (real-time)   │              │  (async jobs)    │
        └────────────────┘              └──────────────────┘
```

- **Classical engine**: pure wgpu passes woven into the existing pixel pipeline.
  Recomputed live on every slider change; must sustain interactive preview.
- **AI engine**: an asynchronous job producing a *denoised base buffer* that replaces the
  demosaiced image at the head of the pipeline. Cached on disk; all other edits apply on
  top, so once computed it costs nothing at interaction time. (This mirrors LR 14.4's
  non-destructive Denoise and RapidRAW's model-based features.)

## 2. Pipeline placement

Assuming MeraRAW's current pipeline is roughly
`decode(rawler) → black/white levels → demosaic → WGSL adjustment chain → export`:

```
decode (rawler)
  └─ black level, white balance pre-scale (linear)
      └─ [P0] hot/dead pixel pass ............ raw mosaic, compute shader
      └─ [P1] AI denoise (optional) .......... replaces demosaic output when enabled
      └─ demosaic
      └─ [P2] impulse median (optional) ...... post-demosaic RGB
      └─ [P3] classical denoise .............. Y0U0V0, VST, wavelet/NLM, detail recovery
      └─ existing adjustment megashader (exposure, curves, HSL, ...)
      └─ [P4] sharpening / texture / grain ... unchanged, but AFTER P3 by construction
```

Rules:
- P0 always-on-able independent of the rest (cheap).
- P1 and P3 can coexist: AI does the heavy lifting, classical adds user-tunable
  refinement on top (LR's model: Denoise + manual sliders).
- P3 operates on linear data *before* tone mapping. If MeraRAW's megashader currently
  applies tone early, denoise must be inserted upstream of it.

## 3. Rust module layout

```
src-tauri/src/denoise/
├── mod.rs               // public API, DenoiseSettings struct, command handlers
├── profile.rs           // noise model: EXIF seed + image-based (a,b) estimation, VST LUTs
├── classical/
│   ├── mod.rs           // pass orchestration, parameter → uniform buffer mapping
│   ├── hotpixel.rs      // P0
│   ├── impulse.rs       // P2 median
│   ├── wavelet.rs       // à-trous decompose/shrink/reconstruct pass chain
│   ├── nlm.rs           // non-local means pass
│   └── recovery.rs      // residual DCT + texture-mask blend
├── ai/
│   ├── mod.rs           // job manager: queue, progress events, cancellation
│   ├── session.rs       // ort (ONNX Runtime) session lifecycle, EP selection
│   ├── tiler.rs         // tile split/overlap/merge, halo handling
│   └── models.rs        // model registry, download, checksum, versioning
├── cache.rs             // denoised-base cache (disk), keyed by (file hash, model, amount)
└── shaders/
    ├── hotpixel.wgsl
    ├── impulse_median.wgsl
    ├── ycbcr_y0u0v0.wgsl        // color space to/from
    ├── vst.wgsl                  // forward + unbiased inverse (LUT texture)
    ├── atrous_decompose.wgsl     // one level per dispatch, ping-pong textures
    ├── atrous_shrink.wgsl
    ├── nlm.wgsl                  // tiled, workgroup-shared-memory patch cache
    ├── dct_residual.wgsl         // 8x8/16x16 block DCT threshold
    └── blend_recovery.wgsl
```

## 4. Settings model (sidecar)

```rust
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct DenoiseSettings {
    pub enabled: bool,
    pub mode: DenoiseMode,            // Classical | Ai | AiPlusClassical
    // -------- AI --------
    pub ai_amount: f32,               // 0..100, default 50 (LR semantics)
    pub ai_model: String,             // registry id, e.g. "nind-utnet-v2"
    // -------- Classical: simple --------
    pub luminance: f32,               // 0..100
    pub luma_detail: f32,             // 0..100  (detail recovery)
    pub chrominance: f32,             // 0..100, default from auto-estimate
    pub chroma_auto: bool,            // RT-style auto chroma, default true
    pub strength: f32,                // 0.25..4.0 global profile scale, default 1.0
    // -------- Classical: advanced --------
    pub engine: LumaEngine,           // WaveletAuto | Wavelet | Nlm
    pub wavelet_luma_curve: [f32; 6], // per-scale strength
    pub wavelet_chroma_curve: [f32; 6],
    pub nlm_patch: u8, pub nlm_search: u8, pub nlm_center_weight: f32,
    pub impulse: f32,                 // 0..100 salt&pepper median
    pub hot_pixels: bool,             // default true
    pub recovery_mode: RecoveryMode,  // Conservative | Aggressive
    // -------- Local --------
    pub respects_masks: bool,         // panel available inside local adjustments
}
```

All fields serialize to the existing sidecar (`.mrdata` or equivalent). Reordering-safe:
denoise settings are declarative; pipeline order is fixed by the engine, not by the file.

## 5. GPU strategy (classical path)

- **Compute shaders**, not fragment passes, for everything except trivial blends —
  wavelet levels and NLM need scatter-free but neighborhood-heavy access; workgroup
  shared memory tiles (e.g. 16×16 + apron) cut global memory traffic dramatically,
  especially for NLM.
- **Ping-pong `rgba16float` textures** for the à-trous pyramid (store detail levels as a
  texture array: `levels: texture_2d_array<f32>`, 6 layers max).
- **VST as 1D LUT texture** (forward and exact-unbiased inverse), regenerated when
  (a, b) or strength changes — avoids sqrt-heavy math per pixel and makes the unbiased
  inverse free.
- **Preview-scale processing**: at fit-to-screen zoom, run denoise on the downsampled
  buffer but with σ scaled appropriately (downsampling averages noise: σ_preview ≈
  σ / downscale). At 100% zoom, run full-res on the visible tile only. This is how the
  panel stays 60 fps.
- **Cost budget** (24 MP, mid-range GPU, full res): hot-pixel <1 ms, impulse median
  <2 ms, wavelet chain ~6–12 ms, NLM 20–80 ms (hence preview-scale + tile tricks),
  DCT recovery ~5 ms.

## 6. AI job architecture

- **Runtime: `ort` (ONNX Runtime) as default.** Execution providers by platform:
  DirectML (Windows), CoreML (macOS), CUDA/ROCm if present, CPU fallback. Alternative
  pure-Rust path via `candle`/`burn` + wgpu is kept behind a feature flag (see doc 05).
- **Job lifecycle:** `enqueue(file, settings) → progress events (tauri emit, per-tile) →
  result written to cache → pipeline invalidation → re-render`. Cancellation via token
  checked between tiles.
- **Tiling:** 512×512 tiles with 32–64 px overlap, feathered merge, to bound VRAM and
  give granular progress. Reflect-pad borders.
- **Cache:** `cache_dir/denoise/{content_hash}/{model_id}/{amount_bucket}.buf`
  (half-float planar, zstd). Amount is bucketed (e.g. steps of 5) or — better — the
  model outputs a *full-strength* denoised buffer and Amount is implemented as a
  GPU blend `mix(original, denoised, amount/100)`, so one inference serves every
  slider position. **Choose the blend design** — it makes the AI slider real-time after
  the first run, something Lightroom cannot do.
- **Memory guardrails:** query adapter limits; reduce tile size under pressure; never
  hold more than 2 tiles + model workspace resident.

## 7. Tauri command surface

```rust
#[tauri::command] fn denoise_estimate_profile(path) -> NoiseProfile;      // (a,b), suggested defaults
#[tauri::command] fn denoise_set_settings(path, settings) -> ();          // live classical update
#[tauri::command] fn denoise_ai_start(path, settings) -> JobId;
#[tauri::command] fn denoise_ai_cancel(job) -> ();
#[tauri::command] fn denoise_models_list() -> Vec<ModelInfo>;
#[tauri::command] fn denoise_models_download(id) -> JobId;                // progress via events
// events: "denoise://progress" { job, pct, tile, eta }, "denoise://done", "denoise://error"
```

## 8. Failure & fallback matrix

| Condition | Behavior |
|---|---|
| No GPU / wgpu init fails | Classical path has a rayon CPU fallback for export (slow); preview shows warning |
| ONNX EP unavailable | Fall back CPU EP with time estimate warning (LR shows ETA too) |
| Model not downloaded | Panel shows download card with size; classical stays usable |
| VRAM exhaustion mid-job | Halve tile size, retry tile ×2, then fail job gracefully |
| Unsupported raw (already-demosaiced DNG, JPEG) | AI mosaic-domain models disabled; RGB-domain model or classical only (LR has the same limitation) |
