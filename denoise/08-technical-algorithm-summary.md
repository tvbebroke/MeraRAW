# MeraRAW Denoise — Full Technical Algorithm Summary

**Purpose.** One document that describes *exactly* what MeraRAW implements today (code in `src-tauri/core/src/denoise/` + `graph/noise.wgsl`), with enough math and citations that you can verify each claim against papers / darktable / RawTherapee / Adobe write-ups.

**Status of this doc.** Implementation snapshot as of MeraRAW 0.1.5 denoise land. Where the GPU path *deviates* from the CPU reference (or from textbook à-trous), that deviation is called out explicitly — those are the places most likely to disagree with “what you read online.”

---

## 1. Design pedigree (what we borrow)

| Source | Idea we use |
|---|---|
| **Sensor physics / Adobe Denoise write-ups** | Shot (Poisson) + read (Gaussian) noise; denoise early in linear domain |
| **darktable *denoise (profiled)*** | Poisson–Gaussian profile `(a,b)`, variance-stabilizing transform, Y0U0V0, NLM + wavelet engines, Strength = profile scale |
| **RawTherapee** | Multi-stage chain (impulse → wavelet → detail recovery), chroma auto, Conservative/Aggressive texture mask |
| **Lightroom / ACR** | Separate AI track with Amount blend; manual Luminance/Color/Detail as refinement |
| **Makitalo & Foi** | Exact unbiased inverse of the generalized Anscombe transform |
| **Starlet / à-trous (Holschneider, Starck–Murtagh)** | Undecimated B3-spline wavelet shrinkage |

We do **not** claim bit-identical output to any of those apps. The chain is deliberately “same family.”

---

## 2. Pipeline placement

### 2.1 Ideal order (design)

```
RAW mosaic
  → [P0] hot/dead pixel (same-CFA median)     ← raw domain
  → demosaic
  → [P2] impulse median (optional)           ← post-demosaic RGB
  → [P3] classical denoise (this doc)        ← linear working RGB, before tone
  → exposure / WB / grade / curves / LUT
  → sharpen                                  ← always AFTER denoise
```

AI track (when wired end-to-end): produces a cached denoised base that replaces or blends under the classical path; Amount is a post-inference blend.

### 2.2 What ships today in the render graph

Fixed module order in `graph/config.rs`:

```
exposure → white_balance → calibration → noise(detail) → color_grade → hsl
  → tone_curve → lut → sharpen(detail)
```

So classical denoise runs **after** exposure/WB/calibration, still in linear Rec.2020-ish working space, **before** tone/LUT/sharpen. That is later than darktable’s raw-domain profiled denoise, but still before tone mapping (the important part for VST validity).

**P0 hot pixels** run in the merawler demosaic path on the Bayer mosaic (always-on for that path), not as a graph node.

---

## 3. Noise model

### 3.1 Poisson–Gaussian

For a linear pixel value \(z\) (post black-level, in normalized working units):

\[
\operatorname{Var}(z) = a\,z_{\text{true}} + b
\]

- \(a\) — shot-noise gain (∝ ISO / conversion gain)
- \(b\) — read-noise variance floor (∝ ISO² roughly for ISO-less amplification models)

Code: `NoiseProfile { a, b, source }` in `profile.rs`.

**Sigma at level \(x\):**

\[
\sigma(x) = \sqrt{\max(a\,x + b,\, 0)}
\]

### 3.2 Profile sources (priority)

1. **Measured** — image-based fit (`estimate_from_image`)
2. **ISO seed** — `NoiseProfile::from_iso(iso)` heuristic:
   \[
   r = \frac{\max(\mathrm{ISO}, 25)}{100},\quad
   a = 1.7\times 10^{-5}\,r,\quad
   b = 2.5\times 10^{-9}\,r^{2}
   \]
   (generic full-well / read-noise ballpark — not a per-camera darktable profile)
3. **Default** — mid-ISO guess \((a,b) \approx (1.4\times 10^{-4},\, 2.0\times 10^{-7})\)

**Strength** \(S\) (UI `detail.nr_strength`, default 1.0, range 0.25–4) scales the profile as:

\[
a' = a\,S^{2},\quad b' = b\,S^{2}
\]

