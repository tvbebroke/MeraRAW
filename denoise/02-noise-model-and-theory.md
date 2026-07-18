# 02 — Noise Model & Theory

The math the denoise engine is built on. Everything downstream (algorithm choices, pipeline
placement, UI defaults) follows from this model.

---

## 1. Where sensor noise comes from

For a raw pixel value `x` (linear, black-level subtracted), the dominant noise sources:

1. **Shot noise (photon noise).** Light arrival is a Poisson process: variance equals the
   mean photon count. Unavoidable physics — the "rain in buckets" model from Adobe's
   Denoise write-up. Dominates mid-tones/highlights.
2. **Read noise.** Electronics: amplifier + ADC noise, approximately Gaussian with
   constant variance. Dominates deep shadows — this is why pushing exposure in post is
   noisier than raising ISO in camera (ISO amplification happens before the read stage on
   most sensors).
3. **Fixed-pattern components.** Hot/dead pixels (defective sites), banding/line noise
   (row/column readout), PRNU. Handled by dedicated pre-passes, not the statistical
   denoiser.
4. **Quantization noise.** Usually negligible next to 1–2.

## 2. The Poisson–Gaussian model

Standard signal-dependent model used by darktable's profiled denoise:

```
x_noisy = x_true + n,     Var(n) = a·x_true + b
```

- `a` — shot-noise gain (scales with ISO)
- `b` — read-noise floor (also ISO-dependent)

`(a, b)` per ISO per camera is exactly what a darktable "noise profile" is.

**Consequences:**
- Noise variance is *not constant* — shadows pushed up in post have far more visible
  noise than highlights. Any denoiser using a single global threshold will either
  under-denoise shadows or destroy highlight detail.
- Noise is only simple (uncorrelated, per-pixel, signal-dependent in this clean form)
  **in the raw/linear domain, before demosaicing and before tone curves**. Demosaic
  interpolation correlates neighboring pixels; gamma/tone-mapping makes variance a
  complicated function of the signal. This is the theoretical reason both darktable
  and Lightroom's AI Denoise operate as early as possible.

## 3. Variance stabilization (VST)

Trick: transform the data so noise variance becomes ~constant, run a denoiser designed
for uniform Gaussian noise, invert the transform.

Classic Anscombe transform for pure Poisson:
```
f(x) = 2·sqrt(x + 3/8)
```
Generalized to Poisson–Gaussian with profile (a, b):
```
f(x) = (2/a) · sqrt(a·x + 3/8·a² + b)
```

Implementation notes:
- The naive algebraic inverse is **biased** in shadows (this is the cause of darktable's
  historical "shadows go dark/purple" artifact and why it grew a *bias correction*
  control). Use the closed-form **exact unbiased inverse** (Makitalo & Foi) or a small
  LUT correction.
- With VST in place, one denoiser + one threshold serves the whole tonal range. Strength
  slider = scale factor on the assumed `(a, b)`.
- Without a full community profile database, MeraRAW can **estimate (a, b) from the image
  itself**: bin pixels by intensity from flat regions (low local gradient), fit variance
  vs. mean with robust regression. Use ISO + camera model from EXIF to seed the estimate.
  This is the "profile-lite" approach — ship it first, add a real profile DB later
  (darktable's profiles are open data and a compatible format could be imported).

## 4. Luma vs. chroma noise

After (or during) demosaic, decompose into a luminance channel and two chroma channels.
Perceptual facts all four reference apps agree on:

- **Chroma noise** (colored speckles/blotches) is always ugly and can be removed
  aggressively — the eye barely resolves fine chroma detail. LR defaults Color NR to 25;
  RT removes it automatically; darktable's Y0U0V0 allows strong chroma shrinkage.
- **Luma noise** resembles film grain, carries perceived sharpness, and over-removal
  produces the "plastic" look. Remove conservatively; offer detail recovery.

Use darktable's **Y0U0V0** space (a luma/chroma rotation optimized so that noise
separates cleanly and channel crosstalk is minimized) rather than Lab — Lab conversion
mid-pipeline is expensive and its L is tone-curve dependent. Y0U0V0 is a simple linear
3×3 matrix:

```
Y0 =  (R + G + B) / 3            (achromatic axis, equal weights — noise-optimal)
U0 =  (R − B) / 2
V0 =  (R − 2G + B) / 4
```
(Exact coefficients TBD in implementation; darktable's source uses an orthonormal-ish
variant. Requirement: linear, invertible, cheap in a shader.)

