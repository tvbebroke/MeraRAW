# 06 — Edit Photos (Develop Engine)

**Applies to:** Desktop v2.0 / Mobile v4.0 / Web — **[BUILD: October 2018]**, with
per-control build notes inline. **This is the core of the application.**

## Surface & gating
- Edit controls live in **Detail view** only (not in Photo Grid / Square Grid).
- Edit panel groups, in order: **Profile → Light → Color → Effects → Detail → Optics →
  Geometry**. Each group expand/collapses.
- Edit pipeline is non-destructive; every control is a parameter on an edit record applied
  over the decoded source.

---

## 6.1 Profile
Profiles set the **base color/tone rendering** before any slider. Applying a profile does
**not** alter other slider values — it's a separate layer the sliders sit on top of.

**Profile groups (raw photo):**
- **Adobe Raw** — Adobe Color (default for color), Adobe Monochrome (default for B&W),
  plus Vivid/Neutral/Landscape/Portrait/etc.
- **Camera Matching** — per camera make/model; mimics in-camera JPEG rendering.
- **Legacy** — profiles from earlier app versions.

**Creative profiles (any file type incl. JPEG/TIFF):** Artistic, B&W, Modern, Vintage.
- Creative profiles expose an **Amount** slider (profile intensity). Adobe Raw / Camera
  Matching do **not**.

**Profile management:**
- Browse modes: List / Grid / Large thumbnail; filter by type (Color / B&W).
- **Favorites** group (star toggle per profile).
- **Manage Profiles** → show/hide groups; setting is **per-device**, not synced.
- **Import profiles:** XMP format via three-dot menu → Import Profiles. `[BUILD: v1.4+]`
- **DCP→XMP auto-conversion** on first launch after update. Manual DCP drop locations:
  - Win: `C:\ProgramData\Adobe\CameraRaw\CameraProfiles`
  - Mac: `~/Library/Application Support/Adobe/CameraRaw/CameraProfiles`
- `[BUILD: v1.4 June 2018]` Presets **and** profiles (incl. custom/third-party) sync across
  CC desktop+mobile — **but do not sync to Lightroom Classic.**

> Ties directly to the earlier camera-profile work: the editor consumes XMP-format
> profiles at runtime; DCP is converted on ingest.

---

## 6.2 Light (tonal range)
Sliders: **Exposure, Contrast, Highlights, Shadows, Whites, Blacks**.
- **Exposure** — global brightness.
- **Contrast** — midtone contrast spread.
- **Highlights** — brightness of lighter regions (−recovers detail / +brightens).
- **Shadows** — brightness of darker regions (−deepens / +recovers).
- **Whites** — white clipping point.
- **Blacks** — black clipping point.
- **AUTO** button → ML auto-sets Exposure, Contrast, Highlights, Shadows, Whites, Blacks,
  **Saturation, Vibrance**.

### Tone Curve
- **Parametric/point curve.** X = input tone (black→white left→right); Y = output tone.
  45° line = identity.
- **Channels:** RGB (luminance) + **Red / Green / Blue** point curves individually.
- Add control point = click; delete = right/ctrl-click → Delete Control Point; drag to
  edit; Reset Channel via context menu.
- `[BUILD: v1.4 June 2018]` **Removed**: Medium/Strong Contrast tone-curve presets and the
  ability to save/apply curves inside the panel. Curves are now persisted **as presets**
  (so they sync ecosystem-wide).

---

## 6.3 Color
- **White Balance:** preset menu **or** WB **eyedropper** (click a neutral area).
- **Temp** — cool↔warm (yellow↔blue).
- **Tint** — green↔purple.
- **Vibrance** — nonlinear saturation; boosts low-sat colors more; protects skin tones.
- **Saturation** — linear saturation of all colors.
- **B&W** button — convert to monochrome.
- **HSL / Color Mix** — per-color **Hue, Saturation, Luminance** sliders.

---

