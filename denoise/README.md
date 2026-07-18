# MeraRAW — Advanced Denoising Feature

**Project specification & implementation guide**
Target stack: Rust + Tauri + wgpu (WGSL) · Status: design phase

---

## What this is

A complete, research-backed project description for building a state-of-the-art noise
reduction system in MeraRAW. The design draws on how the four reference applications
solve the problem:

| App | Core approach | What we borrow |
|---|---|---|
| **darktable** | *denoise (profiled)* — per-camera noise profiles + variance stabilization, then Non-Local Means or wavelet shrinkage; separate *raw denoise* and *surface blur* modules | Noise profiling, variance-stabilizing transform, NLM + wavelet dual engine, Y0U0V0 luma/chroma split, per-scale wavelet curves |
| **RawTherapee** | Wavelet decomposition (5–7 levels) + DCT (Fourier) residual pass, median filters, impulse-noise tool, hot/dead pixel filter, conservative/aggressive modes | Multi-stage pipeline (impulse → wavelet → DCT residual → median), luminance "detail recovery", chroma auto mode, tiling for memory |
| **Lightroom / ACR** | AI Denoise: a neural network that performs **joint demosaic + denoise** directly on Bayer/X-Trans mosaic data, single Amount slider, non-destructive result; plus classic manual Luminance/Color sliders | One-slider AI mode operating on raw mosaic data, "it just works" UX, manual sliders as fallback/refinement |
| **RapidRAW** | Same stack as us (Rust/Tauri/wgpu/WGSL). Manual denoise module in the GPU shader pipeline + AI noise reduction powered by **nind-denoise** UNet models | Proof that WGSL real-time denoise + optional ONNX UNet inference works in this exact architecture |

## Document map

| File | Contents |
|---|---|
| [`01-competitive-analysis.md`](01-competitive-analysis.md) | Deep dive into how each reference app implements denoising — algorithms, pipeline placement, UI |
| [`02-noise-model-and-theory.md`](02-noise-model-and-theory.md) | Sensor noise physics, Poisson–Gaussian model, variance stabilization, luma/chroma & frequency decomposition — the math the engine is built on |
| [`03-architecture.md`](03-architecture.md) | Where denoise sits in MeraRAW's pixel pipeline, Rust module layout, GPU/CPU split, caching, Tauri command surface |
| [`04-classical-algorithms.md`](04-classical-algorithms.md) | Implementation specs for every classical stage: hot-pixel, impulse median, guided pre-filter, GPU Non-Local Means, à-trous wavelet shrinkage, chroma denoise, detail recovery |
| [`05-ai-denoising.md`](05-ai-denoising.md) | Neural denoise track: model choice (nind-denoise-style UNet), ONNX Runtime vs candle/burn, tiled inference, model packaging, joint demosaic-denoise roadmap |
| [`06-ui-ux-spec.md`](06-ui-ux-spec.md) | Panel layout, slider semantics, auto mode, presets, preview behavior, progress & cancellation UX |
| [`07-implementation-roadmap.md`](07-implementation-roadmap.md) | Phased milestones, crate list, test strategy, benchmark suite, acceptance criteria |
| [`08-technical-algorithm-summary.md`](08-technical-algorithm-summary.md) | Full technical summary of the shipped math/algorithms (for verifying against papers & other apps) |

## Design pillars

1. **Two tracks, one panel.** A *Classical* engine (real-time, GPU, always available) and an
   *AI* engine (slower, higher quality, optional model download) — mirroring
   Lightroom's manual-vs-Denoise split and RapidRAW's manual module + nind-denoise.
2. **Profile-aware.** Noise strength is estimated from ISO + camera metadata (and refined
   from the image itself), so the default result is right before the user touches a slider —
   darktable's biggest usability win.
3. **Denoise early.** Classical raw-domain steps run before/at demosaic where noise is still
   uncorrelated; perceptual controls operate in a luma/chroma space. Never denoise after
   sharpening.
4. **Non-destructive.** All settings live in the sidecar; AI results are cached tiles/buffers,
   never baked into the source file.
5. **Real-time preview.** The classical path must hold 60 fps interaction on a 24 MP image
   preview on mid-range GPUs; AI runs as an async job with progress + cancel.

## Quick pipeline sketch

```
RAW mosaic
  │
  ├─ hot/dead pixel suppression (raw domain)
  ├─ [AI track] joint denoise (UNet on mosaic or early RGB) ──┐
  ├─ demosaic                                                 │
  ├─ [classical track] impulse median (salt & pepper)         │
  ├─ [classical track] transform to Y0U0V0                    │
  │     ├─ luma: NLM or wavelet shrinkage (user choice/auto)  │
  │     └─ chroma: wavelet shrinkage (stronger, safe)         │
  ├─ detail recovery (blend original high-freq back)  ◄───────┘
  ├─ ...rest of pipeline (tone, color, local adjustments)
  └─ sharpening (always AFTER denoise)
```