## 5. Frequency decomposition

Noise lives at all scales: fine grain (high frequency) and coarse blotches (low
frequency, especially chroma). A multiscale transform lets us shrink each scale
independently — this is RT's fine/coarse chroma split and darktable's wavelet curves.

**Chosen transform: à-trous ("with holes") undecimated wavelet, B3-spline kernel.**
Reasons:
- Shift-invariant (no blocking/ringing artifacts of decimated DWT).
- Each level is a simple separable 5-tap convolution with increasing hole spacing —
  trivially GPU-parallel in WGSL, and darktable/RT both build on it.
- Perfect reconstruction: `image = Σ detail_levels + residual`.

Per-level **shrinkage** of detail coefficients `d` with noise σ_level (propagated from
the stabilized noise σ through the kernel):

```
soft threshold:   d' = sign(d) · max(|d| − k·σ_level, 0)
or Wiener-like:   d' = d · max(d² − σ², 0) / d²   (less bias, keeps texture better)
```

5 levels for luma, 6 for chroma (RT's numbers) is a solid default; expose per-level
strength as the "advanced curves" UI.

## 6. Patch self-similarity (Non-Local Means)

Complementary principle: natural images repeat themselves. For pixel `p`, average pixels
`q` in a search window, weighted by similarity of the *patches* around them:

```
w(p,q) = exp( − max(‖P(p) − P(q)‖² − 2σ², 0) / h² )
out(p) = Σ w(p,q)·x(q) / Σ w(p,q)
```

- Patch size ~3×3–7×7; search window ~radius 7–15.
- `h` (filtering strength) derives from stabilized σ → auto mode possible.
- **Central-pixel weight** (darktable): blend some of the original pixel back in —
  cheap, effective detail preservation.
- **Scattering**: sample the search window sparsely/stochastically at coarse scales to
  cut cost (darktable does this to make NLM tractable).
- Cost is O(searchArea · patchArea) per pixel → this is a GPU shader by necessity.

NLM excels on heavy, coarse noise and smooth gradients (skies, skin); wavelets are
cheaper and better at controlled per-scale work. Ship both, default to wavelets
(darktable moved its default to wavelet-auto for the same reason).

## 7. Detail recovery (the RT/LR secret sauce)

Aggressive denoise + selective restore beats timid denoise. Two mechanisms to implement:

1. **Residual DCT pass (RawTherapee).** Compute `residual = original − denoised`. The
   residual contains noise *plus* fine real detail. Run a 8×8/16×16 block DCT over the
   residual, threshold small coefficients (noise), keep large ones (structure), add the
   filtered residual back. The *Detail Recovery* slider scales how much comes back.
2. **Edge/texture masked blend.** Compute a texture-vs-flat map (local variance vs.
   expected noise variance from the model). Blend `denoised ↔ original` per pixel:
   flat areas take full denoise, textured areas keep more original. This doubles as
   the Conservative/Aggressive switch (threshold on the map).

## 8. Ordering rules (hard constraints)

1. Hot/dead pixel + line-noise fixes → **raw mosaic domain**, before everything.
2. Statistical denoise → **linear domain, at/near demosaic**, before tone curves and
   input color transforms that would invalidate the noise model.
3. Impulse median → right after demosaic (salt & pepper survives demosaic as colored
   dots).
4. **Sharpening, clarity, texture, grain → strictly after denoise.** Also: warn/auto-zero
   fine-detail contrast boosts when strong NR is active (RT's documented workflow).
5. High-ISO images prefer noise-tolerant demosaicers (LMMSE/IGV-class) over
   detail-maximizing ones (AMaZE) — if MeraRAW has multiple demosaicers, auto-switch or
   suggest at high ISO.

## 9. References

- darktable docs — *denoise (profiled)*, *raw denoise* module reference.
- RawPedia — *Noise Reduction*, *About Noise Reduction*, *Impulse Noise Reduction*,
  *Wavelet Levels*.
- Eric Chan (Adobe), *Denoise Demystified*, Adobe Blog, 2023.
- Buades, Coll, Morel — *A non-local algorithm for image denoising*, CVPR 2005.
- Starck, Murtagh — à-trous / starlet transform literature.
- Makitalo, Foi — *Optimal inversion of the generalized Anscombe transformation*.
- Brummer, De Vleeschouwer — *Natural Image Noise Dataset*, CVPRW 2019 (NIND).