so \(\sigma' = S\,\sigma\). This matches darktable’s “Strength” semantics (scale assumed noise, not output blend).

### 3.3 Image-based `(a,b)` estimation

Algorithm in `estimate_from_image` (CPU, ~thumbnail / small_cpu):

1. Tile into \(16\times 16\) blocks; subsample if too many blocks.
2. Per block & channel: mean, variance, gradient energy.
3. Keep flattest ~25% of blocks (low gradient).
4. Bin by mean; per-bin median variance.
5. Least-squares fit \(\mathrm{var} = a\cdot\mathrm{mean} + b\) on bin medians.
6. Clamp \(a,b > 0\).

Acceptance target (design): estimated \(\sigma\) within ±20% of truth on ISO ladders.

---

## 4. Variance-stabilizing transform (VST)

### 4.1 Forward generalized Anscombe (GAT)

With profile \((a,b)\), \(a > 0\):

\[
f(z) = 2\sqrt{\frac{z}{a} + \frac{3}{8} + \frac{b}{a^{2}}}
\]

Degenerate pure-Gaussian (\(a \approx 0\)):

\[
f(z) = \frac{z}{\sqrt{b}}
\]

After a correct GAT, additive noise in the stabilized domain has \(\sigma \approx 1\) (approximately, for moderate counts).

**Code note.** Doc 02 sometimes writes \((2/a)\sqrt{a z + \cdots}\); algebraically equivalent forms exist. Ours matches the common “\(2\sqrt{z/a + 3/8 + b/a^2}\)” form used in many implementations.

### 4.2 Exact unbiased inverse (Makitalo & Foi)

Algebraic inverse of the forward map is **biased dark** in shadows when applied to \(\mathbb{E}[f(z)]\) (the quantity a perfect denoiser returns). We use the closed-form unbiased series (code comment: Makitalo & Foi), with Gaussian variance term \(\sigma^{2} = b/a^{2}\) subtracted:

\[
\begin{aligned}
d &\leftarrow \max(d,\, 0.5)\\
\hat{z} &= a\Big(
  \tfrac{d^{2}}{4}
  + \tfrac{1}{4}\frac{\sqrt{3/2}}{d}
  - \frac{1.375}{d^{2}}
  + 0.625\frac{\sqrt{3/2}}{d^{3}}
  - 0.125
  - \sigma^{2}
\Big)_{+}
\end{aligned}
\]

Algebraic inverse (tests / identity checks only):

\[
\hat{z}_{\mathrm{alg}} = a\Big(\big(\tfrac{d}{2}\big)^{2} - \tfrac{3}{8} - \tfrac{b}{a^{2}}\Big)_{+}
\]

**Verify online against:** Makitalo & Foi, “Optimal Inversion of the Generalized Anscombe Transformation for Poisson-Gaussian Noise” (IEEE TIP / related tech reports). Constant coefficients may differ slightly by paper revision; compare structure (powers of \(1/d\), \(\sigma^{2}\) subtract, scale by \(a\)).

### 4.3 Pinned ordering vs some docs

**We apply VST per RGB channel *before* the Y0U0V0 rotation**, then denoise in stabilized Y0U0V0.

Reason: chroma axes can be negative; \(\sqrt{\cdot}\) in a VST on signed chroma is ill-posed. Stabilizing RGB first keeps post-rotation channel noise scales analytic:

| Channel | Approx. σ after unit-variance RGB VST |
|---|---|
| Y0 | \(1/\sqrt{3} \approx 0.577\) |
| U0 | \(1/\sqrt{2} \approx 0.707\) |
| V0 | \(\sqrt{6}/4 \approx 0.612\) |

Constants: `SIGMA_Y0`, `SIGMA_U0`, `SIGMA_V0` in `cpu.rs` / `noise.wgsl`.

Some darktable write-ups stabilize after color transform with channel-specific handling — different but related. Ours is the RGB-then-rotate variant.

---

## 5. Color space: Y0U0V0

Opponent-style luma/chroma used by darktable-style denoise (not Rec.709 luma, not IPT):

**Forward:**

\[
\begin{aligned}
Y_0 &= \frac{R+G+B}{3}\\
U_0 &= \frac{R-B}{2}\\
V_0 &= \frac{R - 2G + B}{4}
\end{aligned}
\]

**Inverse:**

\[
\begin{aligned}
G &= Y_0 - \tfrac{4}{3} V_0\\
R &= Y_0 + \tfrac{2}{3} V_0 + U_0\\
B &= Y_0 + \tfrac{2}{3} V_0 - U_0
\end{aligned}
\]

Exact inverse (unit-tested).

---

## 6. Classical chain (CPU reference = ground truth)

Entry: `denoise_rgb(rgb, w, h, &ChainParams)` in `cpu.rs`.

```
RGB
 → [optional] conditional 3×3 impulse median          (P2)
 → per-channel VST
 → RGB → Y0U0V0
 → à-trous Wiener shrinkage (chroma always; luma if wavelet)
 → [optional] NLM on Y0 only (replaces luma wavelet result)
 → detail recovery + texture-mask blend
 → Y0U0V0 → RGB
 → unbiased inverse VST
 → RGB out
```

Identity short-circuit when all luma/chroma strengths and impulse are 0.

### 6.1 P2 — Conditional impulse median

Per RGB channel, \(3\times 3\) neighborhood:

\[
m = \mathrm{median}(\mathcal{N}),\quad
\text{replace center if } |c - m| > k\,\sigma(m)
\]

- Slider `impulse` 0–100 maps to \(k\) from \(6\) (timid) down to \(1.5\) (eager); 0 disables.
- Plain median blurs; the \(\sigma\)-gate is what preserves texture (RT Impulse Noise Reduction idea).

### 6.2 P0 — Hot / dead pixel (mosaic)

On Bayer mosaic, for each site compare to 4 same-CFA neighbors at ±2:

\[
m = \mathrm{median}(n_i),\quad
d = \max n_i - \min n_i,\quad
\text{replace if } |x-m| > t\,\max(d,\,\varepsilon\cdot\mathrm{white})
\]

Default \(t \approx 3.5\)–\(4\), \(\varepsilon = 0.002\). Runs in merawler path before demosaic.

### 6.3 À-trous (starlet) wavelet shrinkage — CPU (true cascade)

**B3-spline kernel** (separable), taps:

\[
\tfrac{1}{16}[1,\,4,\,6,\,4,\,1]
\]

At level \(j\), tap spacing is \(2^{j}\) (à-trous / “with holes”).

**Cascade (textbook undecimated starlet):**

\[
\begin{aligned}
s_{0} &= \text{input}\\
s_{j+1} &= s_{j} * h_{j} \quad\text{(B3 with step }2^{j}\text{)}\\
d_{j} &= s_{j} - s_{j+1}\\
d_{j}' &= g(d_{j};\, t_{j})\, d_{j}\\
\text{out} &= s_{L} + \sum_{j} d_{j}'
\end{aligned}
\]

Levels: **5 luma**, **6 chroma**.

**Wiener-style gain** (preferred over soft threshold — less texture bias):

\[
g(d; t) = \frac{\max(d^{2} - t^{2},\, 0)}{d^{2} + \varepsilon}
\]

**Threshold** on channel \(c\) at level \(j\):

\[
t_{c,j} = s_{c,j}\cdot \sigma_{c}\cdot \kappa_{j}\cdot \sigma_{\mathrm{scale}}
\]

where:

- \(s_{c,j}\) — user strength from slider × fixed curve × optional per-level curve mul  
- \(\sigma_{c}\) — `SIGMA_Y0/U0/V0`  
- \(\kappa_{j}\) — `ATROUS_SIGMA[j]` noise attenuation through the B3 pyramid  
- \(\sigma_{\mathrm{scale}}\) — preview downscale factor (≤ 1)

**Measured `ATROUS_SIGMA` (white-noise Monte Carlo, our kernel + clamp-to-edge):**

| Level \(j\) | \(\kappa_j\) |
|---|---|
| 0 | 0.8907 |
| 1 | 0.2007 |
| 2 | 0.0855 |
| 3 | 0.0531 |
| 4 | 0.0265 |
| 5 | 0.0133 |

Literature often quotes ≈ `0.889, 0.200, 0.086, 0.041, 0.020, 0.010` for ideal infinite-support / periodic cases. Our coarser levels differ slightly because of **finite image + clamp-to-edge**; we ship the measured table and unit-test it.

**Simple slider → curve shapes** (multipliers before user curve):

- Luma curve emphasis on fine scales: `[1.0, 0.95, 0.85, 0.70, 0.50, 0.30]`
- Chroma emphasis on coarse blotches: `[0.85, 0.95, 1.0, 1.0, 1.0, 1.0]`
- Slider 100 → max \(s\) of **2.5 σ (luma)** / **3.5 σ (chroma)** at curve weight 1

**Perfect reconstruction:** all \(s = 0\) ⇒ identity (tested).

### 6.4 GPU wavelet path — important deviation

`noise.wgsl` fits the chain in **one compute dispatch**. It does **not** cascade \(s_{j+1} = \mathrm{smooth}(s_j)\). Instead, each level’s smooth is a dilated B3 of the **source** YUV, and details are successive differences of those smooths (parallel multi-scale / “lazy starlet” approximation).

| | CPU | GPU (preview/export graph) |
|---|---|---|
| Pyramid | True cascade | Parallel dilated smooths of source |
| Role | Numeric GT, bench, AI stand-in | Interactive + tiled export |

If you compare GPU frames to textbook à-trous or to our CPU path, expect **close but not identical** wavelet behavior. Threshold/Wiener math is the same family.

### 6.5 Non-local means (optional luma engine)

When engine = NLM (or Wavelet Auto at high relative mid-gray noise):

- Operates on **stabilized Y0 only**; chroma stays wavelet.
- Patch radius default 1 (3×3), search radius default 5.
- Noise-compensated patch distance:

\[
D = \sum_{\mathrm{patch}}(Y_p - Y_q)^{2} - 2\sigma^{2}\,|\mathrm{patch}|
\]

\[
w = \exp\!\big(-\max(D,0) / (h^{2}\,|\mathrm{patch}|)\big)
\]

- Scattered sampling beyond radius 3 (skip 3 of 4 far samples; weight ×4).
- Central pixel weight `nlm_center` maps from UI (more denoise → less original).
- Filtering strength \(h\) from luminance slider: \(h = 0.6 + 1.2\,L\) (in σ units).

This is the **Buades–Coll–Morel NLM** family as used in darktable’s NLM path (not BM3D).

### 6.6 Detail recovery + texture mask

**Pinned deviation from design doc 04 §P3.d:** we do **not** ship the RT-style residual block-DCT pass. Instead:

1. **σ-gated residual add-back on Y0** (Detail slider):
   \[
   r = Y_{\mathrm{orig}} - Y_{\mathrm{den}},\quad
   \mathrm{gate} = \mathrm{smoothstep}(1.5,\,4,\, |r|/\sigma),\quad
   Y \leftarrow Y_{\mathrm{den}} + \mathrm{detail}\cdot r\cdot\mathrm{gate}
   \]

2. **Texture-mask blend** (Conservative \(k=3\) / Aggressive \(k=1.5\)):
   \[
   T = \mathrm{clamp}\!\Big(\frac{\mathrm{localVar}_{5\times5} - \sigma^{2}}{k\,\sigma^{2}},\,0,\,1\Big)
   \]
   \[
   \mathrm{out} = \mathrm{den} + T\cdot 0.5\cdot(\mathrm{orig} - \mathrm{den})
   \]

Same *slider intent* as RT Detail + Conservative/Aggressive; different residual filter.

---

## 7. Parameter mapping (UI → math)

| UI control | Sidecar path | Maps to |
|---|---|---|
| Luminance | `detail.noise_luma` | Luma wavelet/NLM strength 0–100 |
| Detail | `detail.detail_preserve` | Recovery gain 0–100 (name historical) |
| Color | `detail.noise_chroma` | Chroma wavelet strength |
| Color Auto | `detail.chroma_auto` | When engaged, chroma ← \(f(\sigma_{\mathrm{mid}})\approx 800\cdot\sigma(0.18)/0.18\) capped 80 |
| Strength | `detail.nr_strength` | Profile scale \(S\) |
| Impulse | `detail.impulse` | Conditional median \(k\) |
| Engine | `detail.nr_engine` | 0 Wavelet Auto / 1 Wavelet / 2 NLM |
| Aggressive | `detail.nr_aggressive` | Texture \(k\): 3.0 vs 1.5 |
| Hot pixels | `detail.hot_pixels` | Intended decode toggle (mosaic path currently always-on for merawler) |
| AI Amount / Enable | `detail.ai_amount`, `detail.ai_enabled` | AI track |

`ChainParams::from_sliders` is the single flatten point into GPU uniforms / CPU chain.

---

## 8. AI track (as implemented)

Architecture (doc 03/05):

- Tiled inference, 512² tiles, 64 px overlap, cosine weights (`tiler.rs`).
- Prefer **full-strength denoise then Amount blend** so Amount is real-time after one run.
- Disk cache under app support, content-addressed (blake3 of pixels + model id + amount bucket).
- Single-flight job manager; per-job cancel.

**Current inference:** ONNX/UNet not loaded yet. Jobs run the **classical CPU chain as a stand-in** (`stand_in: true` in model registry when `.onnx` absent). When a real nind-style UNet is installed, `ready` flips and stand-in is disabled for that model.

This matches RapidRAW’s “WGSL manual + optional nind-denoise” split in spirit, not weights.

---

## 9. End-to-end formulas (one pixel, classical)

Let \(S\) = Strength, \(L,C,D,I\) = Luma/Color/Detail/Impulse sliders in \([0,1]\).

1. Impulse (if \(I>0\)): conditional median with \(k = 6 - 4.5 I\).
2. \(R',G',B' = f_{\mathrm{VST}}(R,G,B;\, aS^{2}, bS^{2})\).
3. \((Y,U,V) = \mathrm{Y0U0V0}(R',G',B')\).
4. Wavelet or NLM → \((Y_d, U_d, V_d)\).
5. Recovery + texture → \((Y_f, U_f, V_f)\).
6. \((R'',G'',B'') = \mathrm{Y0U0V0}^{-1}(Y_f,U_f,V_f)\).
7. Output \(= f^{-1}_{\mathrm{unbiased}}(R'',G'',B'')\).

---

## 10. What to verify against online sources

Use this checklist when reading papers / dt / RT docs:

| Claim | Where to verify | Our code stance |
|---|---|---|
| \(\mathrm{Var}=a x+b\) | darktable noise profiling docs; Foi et al. | ✅ Implemented |
| Generalized Anscombe forward | Makitalo & Foi; Wikipedia Anscombe; dt VST | ✅ \(2\sqrt{z/a+3/8+b/a^2}\) |
| Unbiased inverse needed | Makitalo & Foi; dt “bias correction” history | ✅ Closed form (coeff check vs paper) |
| Denoise before tone/sharpen | Universal RAW practice | ✅ Before tone/LUT/sharpen; after WB in current graph |
| Y0U0V0 | darktable denoise (profiled) source | ✅ Exact matrices above |
| À-trous B3 `[1,4,6,4,1]/16` | Starck–Murtagh starlet; dt wavelet | ✅ CPU cascade; GPU parallel approx |
| Wiener shrink \( (d^2-t^2)_+/d^2 \) | Common wavelet denoising; dt variants | ✅ |
| NLM noise-compensated distance | Buades et al.; dt NLM | ✅ Y0 only |
| RT residual DCT detail recovery | RT NR docs | ❌ Replaced by σ-gated residual + texture mask |
| Joint demosaic+denoise AI | Adobe Enhance Details / Denoise | ⏳ Scaffold only (stand-in classical) |

---

## 11. Code map

| Path | Role |
|---|---|
| `core/src/denoise/profile.rs` | `(a,b)`, VST forward/inverse, image estimate |
| `core/src/denoise/cpu.rs` | Classical reference chain + hot pixel + tests |
| `core/src/denoise/settings.rs` | Sidecar ↔ `ChainParams` |
| `core/src/denoise/metrics.rs` | PSNR/SSIM, synthetic noise |
| `core/src/denoise/ai/*` | Jobs, tiler, cache, model registry |
| `core/src/graph/noise.wgsl` | GPU classical pass |
| `core/src/graph/config.rs` | Uniform packing from `EditDoc` |
| `core/examples/denoise_bench.rs` | Synthetic ISO ladder report |

Bench:

```bash
cargo run -p meratech-core --example denoise_bench
```

---

## 12. Known limitations (honesty for verification)

1. **No per-camera darktable noise profile DB** yet — ISO heuristic + image fit only.  
2. **GPU ≠ CPU wavelet pyramid** (cascade vs parallel).  
3. **No residual DCT** — Detail slider is σ-gated add-back.  
4. **AI UNet weights not shipped** — stand-in classical until ONNX + digest verify.  
5. **Denoise sits after exposure/WB** in the live graph, not on mosaic (except P0 hot pixels).  
6. **Chroma “auto”** only engages after the user touches NR (preserves identity-at-default for the skip-node graph).

---

## 13. Primary references (for your online check)

1. Makitalo & Foi — *Optimal Inversion of the Generalized Anscombe Transformation for Poisson-Gaussian Noise*.  
2. Starck, Fadili, Murtagh — starlet / undecimated wavelet transforms.  
3. Buades, Coll, Morel — *A non-local algorithm for image denoising* (CVPR 2005).  
4. darktable manual — *denoise (profiled)*, noise profiling, VST notes.  
5. RawTherapee — Noise Reduction / Impulse / Detail recovery documentation.  
6. Adobe — Lightroom Denoise / Enhance technical overviews (Amount semantics, early raw processing).  
7. Foi et al. — practical Poisson–Gaussian noise estimation from a single image (profile fitting lineage).

---

*End of technical summary. If a formula in §4–§6 disagrees with a primary source, treat the source as ground truth for intent and file a code bug — this doc describes the shipped math, not an external standard.*
