# Phase 03 — RAW Decoding Pipeline

**Goal:** Turn a camera RAW file into a linear, white-balanced, demosaiced image in your working color space — the hardest and most differentiating part of a RAW editor.

**This phase has a strategic fork with real license consequences.**

| Option | What | License | Closed-commercial verdict |
|---|---|---|---|
| A. Own reader | Write your own RAW parser + demosaic | yours | 🟢 max freedom, max work |
| B. Embed `rawler` (dnglab) | Modern parser, pixels+metadata, powers RapidRAW | LGPL-2.1 | 🟡 usable **with linking obligations** |
| B'. Embed `rawloader`+`imagepipe` (pedrocr) | Classic reader + pipeline | LGPL-2.1 / LGPL-3.0 | 🟡 same caveats |
| C. Buy `zenraw` (imazen) | Cleanest API, linear-f32 out, swappable demosaic | AGPL **or paid commercial** | 💰 legal for closed only if you pay |

**Recommendation:** use **A** (own reader) for the product, and use **B/B'** as *correctness references* (Learn, not Embed). If you embed a 🟡 crate, you must engineer the LGPL linking boundary (see risks). Use **C**'s 60-day trial purely to benchmark your color/demosaic quality.

---

## Architecture — the RAW pipeline stages (order matters)

Getting the stage order wrong is the #1 cause of color bugs. Reference order (validated against `imagepipe`):

```
load mosaic  →  subtract black level  →  scale to white level
             →  apply white-balance multipliers
             →  demosaic (Bayer / X-Trans → RGB)
             →  camera-space → working-space (via camera→XYZ matrix)
             →  (hand off to Phase 04 pipeline: tone, output transform)
```

What a RAW reader must give you (this is the `rawloader` feature contract — match it):
- raw pixels exactly as encoded (the mosaic)
- camera identification (for matrices/profiles)
- crop margins (top/right/bottom/left) to the active area
- per-channel **black and white points**
- **white-balance multipliers**
- **camera→XYZ** conversion matrix

```rust
// crates/raw/src/lib.rs
pub struct RawImage {
    pub width: u32, pub height: u32,
    pub cfa: CfaPattern,            // Bayer RGGB / X-Trans / ...
    pub mosaic: MosaicData,         // u16 typically
    pub crop: CropMargins,
    pub black: [u16; 4], pub white: [u16; 4],
    pub wb_coeffs: [f32; 4],
    pub cam_to_xyz: [[f32; 3]; 3],
}
```

Reference decode call shape (from `rawloader`, for study):
```rust
let image = rawloader::decode_file("IMG_1234.CR2")?;
if let rawloader::RawImageData::Integer(data) = image.data { /* mosaic */ }
```

---

## Sub-phases

### 03.1 — Container/format parsing
- Parse the RAW container (TIFF/EP-based for most: CR2/CR3, NEF, ARW, DNG, RAF…).
- Extract mosaic + the metadata contract above. If writing your own, start with **DNG** (open spec, well documented) then add proprietary formats.

### 03.2 — Black/white level + white balance
- Subtract black, normalize to white, apply WB multipliers in linear space.
- Validate against a known DNG where you can cross-check values.

### 03.3 — Demosaic
- Implement at least one high-quality algorithm (Malvar-He-Cutler is a good default; AHD/Menon are higher quality). Provide **selectable** algorithms (this is a `zenraw` feature worth matching).
- Handle X-Trans separately if you support Fuji.

### 03.4 — Color space conversion
- Camera-space → XYZ → your working space (linear Rec.709/ProPhoto/etc.) via the camera matrix.
- This is where color accuracy is won or lost — test against reference renders.

### 03.5 — Camera coverage
- Track supported camera/format matrix. Coverage breadth is a primary competitive axis; use `rawler`/`rawloader`'s supported lists as the target.

---

## Evaluate against your editor / RapidRAW
- **Camera coverage:** how many bodies/formats do you support vs `rawler` (RapidRAW's base)?
- **Demosaic quality:** side-by-side your output vs `zenraw` (MalvarHeCutler) and `imagepipe` on the same RAW — check for zippering, false color, moiré.
- **Color accuracy:** render a color target (or known DNG) through yours vs a reference; measure deltaE.
- **Stage order:** confirm yours matches the reference order above — a subtle reorder (e.g. WB after demosaic) shifts color.

## Testing & acceptance criteria
- [ ] A reference **DNG** decodes to expected dimensions, crop, and neutral WB looks neutral.
- [ ] Black/white/WB values cross-checked against an independent reader on ≥3 cameras.
- [ ] Demosaic produces no visible zippering on a resolution chart.
- [ ] deltaE vs reference render under an agreed threshold on a color target.
- [ ] At least N target cameras decode without error (define N from your market).

## Risks / gotchas
- **🟡 LGPL linking boundary (if you embed rawler/rawloader/imagepipe):** LGPL expects users to be able to substitute a modified library. Rust's default *static* linking makes this murky. Mitigations: isolate the crate in a dynamically-loaded module, ship relinkable object files, or don't embed it. Get legal sign-off.
- **rawler panics on malformed input by design** — its maintainers say don't use it on untrusted files. Fine for user-owned desktop files; sandbox otherwise.
- **💰 zenraw** free tier is AGPL — do not ship it closed without the commercial license.
- Proprietary RAW formats are under-documented; expect reverse-engineering effort if writing your own.
