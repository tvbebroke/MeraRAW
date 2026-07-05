# MeraRAW — Denoise Feature: Project Description & Technical Research

## 1. Purpose

This document defines the technical foundation for building a denoise feature in MeraRAW, using Adobe Lightroom's AI Denoise as the primary reference point (widely considered the best-in-class implementation among mainstream tools) alongside DxO PureRAW's DeepPRIME and classical noise-reduction methods. The goal is to give the team (or a solo dev) a clear, evidence-based picture of *what* denoise needs to do, *how* the best implementations do it, and *what approach MeraRAW should take*.

---

## 2. Why Photos Have Noise

Noise isn't a flaw to patch over after the fact — it's baked into the physics of capturing light. Understanding the source matters because it determines what a denoiser is actually trying to undo.

**Photon shot noise (the dominant source).** Light itself arrives at a sensor in discrete photon packets, and the exact number of photons hitting one pixel vs. its neighbor varies randomly even under perfectly even illumination. Adobe's engineering team describes this with a rain-bucket analogy: two identical buckets left out in the same rain for the same time will hold slightly different amounts of water purely due to statistical variation — that variation *is* the noise. The lower the light (higher ISO needed), the fewer photons are collected, and the worse this statistical variation gets relative to the signal.

**Sensor/read noise.** The readout and amplification circuitry on the sensor adds its own noise, most visible in shadows and worsened by heat (long exposures, video, hot ambient temperatures).