## 6.4 Effects
- **Clarity** — local midtone/edge contrast (−softens / +adds punch).
- **Dehaze** — −adds haze / +removes haze.
- **Vignette** — post-crop edge darkening/lightening, with:
  - **Feather** (transition softness), **Midpoint** (extent from corners),
    **Roundness** (oval↔circular), **Highlights** (highlight preservation when Amount<0).
- **Split Toning** — separate **Hue + Saturation** for Shadows and Highlights, plus a
  **Balance** slider biasing the split point.

---

## 6.5 Detail
- **Sharpening** (amount) with sub-sliders:
  - **Radius** — size of detail sharpened.
  - **Detail** — high-frequency emphasis / edge vs texture.
  - **Masking** — edge mask; 0 = uniform, 100 = edges only.
- **Noise Reduction** (luminance): **Detail** (threshold), **Contrast**.
- **Color Noise Reduction:** **Detail** (threshold), **Smoothness**.
- **Grain:** amount + **Size** (≥25 adds blue to blend with NR) + **Roughness** (uniform↔uneven).

---

## 6.6 Optics
- **Remove Chromatic Aberration** — checkbox; auto-corrects blue/yellow + red/green fringing.
- **Enable Lens Corrections** — checkbox; auto-selects a lens profile from EXIF
  (camera model, focal length, f-stop, focus distance).
  - **Built-in profiles** auto-apply for MFT (Panasonic/Olympus), Fuji X, Leica Q, many
    Canon point-and-shoots — shown as "Built-in Lens Profile Applied".
  - Manual override: choose **Make / Model / Profile**. Available profiles differ for
    raw vs non-raw.
  - Customize via **Distortion Correction** (default 100 = 100% of profile) and
    **Lens Vignetting** (default 100). >100 over-corrects, <100 under-corrects.

---

## 6.7 Geometry (Upright)
- **Upright modes:** **Auto** (balanced V+H), **Level** (horizontal only),
  **Vertical**, **Full** (all), **Guided** (manual).
- **Guided:** draw up to **4 guides**; transform applies after ≥2 guides.
- **Manual Transform** sliders refine after Upright.
- **Recommendation/dependency:** enable **Lens Corrections (Optics) before** Upright.

---

## IMPL
```
EditRecord {
  profile: { group, name, amount? }          // amount only for Creative
  light:   { exposure, contrast, highlights, shadows, whites, blacks }
  toneCurve: { rgb:[pts], r:[pts], g:[pts], b:[pts] }   // points persisted; presets only
  color:   { wb: preset|custom, temp, tint, vibrance, saturation, bw:bool, hsl:{per-color hsl} }
  effects: { clarity, dehaze, vignette:{amount,feather,midpoint,roundness,highlights},
             splitTone:{shadow:{h,s}, highlight:{h,s}, balance} }
  detail:  { sharpen:{amount,radius,detail,masking},
             lumaNR:{amount,detail,contrast}, colorNR:{amount,detail,smoothness},
             grain:{amount,size,roughness} }
  optics:  { removeCA:bool, lensCorrections:{enabled, profileRef, distortion=100, vignetting=100} }
  geometry:{ upright: auto|level|vertical|full|guided, guides:[<=4], manualTransform:{...} }
}
```
- **Pipeline order** (apply): decode → profile (base render) → WB/Temp/Tint → Light →
  Tone Curve → HSL/Color → Detail (sharpen/NR) → Optics (CA/lens) → Geometry → Effects
  (clarity/dehaze/vignette/split-tone). (Vignette is explicitly **post-crop**.)
- **AUTO** is an ML endpoint returning 8 slider values; treat as a service call, cache result.
- Gate edit UI to Detail view; keep grid views read-only for edits.
- Profiles are a **base layer**, independent of sliders — never fold profile into slider state.
- Persist tone curves and looks **as presets** (sync unit), not inline panel state `[BUILD: v1.4+]`.
