# 05 — AI Denoising Track

The neural engine: one slider, Lightroom-class results, fully local, non-destructive.

---

## 1. Goal & positioning

- **UX target (from Lightroom):** a single *Amount* slider (0–100, default 50) and an
  Enhance-style action with ETA, preview, progress, cancel. No parameters to understand.
- **Architecture target (from RapidRAW):** local inference, optional downloadable models,
  the core app stays lightweight (<20–30 MB) — models are fetched on first use.
- **Quality target:** visibly better than the classical track at ISO ≥ 3200; reconstructs
  texture instead of smoothing (the qualitative difference Adobe describes: recognize
  patterns and rebuild plausible detail, don't blur).

## 2. Model strategy — three phases

### Phase A (ship first): RGB-domain UNet — nind-denoise lineage
- **What:** UNet / UtNet trained on the **Natural Image Noise Dataset (NIND)** — real
  clean/noisy pairs (same static scene at base and high ISO), so it learns *real sensor
  noise*, not synthetic Gaussian. This is exactly what powers RapidRAW's AI NR, and the
  models are openly available (convert PyTorch → ONNX).
- **Input:** demosaiced linear RGB (pre-sharpening, pre-tone). nind-denoise's own
  guidance: run on unsharpened images, sharpen after.
- **Why first:** proven in our exact stack, permissively available, works on non-raw
  sources too (JPEG/TIFF get AI denoise — something Lightroom can't do).
- **Deliverable:** `meraraw-denoise-rgb-v1.onnx` (~5–30 MB fp16).

### Phase B: noise-conditioned model
- Add a **noise-level map input channel** (from our (a, b) profile, per-pixel expected σ)
  → one model handles all ISOs smoothly and the Amount slider can modulate the map
  instead of blending outputs. Architectures to evaluate: UNet+σ-map (FFDNet-style
  conditioning), NAFNet-small, Restormer-lite. Constraint: must run a 512² tile in
  < 150 ms on DirectML/CoreML mid-range hardware.

### Phase C (flagship): joint demosaic + denoise on the mosaic
- The Lightroom approach: network input = packed Bayer planes (RGGB 4×H/2×W/2) +
  noise map; output = clean full-res RGB. Best possible quality — noise is untouched by
  interpolation. X-Trans needs its own model or a 6×6-aware packing.
- Training data: NIND raws + synthetic Poisson–Gaussian noise injected into clean raws
  (unprocessed pairs), following the published joint-demosaic-denoise literature.
- This replaces the demosaic step entirely when active (pipeline P1 slot).

## 3. Inference runtime

**Primary: `ort` crate (ONNX Runtime).**

| Platform | Execution provider |
|---|---|
| Windows | DirectML (any GPU vendor) → CPU fallback |
| macOS | CoreML (ANE/GPU) → CPU |
| Linux | CUDA / ROCm if present → CPU |

Pros: mature EPs, fp16, easy model interchange. Cons: native dylib per platform
(~15–60 MB) — acceptable as an on-demand download alongside models if binary size is a
concern; Tauri sidecar or dynamic-load pattern.

**Alternative (feature-flagged): `candle` or `burn` with the wgpu backend.** Pure Rust,
zero extra native deps, shares the GPU device with the render pipeline. Currently slower
and less operator coverage than ORT EPs — keep as an experiment target, promote if it
reaches ≥70% of DirectML throughput.

## 4. Tiled inference

- Tile 512×512 (adaptive down to 256 under VRAM pressure), **overlap 64 px** (UNet
  receptive field halo), reflect-padding at image borders.
- Merge with a raised-cosine window over the overlap; verified seam-free on gradient test
  images.
- Pre/post: linear RGB → model's expected range (train with linear input to avoid a
  gamma mismatch); fp16 tensors.
- Progress = tiles done / total; cancellation checked per tile.
- A 24 MP image ≈ 110 tiles ≈ 10–30 s on mid GPU, minutes on CPU → always show ETA
  (Lightroom sets this expectation precedent).

## 5. The Amount slider — do it better than Lightroom

Run inference **once at full strength**, cache the result, and implement Amount as a
GPU blend in the pipeline:

```
base = mix(demosaiced, ai_denoised_cached, amount/100)
```

Consequences:
- Amount becomes a **real-time slider** after the first run (LR requires re-running
  Enhance to change it — a known pain point).
- One cache entry per (file, model) instead of per amount.
- Classical track on top still works for refinement (chroma touch-up, extra luma).

Optional refinement: perceptually weighted blend (blend more in flat areas, less on
edges, using the P3.d texture map) exposed as a "protect detail" toggle.

## 6. Model management

```
~/.meraraw/models/
  registry.json      // id, version, sha256, size, url[], license, domains: [rgb|bayer|xtrans]
  nind-utnet-v1.onnx
```
- Registry fetched from a static endpoint (also bundled snapshot for offline).
- Download with resume + sha256 verify; Tauri progress events.
- License note in UI (NIND models: check upstream license/attribution; ship our own
  retrained weights for a clean story — training recipe in Phase B).

## 7. Cache & non-destructiveness

- Output stored as half-float planar buffer, zstd, keyed by
  `(content_hash, model_id, model_version)`. Typical 24 MP entry ~70–140 MB → LRU cap
  (configurable, default 10 GB) + "clear denoise cache" in settings.
- Sidecar stores only `{mode, model_id, amount}` — re-running is deterministic, so the
  cache is disposable (unlike LR's old Enhanced-NR DNGs that bloated catalogs 2–5×; we
  regenerate on demand).
- Export path: if cache missing at export time, inference runs as part of the export job.

## 8. Constraints & edge cases

- Mosaic-domain models (Phase C): Bayer + X-Trans only, mirroring LR's constraint;
  linear DNG / JPEG / TIFF fall back to the RGB model.
- Highlights: pass clipped-highlight mask to inference postprocess so reconstruction
  never invents texture inside blown areas.
- Determinism: fix ORT graph optimization level & disable non-deterministic kernels so
  the same input yields byte-identical cache entries (important for tests).
- Batch: queue N files sequentially (GPU) — expose "Apply AI denoise to selected" in the
  library, LR-style sync workflow.

## 9. Training pipeline (Phase B/C, separate repo)

- Data: NIND + our own captured pairs (checklist: tripod, static scene, ISO ladder,
  multiple cameras incl. X-Trans) + synthetic Poisson–Gaussian augmentation with sampled
  (a, b) from real profiles.
- Loss: L1 + MS-SSIM (nind-denoise's evaluation metrics) + optional light perceptual
  term; avoid GAN artifacts for v1.
- Export: PyTorch → ONNX (opset ≥ 17), fp16, dynamic H/W dims, fixed 512² profile shape
  for EP warmup.
- Eval harness shared with doc 07 benchmarks.