**Why it matters more today than 10 years ago:** phone and mirrorless sensors have shrunk pixel pitch while push toward ISO 25600+ has become "standard," so noise is a bigger practical problem now than when most desktop NR algorithms (including Lightroom's *pre-2023* sliders) were designed.

---

## 3. Two Families of Denoising Approaches

### 3.1 Classical (non-learned) algorithms
These operate on general noise statistics without training on photographic content.

| Method | Idea | Strength | Weakness |
|---|---|---|---|
| Gaussian / Median blur | Local averaging | Very fast | Destroys detail indiscriminately |
| Bilateral filter | Averages nearby pixels only if similar in color/intensity (edge-aware) | Preserves edges better than blur | Still smudges fine texture; halos |
| Non-Local Means (NLM) | Averages pixels from *anywhere* in the image with similar surrounding patches | Good detail/noise tradeoff | Slow; can create "waxy" look |
| Wavelet-domain shrinkage | Decomposes image into frequency bands, thresholds noise-dominated bands | Good for grain/luminance noise | Weak on chroma noise, color speckling |
| BM3D | Groups similar 3D patches, filters in transform domain | Long-time gold standard for classical denoising | Very slow, and *still* loses fine texture at high noise levels |

**Fundamental limitation:** classical methods can't distinguish "this looks like noise" from "this looks like fine texture" — grass, fabric weave, animal fur, skin pores, film grain, and start noise all resemble the local statistics of true noise. This is exactly the tradeoff photographers complain about with Lightroom's legacy Luminance/Color sliders: push them up and you get a "watercolor" or "plastic skin" look because detail-sized information is being smoothed away along with noise.

### 3.2 Deep learning (AI) approaches
These are trained on millions of examples so the network learns the *difference* between texture and noise from real photographic content, not just generic statistics.

Key research lineage relevant to MeraRAW:
- **DnCNN (2017)** — one of the first deep CNNs to significantly beat classical denoisers on real photographs by learning residual noise directly.
- **Noise2Noise / Noise2Void** — trained without clean ground truth, using pairs of independently noisy captures of the same scene; useful when perfectly clean reference images are hard to obtain.
- **Joint demosaic + denoise networks** — instead of denoising *after* the RAW mosaic has already been interpolated into RGB (baking noise-driven color/interpolation errors permanently into the image), a single network performs demosaicing and denoising together, directly on raw sensor data. This is the architecture both Adobe and DxO use, and it's the single biggest technical differentiator vs. older tools.
- **Mobile-optimized lightweight networks** (e.g., "Practical Deep Raw Image Denoising on Mobile Devices," ECCV 2020; "Lightweight network towards real-time image denoising on mobile devices," 2022) — these specifically address running such models within mobile compute/memory/battery budgets, which is directly relevant to MeraRAW if it's a mobile app. They found that FLOPs and parameter count aren't the real bottleneck on-device — **memory access cost and NPU-incompatible ops** are, which should shape model architecture choices.

---

## 4. Deep Dive: How Lightroom's AI Denoise Actually Works

Source: Adobe's own engineering write-up ("Denoise Demystified," Eric Chan, ACR/Lightroom team, with technical credit to Michaël Gharbi and Bo Sun), plus independent technical analysis of the shipped feature.

### 4.1 Core architecture: joint demosaic + denoise, done once, on raw sensor data
Camera sensors capture a single color value per photosite in a mosaic pattern (Bayer for most cameras, X-Trans for Fujifilm) — not full RGB per pixel. Traditionally, denoising happens *after* demosaicing has already interpolated the missing colors, which means any noise-driven interpolation errors (false color, moiré, "worm" artifacts especially on X-Trans) are already baked in and can't be cleanly undone.

<cite index="21-1">Adobe's Denoise feature was specifically designed and trained to perform both demosaicing and denoising in a single step, rather than as two separate passes.</cite> This single-step design is why AI Denoise avoids the mosaic-pattern artifacts that plague sequential pipelines.

### 4.2 Training data: millions of noisy/clean patch pairs, plus dark frames
<cite index="21-1">The model was trained on millions of pairs of high-noise and low-noise image patches so the network could learn how to map from one to the other</cite> — deliberately drawn from "everyday life" subject matter (bricks, branches, cloth, fabric, foliage) so the model generalizes across genres rather than overfitting to a narrow test set.

Three additional ingredients Adobe called out as essential:
1. **Noise simulation + data augmentation pipeline** — engineered so the resulting model is robust across a broad range of real-world camera/ISO/temperature conditions, not just the conditions literally present in the training set.
2. **A large "dark frame" dataset** — <cite index="21-1">images captured with the lens cap on, used to teach the model to recognize and remove fixed pattern noise in shadows, which is especially prominent on older camera sensors</cite>.
3. **Training directly on raw sensor data end-to-end** — this is what lets the same model deliver Raw Details quality "for free" as a side effect of denoising, since both problems (interpolation and noise) are being solved jointly rather than optimized separately and then composed.

### 4.3 Network type
<cite index="21-1">The underlying structure is a deep convolutional neural network — meaning what happens to a given pixel is determined by analyzing the pixels immediately surrounding it, giving the model the local context it needs to reconstruct detail rather than guess blindly.</cite>

### 4.4 Quality bar Adobe set for themselves
Their stated internal target: <cite index="21-1">deliver clean, usable results for a 20-megapixel full-frame sensor at ISO 51200</cite> — a genuinely aggressive bar, since that ISO is well beyond where classical NR sliders produce usable results on most cameras.

### 4.5 Hardware acceleration
<cite index="21-1">The models are built to take advantage of dedicated ML hardware — NVIDIA TensorCores on Windows and the Apple Neural Engine on Mac — which is what makes running a genuinely large neural network on a single photo practical in seconds rather than minutes.</cite> This is a critical implementation detail: this isn't a lightweight model that runs anywhere; it depends on modern GPU/NPU acceleration, and Adobe explicitly recommends ≥8GB of GPU memory for best performance.

### 4.6 Output & workflow model
- Historically (2023–mid-2025): running Denoise created a brand-new full-resolution DNG file (several times larger than the original) containing the denoised result — original untouched.
- As of Lightroom Classic 14.4 (June 2025): Adobe re-architected this so denoise results are stored **non-destructively inside the catalog** (`.lrcat-data` sidecar) rather than as a duplicate DNG on disk — same category of change as their AI masking/Generative Remove data. This matters for MeraRAW's design: producing giant duplicate files per denoise operation is a real user-facing storage/workflow pain point that later versions of the *best* tool in the category had to walk back.
- Only one adjustable control is exposed to the user: a single **Amount** slider (0–100, default 50), plus an automatic on/off for "Raw Details" bundled in. This simplicity — one slider instead of Lightroom's older multi-slider Luminance/Color/Detail panel — is repeatedly cited by users as a major UX win: it removes the "fiddly parameter tuning" burden classical NR always had.
- Denoise is currently restricted to Bayer/X-Trans RAW files (not JPEG, HEIC, TIFF, ProRaw, sRaw, or HDR/Pano-merged DNGs) — because the model needs genuine un-demosaiced sensor data to do its joint demosaic+denoise trick.

### 4.7 Known limitations (worth designing around)
- **Can't invent detail that was never captured.** At very high ISO with genuinely no signal left in a region, AI Denoise smooths it convincingly but can't "recover" texture that photon shot noise fully destroyed.
- **Processing time**: seconds to minutes per image depending on GPU, and it's a per-image operation (no true multi-image batch parallelism within a single job in the classic version) — a real bottleneck for high-volume shooters (event/wildlife photographers processing hundreds of files).
- **Large output size** in the pre-14.4 architecture (150–250MB DNGs cited by working photographers) — a top complaint before the catalog-based rewrite.
- **Order-of-operations sensitivity**: Adobe explicitly recommends running Denoise *before* AI masking or content-aware removal, since those tools can behave unpredictably on noisy source pixels and get invalidated/recalculated when denoise is applied afterward.

---

## 5. How Lightroom Compares to Other Leading Tools

| Tool | Where in pipeline | Approach | Strength | Tradeoff |
|---|---|---|---|---|
| **Lightroom AI Denoise** | Integrated in catalog/Develop module | Joint demosaic+denoise CNN, single Amount slider | Best-in-class balance of simplicity + quality; now non-destructive, in-catalog | Bayer/X-Trans RAW only; GPU-heavy; per-image processing time |
| **DxO PureRAW (DeepPRIME / DeepPRIME XD)** | Standalone pre-processor, runs before the file ever reaches your main editor | Also joint demosaic+denoise CNN, but positioned earlier/more aggressively in the pipeline | Frequently rated as recovering *more* fine detail at extreme ISO ("surgical" detail recovery) | Separate app/step in workflow, not integrated into one catalog |
| **Topaz Photo AI** | Post-demosaic plugin | CNN-based, marketed as detail *recovery* (sometimes near-generative at extreme ISO) | Can produce dramatic recovery on very difficult files | Can look less "natural"/over-processed on typical files; separate tool to invoke |
| Classical NR (any app's manual sliders) | Post-demosaic | Bilateral/NLM/wavelet style | Instant, no GPU dependency, fully reversible per-slider | Genuine detail/noise tradeoff — always some smoothing of real texture |

**Takeaway for MeraRAW:** the field has converged on the same core idea — *train a neural network to jointly demosaic and denoise directly on raw sensor data* — as the technique that actually breaks the classical "noise reduction vs. detail" tradeoff. The differentiators between products are less about *whether* to do this and more about (a) where in the workflow it happens, (b) how non-destructive/integrated it feels, and (c) how much compute it demands.

---

## 6. Recommendations for MeraRAW's Denoise Function

### 6.1 Decide your quality tier upfront
Two realistic paths, not mutually exclusive as phases:

**Phase 1 — Classical, ship fast:** Implement a solid bilateral or NLM-based luminance/chroma denoise as a baseline. Cheap, works on any device, no training data needed, sets a quality floor. This is roughly where Lightroom was *before* 2023.

**Phase 2 — AI joint demosaic+denoise (the actual differentiator):** This is what makes Lightroom's tool feel categorically better rather than just "a bit better." Requires:
- A CNN trained on **paired noisy/clean raw patches** (millions of pairs, per Adobe's own account) — realistically, for a smaller team this means either (a) licensing/using an open research dataset (e.g., SIDD, DND, or similar raw-noise benchmark datasets), or (b) generating synthetic noisy/clean pairs via a calibrated noise simulation pipeline (shot noise + read noise models applied to clean low-ISO captures), which is what most academic mobile-denoising papers actually do when they can't capture millions of real pairs.
- A **dark-frame dataset** specific to whichever sensors/cameras MeraRAW targets, to handle fixed-pattern shadow noise — genuinely worth budgeting capture time for if you support a limited set of camera models initially.
- Operating **on raw Bayer/X-Trans data**, before demosaicing — this is the single most important architectural decision; denoising after demosaicing will always underperform a joint model.

### 6.2 On-device vs. cloud
Given this is a RAW photo app (likely mobile), on-device inference is almost certainly the right call for privacy, offline use, and avoiding per-image cloud compute costs. Relevant technical findings from mobile-denoising research:
- The real bottleneck for on-device CNN denoising isn't FLOPs or parameter count — it's **memory access cost and operations that don't map well to mobile NPUs**. Architecture choices should be validated against actual on-device latency, not just theoretical compute cost.
- Deploy via **Core ML** (iOS) and **TensorFlow Lite / NNAPI** (Android) to get hardware acceleration on Neural Engine / mobile NPUs — mirroring Adobe's own reliance on Apple Neural Engine and NVIDIA TensorCores for feasible processing time.
- Consider a tiered strategy: a lightweight on-device model for real-time preview/most images, with an optional "Enhanced" mode (slower, better quality) for hero shots — this mirrors how photographers already use Lightroom (denoise everything lightly, reserve the heaviest tools for a handful of "keeper" images).

### 6.3 UX lessons worth copying
- **One slider, sensible default.** Lightroom's single Amount slider (default 50) is consistently praised over the old multi-slider panel. Resist the urge to expose luminance/chroma/detail as separate controls in the AI mode — offer that only in a "Manual" fallback mode for classical NR.
- **Non-destructive, in-place storage.** Avoid Lightroom's original mistake of writing a giant duplicate file per denoise operation — store denoise results as metadata/derived-state tied to the original file, computed on demand or cached, not as a permanent duplicate RAW.
- **Before/after preview**, ideally via press-and-hold or a slider comparison, before committing.
- **Run denoise early in the edit pipeline** relative to any AI-driven tools (masking, subject select, spot removal) that would otherwise be confused by noisy source pixels.
- **Let users add grain back** if the result looks unnaturally smooth — a small but well-liked touch in Lightroom (pairing Denoise with the Grain effect).

### 6.4 What NOT to expect
Per Adobe's own framing: this technology cleans up statistical noise very well, but it **cannot invent detail that the sensor never captured**. Manage expectations in your marketing/UI copy — "clean up noise while preserving real detail," not "make any photo look clean at any ISO."

---

## 7. Evaluation & Testing Plan

- **Quantitative metrics:** PSNR and SSIM against clean low-ISO reference captures of the same scene (standard in the denoising research literature — e.g., referenced results of ~35 PSNR / ~0.85 SSIM for CNN-based denoisers vs. weaker autoencoder baselines).
- **Qualitative test set:** deliberately mirror Adobe's own test categories — wildlife/sports (high ISO + motion), low-light indoor/concert scenes, astro/night sky (color noise in near-black regions), and low-ISO shadow recovery (noise isn't only a high-ISO problem).
- **Stress test at extreme ISO** (e.g., ISO 25,600–51,200 equivalent) since that's where classical sliders visibly fail and where the AI approach needs to prove its value.
- **Cross-device latency benchmarking** on your actual target device tier (not just flagship phones), since NPU support varies widely.

---

## 8. Summary

The reason Lightroom's Denoise is widely regarded as best-in-class isn't a single clever trick — it's the combination of (1) operating jointly on demosaicing + denoising directly on raw sensor data instead of post-hoc, (2) training on a very large, deliberately diverse and augmented noisy/clean dataset including dedicated dark-frame data, (3) genuine hardware acceleration (Neural Engine/TensorCores) making it fast enough to be usable, and (4) a deliberately minimal, one-slider UX. For MeraRAW, the technically correct path to a comparable feature is a joint demosaic+denoise CNN trained on raw sensor patches (real or noise-simulated pairs), deployed on-device via Core ML/TFLite with NPU acceleration, wrapped in the simplest possible UI.
