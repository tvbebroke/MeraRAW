# MeraRAW image pipeline — technical specification

**Version:** 0.1.4 · **Working space:** scene-referred linear Rec.2020 (D65)
**Scope:** the full decode → demosaic → colour → grade → export path as shipped.

Everything downstream of decode runs on the GPU (`wgpu` → Metal) in a
**scene-referred linear Rec.2020, D65** working space. Highlight headroom
(values > 1.0) is preserved until the display/export transform.

```
 FILE
  ├── RAW
  │    rawler unpack → merawler demosaic → as-shot WB → DCP/cam matrix
  │    → linear Rec.2020 working master (scene-referred)
  │    → edit graph → present (Neutral / Camera+DCP / Filmic / Original)
  │
  └── JPEG / PNG / TIFF / …  (ImageKind::Rendered)
       decode encoded RGB → embedded ICC or sRGB fallback → EOTF
       → linear Rec.2020 working master
       → edit graph (user edits only; no demosaic / DCP / sensor WB)
       → present look 3 (Rec.2020→sRGB + OETF) unless user picks Filmic
```

`ImageKind` is the type-level split. RAW-only stages (`allows_raw_only_stages`)
cannot run on rasters: DCP autoload, demosaic picker, and the Camera punchy
view-look are gated on `kind == Raw`. A zero-edit JPEG must stay faithful to
the decoded source — the default Camera look (gain 1.6 / sat 1.22) is a RAW
rendering, not a JPEG display transform.

Trace: `MERARAW_PIPELINE_TRACE=1` logs per-stage min/max/mean/clip%.

Two execution paths share one node graph. Interactive runs at viewport
resolution and caches the chain by a "dirty-from" index (only nodes at/after a
changed parameter re-run). Export runs full-resolution and **tiled** to bound
memory.

---

## 1. Decode & demosaic

### 1a. Container decode / levels / crop — `rawler` (external open source)

`rawler` (pure-Rust RAW decoder from the **dnglab** project) handles container
parsing (ARW/CR3/NEF/DNG/RAF/ORF/RW2/…), sensor unpacking, black/white-level
rescale to a normalized `[0,1]` mosaic, CFA-pattern identification, active-area /
default-crop rectangles, and EXIF/metadata. Used as a dependency, unmodified.
We deliberately do **not** use rawler's own white-balance / sRGB / demosaic
steps — only its `Rescale` + crop machinery.

### 1b. Demosaic — **merawler** (our engine)

The rescaled single-channel mosaic is demosaiced by **merawler**, the custom
engine we built and wired into the editor (`merawler::demosaic(&CfaImage,
algo)`). It is **user-selectable per image** (darktable-style) and **re-decodes
the RAW on change**. Default algorithm = **RCD**.

Data flow:
`rawler Rescale → normalized full-sensor mosaic → merawler CfaImage → demosaic
(full sensor) → crop to rawler's output rect → camera-native linear RGB`.
Non-Bayer sensors (Fujifilm X-Trans, Apple ProRAW linear DNG, 4-colour CFA)
fall back to rawler's built-in demosaic.

**All seven algorithms are original, clean-room Rust implementations** written
from the *published algorithms* (coefficients/offsets are mathematical facts) —
**not line-by-line ports** of the reference C/C++. Where a reference is GPL we
read only the numeric coefficients and wrote original code, keeping the engine
clear of copyleft. Provenance & citations:

| Algorithm | Origin | Reference consulted | Citation |
|---|---|---|---|
| **Bilinear** | ours (baseline / edge fallback) | — | standard |
| **Malvar** | ours, clean-room | canonical kernels | Malvar, He & Cutler, "High-quality linear interpolation for demosaicing of Bayer-patterned color images," *ICASSP* 2004 |
| **RCD** | ours, clean-room | `librtprocess` `rcd.cc` (GPL-3.0) — coefficients only | Luis Sanz Rodríguez, "Ratio Corrected Demosaicing," 2015 |
| **LMMSE** | ours, clean-room | `librtprocess` `lmmse.cc` (GPL-3.0) | Zhang & Wu, "Color demosaicking via directional LMMSE estimation," *IEEE TIP* 14(12), 2005 |
| **AMaZE** | ours, clean-room (full tiled port) | `librtprocess` `amaze.cc` (GPL-3.0) | Emil Martinec, "AMaZE — Aliasing Minimization and Zipper Elimination," 2010 (RawTherapee) |
| **IGV** | ours, clean-room | `librtprocess` `igv.cc` (GPL-3.0) | L. Sanz Rodríguez, "Integrated Gaussian Vector," 2013, using high-order interpolation of Li, S. & Randhawa |
| **DDFAPD** | ours, clean-room | `colour-demosaicing` `menon2007.py` (**BSD-3-Clause**) | Menon, Andriani & Calvagno, "Demosaicing with directional filtering and a posteriori decision," *IEEE TIP* 16(1), 2007 |

