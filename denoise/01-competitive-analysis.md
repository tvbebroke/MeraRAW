# 01 — Competitive Analysis

How Lightroom, RawTherapee, darktable, and RapidRAW implement noise reduction, and what
MeraRAW should take from each. Sources: official documentation (darktable dtdocs, RawPedia,
Adobe's "Denoise Demystified" blog by Eric Chan) and the RapidRAW repository.

---

## 1. darktable

darktable ships several noise-related modules; the flagship is **denoise (profiled)**.

### 1.1 denoise (profiled)

**Key idea:** the module knows the *noise characteristics of your specific camera at your
specific ISO* from a community-collected profile database. It uses that profile to apply a
**variance-stabilizing transform (VST)** — a generalized Anscombe-style transform tuned by
the profile — so that the Poisson–Gaussian sensor noise becomes approximately uniform
(constant variance) across the tonal range. Then a generic denoiser runs in that stabilized
space, and the transform is inverted.

This is why darktable's defaults are so good: shadows (noisier) automatically receive more
denoising than highlights without the user configuring anything.

**Two core engines, selectable:**

- **Non-local means (NLM):** patch-based; for every pixel, similar patches in a search
  window are averaged, weighted by patch similarity. Excellent smooth results, especially
  on heavy noise; computationally expensive. Options: patch size, search radius,
  scattering (sparse sampling of the search window to cut cost), central-pixel weight
  (how much of the original pixel to keep — a built-in detail-preservation dial), and an
  "auto" variant that infers parameters from the profile.
- **Wavelets:** à-trous (starlet) decomposition; per-scale shrinkage of coefficients.
  Cheaper than NLM. Two color modes:
  - **Y0U0V0** — a luma/chroma space designed for denoising; separate strength *curves*
    for luma and chroma, each curve mapping coarseness (wavelet scale) → strength.
  - **RGB** — independent per-channel curves (useful for e.g. blue-channel noise).

**Per-scale wavelet curves** are a signature feature: x-axis = decomposition level
(coarse → fine), y-axis = shrinkage strength. Users can kill coarse chroma blotches while
leaving fine luma grain untouched.

**Shared controls:** *strength* (scales the profiled variance), *preserve shadows*
(deprecated in newer versions thanks to better VST), *bias correction* (fixes shadows
turning too dark/purple after denoising, a known VST artifact).

**Pipeline placement:** very early — before the input color profile, right around
demosaic, where the profile's noise parameters are still valid. Modern guidance: one
instance handles both luma and chroma internally (the old two-instance blend-mode
technique is obsolete).

**Other darktable modules worth knowing:**
- **raw denoise** — wavelet shrinkage directly on the mosaic before demosaic (no profile).
- **hot pixels** — thresholded detection & neighbor replacement in the raw domain.
- **surface blur / bilateral** — edge-preserving smoothing, occasionally used for chroma.
- **astrophoto denoise** — median-stack based, niche.

**Take for MeraRAW:** the profile + VST concept (even a simplified ISO-parameterized
version), NLM/wavelet dual engine, Y0U0V0 split, per-scale curves as an "advanced" UI,
early pipeline placement, auto parameter inference.

---

## 2. RawTherapee

RawTherapee's noise reduction (largely designed by Emil Martinec in 2012, extended by
Ingo Weyrich and Jacques Desmis) is the most *staged* of the four — several cooperating
tools, each targeting a noise species:

| Noise species | Tool |
|---|---|
| Hot/dead pixels | Hot/Dead Pixel Filter (raw domain) |
| Salt-and-pepper impulse noise | Impulse Noise Reduction |
| Line/pattern noise | Line Noise Filter (raw domain) |
| Chrominance noise | Noise Reduction (wavelet, auto-chroma) |
| Luminance noise | Noise Reduction (wavelet + DCT) |
| Residual artifacts | Median filter pass |

### 2.1 Main Noise Reduction tool (Detail tab)

- Works in **L\*a\*b\*** (default), "Luminance only", or RGB.
- **Wavelet decomposition**: 5 levels for luminance, 6 for chrominance (7 in the newer
  Local Adjustments variant). Shrinkage per level.
- **DCT (Fourier) residual pass** for luminance: after wavelet denoising, a discrete
  cosine transform processes the *residual* (original − wavelet-denoised) to catch noise
  the wavelets missed. The **"Luminance detail" (detail recovery) slider** controls how much
  original detail this pass restores — the signature RT control: set Luminance to 100,
  then raise Detail Recovery until texture returns.
- **Chrominance**: "Automatic global" mode estimates chroma noise and removes it with
  essentially zero user input (RT's docs: chroma noise is endemic, always remove it;
  luma noise can look like film grain and is a taste question). Manual mode adds
  red–green / blue–yellow equalizers and fine vs. coarse chroma sliders (fine = levels
  0–4 dots; coarse = levels 5–6 blotches).
- **Conservative / Aggressive** modes change how flat vs. structured areas are
  differentiated.
- **Median filter** (3×3 … 9×9, iterations, luma/chroma/RGB targets) as a post pass to
  kill leftover pixel-scale artifacts.
