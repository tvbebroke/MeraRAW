# 04 — Classical Algorithms: Implementation Spec

Concrete specs for every stage of the classical (real-time GPU) track. Each section gives
purpose, algorithm, WGSL strategy, parameters, and acceptance criteria.

---

## P0 — Hot / dead pixel suppression (raw mosaic)

**Purpose.** Kill defective sensor sites before they get smeared by demosaic into colored
crosses. (darktable *hot pixels* module; RT Hot/Dead Pixel Filter.)

**Algorithm.** For each mosaic pixel `x`, look at the 4 (or 8) *same-CFA-color* neighbors
`n_i` (offset ±2 in Bayer):
```
m = median(n_i);  d = max(n_i) - min(n_i)
if |x - m| > t · max(d, ε·white):  x' = m
```
`t` ≈ 3–5 (threshold slider internally, "detect more" toggle in UI at most). Also clamp
stuck-low pixels symmetrically.

**WGSL.** Single compute pass over the mosaic texture (`r16uint`/`r32float`), CFA phase
from push constants. Cost trivial.

**Accept.** ISO-invariant; on a dark-frame test raw, ≥95% of injected hot pixels removed,
zero visible softening on a resolution chart.

---

## P2 — Impulse (salt & pepper) median (post-demosaic)

**Purpose.** Remaining single-pixel outliers and demosaic sparkles. (RT Impulse Noise
Reduction; RT also uses median as a final cleanup for NR artifacts.)

**Algorithm.** Conditional median: compute 3×3 median per channel (or on luma only);
replace only where `|x − median| > threshold(σ)` — a plain median blurs everything, the
condition preserves texture. Threshold driven by the slider (0–100 → 0 = off).
Optional 5×5 / second iteration at slider > 70.

**WGSL.** 3×3 median via sorting network (19 comparisons/channel), shared-memory tile.
<2 ms at 24 MP.

**Accept.** Salt&pepper synthetic test: PSNR gain >10 dB at density 0.5%; texture chart
SSIM loss <0.5% with slider at 30.

---

## P3.a — Color space & VST

1. Convert linear RGB → **Y0U0V0** (3×3 matrix, `ycbcr_y0u0v0.wgsl`).
2. Apply forward **VST** per channel via 1D LUT texture built from the profile
   `(a, b) × strength` (see doc 02 §3). Channel-specific σ after stabilization ≈ 1.0 by
   construction — every downstream threshold is expressed in these units.
3. After shrinkage: exact-unbiased inverse LUT, then Y0U0V0 → RGB.

Profile source order: user-fitted profile → EXIF (camera, ISO) heuristic table →
image-based estimation (`profile.rs`, runs once per file on CPU thumbnail, ~10 ms).

---

## P3.b — À-trous wavelet shrinkage (default luma engine, only chroma engine)

**Decompose.** For level `j = 0..L−1` (L = 5 luma, 6 chroma):
```
smooth_{j+1} = conv(smooth_j, B3 kernel with 2^j spacing)   // separable 5-tap [1,4,6,4,1]/16
detail_j     = smooth_j − smooth_{j+1}
```
Ping-pong textures; details stored in a texture array.

**Per-level σ.** Noise σ propagates through the kernel: precompute `σ_j = σ · k_j` where
`k_j` are the known B3 à-trous attenuation factors (≈ .889, .200, .086, .041, .020, .010
for levels 0–5; verify numerically at init with a Monte-Carlo pass — cheap and
camera-independent).

**Shrink.** Wiener-style (preferred over soft threshold — less texture bias):
```
g = max(d_j² − (s_j·σ_j)², 0) / (d_j² + ε)
d_j' = d_j · g
```
`s_j` = per-level strength from the curve UI; simple mode maps the single
Luminance/Chrominance sliders onto a fixed curve shape (more strength at fine levels for
luma; boosted coarse levels for chroma to kill blotches — RT's fine/coarse split).

For chroma, additionally shrink toward the *local chroma mean* at the coarsest level to
remove low-frequency color mottling (LR's *Smoothness*).

**Reconstruct.** `out = smooth_L + Σ d_j'`.

**Accept.** Round-trip with all strengths 0 is bit-identical (±1 ulp). Kodak/NIND crops:
≥ RT default quality at matched settings (visual + SSIM harness, doc 07).

---

## P3.c — Non-Local Means (optional luma engine)

**Use case.** Heavy noise, smooth subjects (sky, skin, night). darktable default until
wavelet-auto took over; keep as user choice + auto-pick at very high ISO.

**Algorithm** (in VST'd Y0 only; chroma stays on wavelets):
```
for q in search window (radius R, scattered sampling at R>7):
    D = Σ_patch (Y(p+t) − Y(q+t))²  − 2σ²·|patch|      // noise-compensated distance
    w = exp(−max(D,0) / (h²·|patch|))
out(p) = (Σ w·Y(q) + w_c·Y(p)) / (Σ w + w_c)            // w_c = central pixel weight
```
Defaults: patch 3×3, R = 7, `h` from slider·σ, `w_c` maps inversely to Luminance slider
(more denoise → less original).

**WGSL strategy.** Tile-based compute: load (16+2·(R+1))² region into workgroup shared
memory as f16; each thread computes its pixel's weighted sum. At preview zoom < 50%,
run on downsampled buffer (σ scaled). Full-res only for the visible viewport tile and
export.

**Accept.** ≤ 40 ms full-res 24 MP on RTX 3060-class; matches darktable NLM output
within SSIM 0.98 on the test set.

---

## P3.d — Detail recovery

Two mechanisms, both post-shrinkage (doc 02 §7):

**1. Residual DCT (RT-style).**
- `res = Y_orig − Y_denoised` (VST domain).
- 16×16 block DCT (WGSL: two 1-D 16-point DCT passes, shared memory), hard-threshold
  coefficients `|c| < t·σ_blk`, inverse DCT.
- `Y_out = Y_denoised + recovery · res_filtered` where `recovery` = Luma Detail slider.
- Overlap blocks by 8 px with raised-cosine window to avoid block edges.

**2. Texture-mask blend (Conservative/Aggressive).**
- Texture map `T = clamp((localVar − σ²) / (k·σ²), 0, 1)` computed on a 7×7 window at
  half res, slightly blurred.
- Final: `out = mix(denoised, mix(denoised, orig, 0.5), T)` — textured regions keep up
  to 50% original. `k` differs between Conservative (higher, protects more) and
  Aggressive.

**Accept.** With Luminance 100 + Detail 60 on an ISO 6400 test image, fine text/fabric
remains legible while flat areas are clean (blind A/B against RT reference workflow).

---

## Pass ordering & fusion

```
[P3 chain, one frame]
 y0u0v0 → vst → { atrous ×L (chroma always, luma if engine=wavelet) | nlm (luma if engine=nlm) }
        → dct_residual (if luma_detail>0) → texture blend → inverse vst → rgb
```
Fuse where possible: y0u0v0+vst forward is one pass; inverse vst+rgb+final blend is one
pass. Total classical pipeline target: **≤ 15 ms** at 24 MP with wavelet engine (fits the
60 fps interaction budget together with the rest of the megashader at preview scale).

## CPU fallback

Mirror implementations in `rayon` for headless export / no-GPU machines. Share the exact
same parameter structs and LUTs; validated against GPU output in tests (tolerance 1e-3).