**Validation:** Kodak-24 CPSNR — IGV 39.7, AMaZE 39.1, DDFAPD 39.1, LMMSE 38.4,
RCD 36.9, Malvar 35.6, bilinear 30.2 dB (matches published figures for these
algorithms; strong evidence the ports are faithful). A line-by-line audit
against source found and fixed two RCD bugs (inverted directional `intp` blend;
`lpf` ratio offset).

**Characteristics (for picking a default / per-shot):** RCD = darktable default,
balanced, artifact-minimizing; LMMSE = best on noisy/high-ISO; AMaZE = maximum
detail (landscape/astro), slowest; IGV = best false-colour suppression on smooth
aliasing but fringes on hard colour edges; Malvar/bilinear = fast previews.

---

## 2. Colour landing (camera → working space)

CPU-side at decode, turning camera-native RGB into linear Rec.2020:

1. **As-shot white balance** — green-normalized camera multipliers `[r/g, 1, b/g]`.
2. **Camera → Rec.2020 matrix** — **DNG dual-illuminant** model (our
   implementation of the Adobe DNG spec §5.4): rawler's per-illuminant
   `ColorMatrix` entries → camera→XYZ, interpolated by estimated **CCT** between
   the two calibration illuminants (e.g. Standard-A ≈ 2856 K and D65), then
   XYZ(D65) → Rec.2020. CCT is estimated from the as-shot WB.
3. **Optional DCP profile** — if a `.dcp` is loaded, its ForwardMatrix /
   ColorMatrix replaces the base matrix. Our DCP parser reads the raw TIFF-IFD
   tags directly (no external DCP library).
4. **Orientation** baked; headroom preserved (only negatives clamped).

Output → the **working master** (contract A4): linear Rec.2020, D65,
scene-referred.

---

## 3. Edit render graph (GPU, WGSL — all ours)

Fixed node order (`graph/config.rs`). Each is a hand-written WGSL shader,
identity at defaults, skipped when so.

| Slot | Node | Space | Operation |
|---|---|---|---|
| 1 | **Exposure** | linear Rec.2020 | `out = rgb · 2^stops` (`color_matrix.wgsl`, M = I) |
| 2 | **White balance** | linear Rec.2020 | Temp/Tint via **Bradford** chromatic adaptation as a Rec.2020 matrix (Planckian locus → xy → adaptation). Bradford cone matrix per Lam (1985) |
| 3 | **Calibration** | linear Rec.2020 | Lightroom "Calibration panel" equivalent — secondary per-primary hue/sat matrix on the P1 base + `shadow_tint` low-luma chroma offset (`calibration.wgsl`) |
| 4 | **Detail / noise** | luma + chroma | Luma: 5×5 **bilateral** (range σ from strength, `detail_preserve`-gated); chroma: 5×5 gaussian on chroma offsets. Runs **before** sharpen (`noise.wgsl`) |
| 5 | **Colour grade** | **Oklab** / LMS / linear | 3-way wheels (shadows/mid/high), **three injection models**: `0` Perceptual — constant-hue Oklab chroma moves; `1` Classic — additive lift/gain in linear RGB (Resolve-style crosstalk); `2` Light — von Kries multiply in LMS cone space. Shared global-chroma + perceptual-sat + gamut-compress finish (`grade.wgsl`) |
| 6 | **HSL / colour mix** | **Oklab LCh** | 8 overlapping hue bands, smooth gaussian weights (no banding), **chroma-gated** so neutrals don't shift; orange (skin) band narrowed for surgical skin moves (`hsl.wgsl`) |
| 7 | **Tone curve** | Rec.2020 | Master RGB-luma curve (constant-hue) + optional per-channel **R/G/B** curves; each a 1-D LUT `[luma\|r\|g\|b]` (`curve.wgsl`) |
| 8 | **3-D LUT** | sRGB-encoded | Real `.cube` look LUTs (our `CubeLut` parser, 3-D only, ≤ 65³, trilinear); encode→trilinear→decode so identity is exact; `lut.opacity` param (`lut.wgsl`) |
| 9 | **Sharpen** | luma only | Unsharp mask on luma only (no chroma fringing), low-contrast-gated by `detail`; **after** tone shaping (`sharpen.wgsl`) |