- **Automatic tiling** for wavelet & DCT to bound memory on large files.
- Workflow guidance baked into docs: pick a noise-tolerant demosaicer (LMMSE/IGV instead
  of AMaZE) for very high ISO; zero out fine-detail sharpening; denoise before sharpening.

**Take for MeraRAW:** the *staged* mental model (impulse → chroma auto → luma + detail
recovery → median cleanup), the DCT-residual detail-recovery mechanism, auto chroma,
demosaic-algorithm interaction, tiling, and the fine/coarse chroma split.

---

## 3. Lightroom / Adobe Camera Raw

Two generations coexist:

### 3.1 Manual Noise Reduction (classic sliders)

- **Luminance / Detail / Contrast** + **Color / Detail / Smoothness**.
- Color NR defaults to 25 on raw import (chroma removal is assumed-wanted, same
  philosophy as RT's auto chroma). Luminance defaults to 0.
- *Detail* protects fine structure; *Contrast* restores luma contrast lost to smoothing;
  *Smoothness* targets low-frequency color mottling (large soft magenta/green clumps).
- Edge-preserving multiscale filtering; fast, applied live in the pipeline.

### 3.2 AI Denoise (2023+, "Denoise Demystified" by Eric Chan)

- A deep neural network that operates **on the raw mosaic (Bayer / X-Trans) data**,
  performing **demosaicing and denoising jointly** — the model sees noise before any
  interpolation smears/correlates it, which is a big part of the quality gap vs.
  classical tools. It reconstructs plausible detail rather than blurring.
- **Single "Amount" slider (0–100, default 50)**. That's the entire UI.
- Originally produced a new `*-Enhanced-NR.dng` (linear DNG, 2–5× larger); since
  LrC 14.4 (June 2025) the result is stored non-destructively in the catalog with no
  extra DNG — the direction of travel is *non-destructive AI denoise*, which validates
  MeraRAW's cache-based design.
- Requires mosaic raw input (Bayer/X-Trans, plus linear raws like ProRAW); doesn't run
  on JPEG/TIFF. Also applies "Raw Details" (demosaic enhancement) as part of the pass.
- Manual sliders remain available for refinement afterwards, and masking lets users add
  local NR to shadows etc.
- Practical guidance from Adobe/users: raising ISO in camera beats pushing exposure in
  post (read noise vs. shot noise); typical Amount 40–65; very high ISO 70–85.

**Take for MeraRAW:** the one-slider AI UX, joint demosaic-denoise as the long-term
quality target, non-destructive storage of AI output, manual sliders as the refinement
layer, local/masked NR integration.

---

## 4. RapidRAW

Directly relevant — identical stack (Rust + Tauri + React, wgpu, WGSL megashader,
`rawler` for raw decoding).

- **Manual Denoise module** (added v1.5.x): granular luminance/color noise reduction
  running inside the GPU WGSL pipeline, real-time like every other adjustment.
- **AI Noise Reduction** powered by **nind-denoise** — UNet models trained on the
  Natural Image Noise Dataset (NIND), i.e. real photographs of static scenes shot at
  low and high ISO to form clean/noisy pairs. RapidRAW runs these models locally
  (ONNX-style inference) as one of its bundled AI features (alongside SAM2 masks,
  U2-Net, Depth Anything, LaMa).
- Architecture lesson: heavy AI features are *asynchronous, optional, downloadable
  models*, while the core editing loop stays a fluid GPU shader path. The nind-denoise
  README itself notes the standard caveat: **denoise unsharpened images, sharpen after**.

**Take for MeraRAW:** feasibility proof and the two-tier pattern (WGSL real-time manual
module + local ONNX UNet for AI), plus nind-denoise/NIND as a concrete open model/dataset
starting point.

---

## 5. Synthesis — the gap MeraRAW can fill

| Capability | LR | RT | dt | RapidRAW | **MeraRAW target** |
|---|---|---|---|---|---|
| One-slider AI denoise | ✅ | ❌ | ❌ | ✅ (basic) | ✅ |
| Camera/ISO noise profiling + VST | ❌ (implicit in AI) | ❌ | ✅ | ❌ | ✅ (simplified param model) |
| Real-time GPU classical denoise | partial | ❌ (CPU) | partial (OpenCL) | ✅ | ✅ |
| Per-scale wavelet control | ❌ | ✅ | ✅ | ❌ | ✅ (advanced panel) |
| Detail-recovery residual pass | ✅ (Detail slider) | ✅ (DCT) | via central-pixel weight | ❌ | ✅ |
| Auto chroma | ✅ (default 25) | ✅ | ✅ (profile) | ❌ | ✅ |
| Non-destructive AI result | ✅ (14.4+) | n/a | n/a | ✅ | ✅ |
| Masked/local NR | ✅ | ✅ (LA) | ✅ (instances) | via masks | ✅ (reuse mask system) |

No open-source editor currently combines **profiled auto defaults (dt)** +
**staged detail recovery (RT)** + **one-slider AI (LR)** + **real-time GPU preview
(RapidRAW)** in one panel. That combination is the feature.