**Perceptual space:** `color_grade` and `hsl` operate in **Oklab** (Björn
Ottosson, 2020) — hue-uniform, so chroma/hue moves stay clean. Rec.2020 ↔ Oklab
matrices are computed at load, not hard-coded.

**Local adjustments / masks:** each mask runs a scoped copy of the module stack
and composites `out = mix(base, local, mask)` (`blend.wgsl`), where the mask
already carries opacity / invert / feather.

**Camera-profile "look"** (distinct from the base matrix): if the DCP provides
them, `ProfileHueSatMap` (dual-illuminant CCT blend) → profile tone curve →
baseline-EV → `ProfileLookTable`, applied as a GPU pass in **linear ProPhoto**,
mirroring Adobe's DCP look semantics (`dcp_look.wgsl`).

**Present / view transform** (display side, not baked into the edit):
Rec.2020 → sRGB gamut → pinned neutral view transform (**Reinhard-extended**,
Lw = 4; Reinhard et al. 2002) → sRGB OETF. A **Filmic** option uses **Minimal
AgX** (Troy Sobotka's AgX), so Filmic preview == Filmic export (`present.wgsl`).

---

## 4. AI masking / segmentation (contract B2 — ours; external model)

On-device, offline, private (`segment.rs`):

- **Subject/person masks** — **U²-Net (u2netp)** via **`tract-onnx`** (pure-Rust
  inference, no FFI / runtime download); the 4.5 MB model is embedded with
  `include_bytes!`. Input squash-resized to 320². *Qin et al., "U²-Net: Going
  Deeper with Nested U-Structure for Salient Object Detection," Pattern
  Recognition, 2020.*
- **Sky** — spectral heuristic (dedicated sky model slots behind the same trait).
- **Object-by-point** — colour-similarity region-grow seeded at the click point.

---

## 5. Export & colour management (`export.rs` — ours)

Full-res tiled render → view/OETF → resize-aware sharpen → encode + ICC + EXIF.

- **Target spaces:** sRGB, Display P3, Adobe RGB, ProPhoto (ROMM). sRGB/P3 share
  the sRGB curve; ProPhoto uses its D50-referred matrix (ISO 22028-2 /
  Lindbloom) with a **Bradford D65→D50** adaptation of the working white and the
  ROMM transfer (linear toe below 1/512, γ 1.8).
- **Formats:** JPEG, PNG, TIFF-16 (pure-Rust), HEIC (via macOS `sips`).
- **Colour-managed everywhere:** the matching **ICC profile is embedded** (JPEG
  APP2 / PNG iCCP / TIFF tag 34675 / HEIC via sips). **EXIF written**
  (hand-rolled little-endian TIFF) unless `strip_metadata` is set.

---

## Provenance summary

**Ours (original code):** the entire **merawler** demosaic engine (7 algorithms,
clean-room), the DNG dual-illuminant colour landing, the DCP parser + GPU look
port, **every render-graph WGSL node** (exposure, WB/Bradford, calibration,
noise, Oklab colour-grade, Oklab-LCh HSL, tone curves, 3-D-LUT engine, sharpen,
masks, present/view-transforms), and the export colour-management / ICC / EXIF
pipeline.

**External open source (dependencies):** `rawler` / dnglab (container decode +
levels + crop), `tract-onnx` (inference runtime) + the U²-Net model, `wgpu`
(GPU abstraction), the image codec crates (`image`, `png`, `tiff`), and macOS
`sips` (HEIC).

**Referenced but not copied:** the demosaic algorithm papers; the GPL
`librtprocess` and BSD `colour-demosaicing` sources (coefficients / verification
only); Oklab (Ottosson 2020); AgX (Sobotka); Bradford (Lam 1985); ROMM/ProPhoto
(ISO 22028-2 / Lindbloom); the Adobe DNG dual-illuminant model.
