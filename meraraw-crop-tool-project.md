# MeraRAW — Crop & Transform Tool: Research + Full Project Description

**Document type:** Feature research & phased project plan
**Scope:** The complete crop experience — crop rectangle, aspect ratios, straighten/rotate, flip, composition overlays ("frames"/guides), perspective interaction points, keyboard/mouse UX, pipeline placement, persistence, and export behavior.
**References studied:** Adobe Lightroom (Classic + CC), darktable, RawTherapee, RapidRAW.

---

## 1. Why the crop tool matters more than it looks

Cropping is usually the single most-used "geometry" operation in a raw editor. It is also the tool where UX quality is most immediately visible: latency, handle feel, snapping behavior, and overlay clarity all telegraph the overall quality of the app. Every reference app treats crop as:

1. **Non-destructive** — the crop is metadata (a set of instructions), never a pixel deletion. Lightroom, darktable, RawTherapee, and RapidRAW all store crop parameters and only apply them at render/export time. RawTherapee goes furthest conceptually: the crop "does not actually occur until you export the image" — the editor merely masks the cropped-off area in the preview.
2. **A composition tool, not just a trimming tool** — overlays (rule of thirds, golden ratio, etc.) turn crop into a teaching/composition aid.
3. **Entangled with rotation and perspective** — straightening and keystone correction change what pixels exist, so crop must react (auto-fit, constrain-to-image, black-corner avoidance).

---

## 2. Competitive research

### 2.1 Adobe Lightroom (Classic + CC) — the UX benchmark

**Invocation & lifecycle**
- `R` opens the crop tool from any module; `Enter`/double-click commits; `Esc` cancels (and if the image was previously cropped, Esc resets to the state at the start of the session).
- Crop is re-editable forever; the panel shows Reset at all times.

**Crop rectangle interaction**
- Initial drag creates the crop; afterwards, corner/edge handles resize and dragging inside the marquee repositions the **crop over the image** (Classic) or the **image under the crop** (CC-style). Both mental models exist; Lightroom Classic moves the image beneath a mostly-fixed crop, which most users find more stable.
- `X` swaps orientation (landscape ↔ portrait) of the current aspect ratio.
- Padlock icon locks/unlocks the aspect ratio; holding `Shift` temporarily constrains the ratio even when unlocked; `Alt/Option`-drag resizes symmetrically from the center.
- Spacebar temporarily pans/zooms while inside the crop tool (CC).

**Aspect ratios**
- Presets (Original, 1:1, 4:5, 5:7, 4:3, 3:2, 16:9, 16:10 …) plus **Enter Custom**, with the **last 5 custom ratios remembered**.
- Right-click context menu inside the marquee: Reset Crop, Crop as Shot, Constrain Aspect Ratio, Crop to Same Aspect Ratio.
- Crop can be **synced across selected images** (batch), and a crop ratio can be applied from Quick Develop in grid view.

**Straighten / rotate**
- Four ways to level an image, all coexisting:
  1. **Angle slider** (±45°),
  2. **Ruler/Straighten tool** — draw a line along a horizon/vertical, image rotates to match,
  3. **Auto** button — content-aware auto-level,
  4. **Freehand rotate** — cursor outside the marquee turns into a rotate cursor; while rotating a fine grid appears to help alignment.
- `Ctrl/Cmd`-drag temporarily invokes the straighten ruler without switching tools.
- **Constrain Crop / Constrain to Image** checkbox prevents the crop from including blank areas created by rotation or lens/transform corrections.

**Overlays ("frames"/guides)** — the richest of the four apps:
- Tool Overlay mode: **Auto / Always / Never**.
- `O` cycles overlays; `Shift+O` rotates/flips asymmetric ones. Set of overlays: **Grid, Rule of Thirds, Diagonal, Triangle (Golden Triangle), Golden Ratio, Golden Spiral, Aspect Ratios**.
- The **Aspect Ratios overlay** is special: it draws several selected target ratios simultaneously inside the crop so you can preview multiple print/social sizes at once (user-configurable via Tools ▸ Crop Guide Overlay ▸ Choose Aspect Ratios).
- Custom overlay images are supported in Classic (advanced).

**Key takeaway from Lightroom:** the crop tool is a *mode* with heavy keyboard support, multiple redundant paths to the same action (slider, ruler, drag, auto), and overlays treated as first-class configurable objects.

### 2.2 darktable — the architecture benchmark

darktable made a deliberate architectural decision worth copying: it **split the old "crop and rotate" module into three separate pipeline modules**:

- **`crop`** — pure creative cropping, placed **late in the pixelpipe (after retouch)** so the retouch/heal tools can still sample from areas outside the crop.
- **`rotate and perspective`** — rotation + keystone/converging-line correction, placed earlier. Includes: rotation with a soft ±10° limit (right-click to enter up to 180°), right-click-drag to draw a leveling line, **automatic structure detection** (analyzes the image for line segments, ShiftN-inspired), a **perspective rectangle** drawing mode, and **automatic-crop options** ("largest area" or "original format") to remove black corners after warping.
- **`orientation`** — flips/90° rotations.

Other darktable crop specifics:
- While the module has focus, the **full uncropped image is shown** with the crop overlaid — you always see what you're giving up.
- Crop dimensions (px) and current ratio are displayed **in the center of the crop while resizing**.
- `Ctrl`-drag constrains movement horizontally, `Shift`-drag vertically; `Shift` while resizing keeps the current freehand ratio.
- Margin sliders (left/right/top/bottom as % of image) mirror the on-canvas crop bidirectionally — useful for precision and for accessibility.
- Large preset ratio list, plus **user-defined ratios via config** (`x:y` or decimal entry directly in the combobox).
- **Guides are a global, shared subsystem** ("guides & overlays"): every geometry-affecting module (crop, rotate/perspective, framing, liquify, lens correction…) gets the same "show guides" checkbox + settings. Guide types include grid (with configurable subdivisions), rule of thirds, golden mean, harmonious/golden triangles, perspective lines, and more; asymmetric guides can be flipped H/V; **line color is configurable**.

**Key takeaways from darktable:** (a) separate *crop* from *rotate/perspective* in the processing pipeline even if the UI presents them together; (b) build guides as a shared, reusable overlay engine, not something hardcoded into the crop tool; (c) always render the full image while cropping.

### 2.3 RawTherapee — the "honest preview" benchmark

- Crop never discards data; the cropped-off region is covered by a **semi-transparent dark mask whose color and opacity are user-configurable** (Preferences ▸ Crop mask color/transparency). Zooming out still reveals the hidden area.
- Crop-placing mode: click "Select Crop", drag to create; **Shift-drag pans the crop**; clicking without dragging clears the crop; a dedicated "fit cropped area to screen" shortcut exists.
- **Guide Type dropdown** per-crop (rule of thirds, diagonals, harmonic means, grid, ePassport…), with orientation auto-detected from the image ("As Image") since v4.2.
- The most distinctive features:
  - **PPI readout** — set a PPI value and RawTherapee shows the *physical print size* of the current crop at that resolution. Purely informational, brilliant for print workflows.
  - **Domain-specific ratios**: beyond 3:2/4:3/16:9/16:10/1:1/2:1 it ships **24:65 XPan**, **DIN/ISO 216 (1.414, A-series paper)**, **8.5:11 US Letter**, **11:17 Tabloid**, and **45:35 ePassport** — the ePassport ratio comes with special horizontal guides for crown/nostrils/chin placement for biometric photo compliance.
  - **Custom ratio by example**: unlock ratio, type width/height numbers to define the ratio, then Shift-drag preserves it.
- Numeric X/Y/W/H entry fields for the crop exist alongside the mouse interaction.

**Key takeaways from RawTherapee:** (a) numeric entry and mouse manipulation must stay in perfect sync; (b) niche-but-loved features (PPI print preview, passport guides, paper ratios) are cheap to build and create devoted users; (c) crop mask appearance should be a preference.

### 2.4 RapidRAW — the closest architectural cousin

RapidRAW (Tauri + Rust + WGSL GPU pipeline, non-destructive `.rrdata` sidecars) is the most relevant comparison for a modern GPU-first editor. Its crop journey — visible in its changelog — is effectively a list of pitfalls MeraRAW should design around from day one:

- Crop ships with **aspect-ratio locking, rotate, flip**, a **rotation slider + straighten line tool**, and predefined 90° rotation.
- Bugs/lessons it had to fix over time (learn from these):
  - **Default aspect ratio should be "Original"** and user-definable (early versions defaulted wrong; users complained).
  - **Preserving crop position when changing aspect ratio** (fixed much later, 2026) — naive implementations recenter the crop and destroy the user's framing.
  - **Zoom behavior inside crop mode** — "can't zoom out enough to crop portrait images", "disable auto-zoom when entering crop mode", top-left zoom bugs.
  - **Auto-crop after rotation** to prevent black borders (added ~1 month post-launch).
  - **Masks and crops must share a coordinate system** — RapidRAW had to fix "mask scaling/alignment issues related to cropping and rotation" and later "lens distortion fixes for AI masks & crop". Any local adjustment/mask feature must be defined in *original image space*, not crop space.
  - **Filmstrip/thumbnail thrash while cropping** — regenerating thumbnails on every crop drag kills performance; users asked for thumbnails to update only when the crop is "locked in".
  - **Crop belongs in presets and copy/paste settings** — users demanded copy/paste of crop+ratio+rotation with adjustments; RapidRAW eventually let masks and crops be saved into presets.
  - Added **automatic geometry-transformation helper lines** and dynamic image-bounds checks for the crop.
  - Show the **new cropped pixel dimensions** in the UI after cropping.

**Key takeaways from RapidRAW:** crop touches everything — masks, thumbnails, presets, copy/paste, export, lens correction. Design the crop's data model and coordinate spaces *first*, or spend a year of changelog entries fixing it.

### 2.5 Feature comparison matrix

| Feature | Lightroom | darktable | RawTherapee | RapidRAW | MeraRAW target |
|---|---|---|---|---|---|
| Non-destructive crop | ✔ | ✔ | ✔ | ✔ (sidecar) | ✔ (P1) |
| Ratio presets + custom | ✔ (last 5 remembered) | ✔ (+config file) | ✔ (by-example) | ✔ basic | ✔ + remembered customs (P2) |
| Orientation flip (X key) | ✔ | ✔ | ✔ (As Image) | partial | ✔ (P2) |
| Freehand rotate outside marquee | ✔ | — | — | — | ✔ (P3) |
| Angle slider | ✔ | ✔ | ✔ | ✔ | ✔ (P3) |
| Straighten line tool | ✔ | ✔ (right-click drag) | ✔ | ✔ | ✔ (P3) |
| Auto-level | ✔ | via structure detect | — | — | ✔ (P6) |
| Constrain to image / auto-crop black corners | ✔ | ✔ (largest area / original format) | ✔ | ✔ (added later) | ✔ (P3) |
| Overlay cycling (O key) | ✔ | — (settings dialog) | — (dropdown) | — | ✔ (P4) |
| Rule of thirds / golden / triangle / spiral / diagonal | ✔ | ✔ | ✔ | helper lines only | ✔ (P4) |
| Multi-aspect-ratio preview overlay | ✔ | — | — | — | ✔ (P4, differentiator) |
| Configurable grid subdivisions + guide color | — | ✔ | — | — | ✔ (P4) |
| Guide engine shared across tools | — | ✔ | — | — | ✔ (P4 architecture) |
| Center info readout (px + ratio) | — | ✔ | ✔ (fields) | requested | ✔ (P2) |
| Margin/numeric entry | — | ✔ (% sliders) | ✔ (px fields) | — | ✔ (P5) |
| PPI print-size readout | — | — | ✔ | — | ✔ (P5) |
| Passport/paper/XPan ratios | — | — | ✔ | — | ✔ (P5) |
| Keystone/perspective | Transform panel | separate module (auto + rect + lines) | ✔ | helper lines | P7 (separate tool, out of core scope) |
| Batch/sync crop | ✔ | via history stack | via profiles | copy/paste + presets | ✔ (P6) |
| Crop in presets | partial | ✔ | ✔ | ✔ (v1.5) | ✔ (P6) |
| Configurable crop mask color/opacity | — | — | ✔ | — | ✔ (P5) |

---

## 3. Design principles for the MeraRAW crop tool

1. **Crop is a parameter set, never a pixel operation.** Store `{rect (normalized 0–1 in *post-rotation* image space), angle, flipH, flipV, ratio lock state, ratio value}` in the edit sidecar. Export applies it at full resolution.
2. **One canonical coordinate space.** All geometry (crop, masks, healing, overlays) is defined in **original sensor space**; rotation/perspective produce a transform matrix; the crop rect lives in the rotated space but is stored with enough info to re-derive it. This single decision prevents the entire RapidRAW mask-misalignment bug class.
3. **Pipeline order (darktable model):** orientation (90°/flip) → lens correction → rotation/perspective → *everything else* → **crop last (or crop-as-view)**, so retouch/masks can reference out-of-crop pixels.
4. **Show the whole image while cropping** (darktable/RawTherapee): dim the cropped-away region with a configurable mask instead of hiding it.
5. **Every mouse action has a keyboard/numeric twin** (accessibility + precision).
6. **Never lose the user's framing.** Changing ratio preserves crop center and maximizes area within the new ratio; toggling orientation pivots around center; undo/redo covers every crop mutation.
7. **60 fps or bust.** Crop dragging must not trigger full re-renders, thumbnail regeneration, or histogram recomputation until pointer-up (RapidRAW lesson).

---

## 4. Interaction specification (summary)

### 4.1 States
`Idle → CropMode(Creating | Adjusting | Rotating | Straightening) → Committed`
- Enter: toolbar button or `C` (or `R` for Lightroom muscle-memory — make it a settings option).
- Commit: `Enter`, double-click, or switching tools. Cancel: `Esc` (restores pre-session crop).

### 4.2 Mouse map
| Gesture | Action |
|---|---|
| Drag on empty canvas (first time) | Create crop |
| Drag corner/edge handle | Resize (respect ratio lock; `Shift` = temp-lock; `Alt` = from center) |
| Drag inside rect | Move crop (or move image-under-crop — user setting; default: move crop, `Space`+drag pans view) |
| Drag outside rect | Freehand rotate; fine grid appears while rotating |
| `Ctrl/Cmd`+drag on image | Straighten ruler (draw a line → auto-level) |
| Right-click in rect | Context menu: Reset, Crop as Shot, Lock ratio, Copy crop, Paste crop |
| Scroll | Zoom preview (never auto-zoom on mode entry) |

### 4.3 Keyboard map
`C` toggle tool · `Enter` commit · `Esc` cancel · `X` swap orientation · `L` lock ratio · `O` cycle overlay · `Shift+O` flip/rotate overlay · arrows nudge crop 1px (Shift = 10px) · `Ctrl+arrows` nudge angle 0.1° · `0` reset angle · `Ctrl+Alt+C` copy crop · `Ctrl+Alt+V` paste crop.

### 4.4 On-canvas HUD
- Center readout while resizing: `4021 × 2681 px · 3:2 · 12.9 MP` (darktable-style), fading out on idle.
- Angle readout while rotating: `−1.7°`.
- Optional PPI line: `@300 PPI → 13.4 × 8.9 in / 34.0 × 22.7 cm`.

### 4.5 Ratio system
- Preset list: `Original · Free · 1:1 · 5:4 · 4:3 · 3:2 · 16:10 · 16:9 · 2:1 · 65:24 (XPan) · A-series (1.414) · US Letter · 4:5 (social) · 9:16 (story)`.
- Custom entry as `x:y` or decimal; **last 5 customs remembered** (Lightroom).
- Default on open: **Original** (RapidRAW user-demanded default), user-overridable.
- Orientation auto-matches image ("As Image", RawTherapee behavior); `X` flips it.

### 4.6 Overlay/guide engine ("frames")
Built as a **standalone overlay subsystem** consumed by the crop tool (and later by perspective, framing, local-adjustment tools — darktable model):

Guide types (v1): None · Grid (configurable N×M subdivisions) · Rule of Thirds · Diagonals · Golden Ratio (phi lines) · Golden Spiral (flippable, Shift+O) · Golden/Harmonious Triangles (flippable) · Center Cross · **Aspect-Ratio Preview** (draw 1–4 user-chosen target ratios inside the crop simultaneously — Lightroom's killer overlay) · ePassport guides (crown/nostril/chin lines, paired with 45:35 ratio).

Guide options: color, opacity, line weight; display mode **Auto (only while dragging) / Always / Never**; per-guide flip for asymmetric ones.

### 4.7 Straighten & rotate
- Angle slider ±45° (soft), numeric entry up to ±180°.
- Straighten line tool (any near-horizontal line → horizontal; near-vertical → vertical, auto-detected by dominant axis).
- Freehand rotate with live fine-grid overlay.
- **Constrain-to-image ON by default**: as the image rotates, the crop auto-shrinks to the largest rect of the current ratio containing no blank pixels (closed-form solution exists for axis-aligned rect in rotated rect — see §5.2).
- Flip H / Flip V buttons; 90° CW/CCW buttons (these route to the *orientation* stage, not the crop).
- Auto-level button (Phase 6): horizon detection via Hough transform / gradient-orientation histogram on a downscaled luma.

---

## 5. Technical architecture

### 5.1 Data model (sidecar)
```json
{
  "orientation": { "rotate90": 0, "flipH": false, "flipV": false },
  "geometry":    { "angleDeg": -1.7, "keystone": null },
  "crop": {
    "rect": { "x": 0.0421, "y": 0.0000, "w": 0.9034, "h": 0.9034 },
    "ratio": { "mode": "preset", "value": "3:2", "locked": true, "orientation": "landscape" },
    "constrainToImage": true
  },
  "ui": { "lastGuide": "thirds", "customRatios": ["1.91:1", "13:11"] }
}
```
- `rect` is normalized to the **rotated, orientation-applied image bounds** so it survives resolution changes (previews vs. full-res export).
- Versioned schema + migration hooks from day one.

### 5.2 Geometry math to implement
- Rotation about image center; derive the **maximal inscribed axis-aligned rectangle** of a given aspect ratio inside a rotated rectangle (closed-form: for rotation θ and image W×H, the largest inscribed rect of ratio r has known analytic solution — implement + property-test it).
- Ratio-preserving handle resize with anchor at opposite corner/edge; `Alt` re-anchors at center.
- Ratio change that preserves center and maximizes area, clamped to image bounds.
- Hit-testing with generous handle targets (≥ 24 px visual, ≥ 12 px logical at any zoom).
- All of this in one pure, unit-tested `crop_geometry` module — no UI code inside.

### 5.3 Rendering
- Crop overlay drawn as a GPU/vector layer over the preview: mask (dimmed region), rect border, handles, guides. No image re-render needed while dragging — only on angle change (which re-rasterizes via the transform shader) — and even then, render at preview resolution with debounced full-quality pass on pointer-up.
- Histogram, clipping indicators, and filmstrip thumbnail update **only on commit / pointer-up** (RapidRAW lesson).

### 5.4 Integration contracts
- **Masks/local adjustments:** stored in original space; UI converts through the current transform. Add regression tests: "crop → mask → change crop → mask stays glued to image content."
- **Export:** applies orientation → geometry → crop at full res, Lanczos-3 (or equivalent high-quality) interpolation for the rotation resample (darktable recommends lanczos3 for angle/keystone).
- **Presets & copy/paste:** crop participates in preset save/apply and copy/paste-settings with a checkbox (off by default in "copy all", explicit in batch).
- **History/undo:** every committed crop mutation is one undo step; in-drag intermediate states are coalesced.

---

## 6. Project phases

### Phase 0 — Foundations & geometry core (1–2 weeks)
**Goal:** the math and data model exist and are bulletproof before any UI.
- `crop_geometry` module: normalized rect, ratio math, rotation, maximal-inscribed-rect, handle resize solver.
- Sidecar schema v1 + serialization + migration scaffold.
- Property-based tests (rect always within bounds; ratio preserved to ε; round-trip normalize/denormalize lossless across resolutions).
- **Exit criteria:** 100% of geometry functions unit-tested; fuzz tests pass 10⁶ iterations.

### Phase 1 — MVP crop (2–3 weeks)
**Goal:** non-destructive freehand + ratio-locked crop, committed to sidecar, honored by preview and export.
- Enter/exit crop mode; create/resize/move rect; dimmed mask over cropped-off area (full image always visible).
- Ratio presets (Original, Free, 1:1, 3:2, 4:3, 16:9, 4:5) + lock toggle + `X` orientation swap.
- Default ratio = Original. No auto-zoom on entry; portrait images fully visible (explicit test case).
- Commit/cancel/reset; single undo step per commit; export applies crop.
- Center HUD readout (px + ratio).
- **Exit criteria:** crop survives app restart via sidecar; export pixel-exact vs. preview; 60 fps drag on a 60 MP file.

### Phase 2 — Ratio system & polish (1–2 weeks)
- Custom ratio entry (`x:y` / decimal); remember last 5 customs.
- Ratio change preserves crop center & maximizes area (regression test: "change ratio ≠ recenter").
- `Shift` temp-lock, `Alt` center-resize, arrow-key nudging, right-click context menu (Reset, Crop as Shot, Lock).
- Crop as Shot (restore camera/EXIF default crop where applicable).
- **Exit criteria:** full keyboard/mouse map from §4.2–4.3 (minus rotation) implemented.

### Phase 3 — Straighten, rotate & flip (2–3 weeks)
- Angle slider + numeric entry; freehand rotate outside marquee with fine grid; straighten ruler (`Ctrl`-drag) with H/V auto-detect.
- Flip H/V and 90° rotations (orientation stage).
- **Constrain-to-image**: auto-fit crop after rotation (largest-area and keep-ratio modes, darktable-style); never show blank corners when enabled.
- High-quality resampling on commit (preview-res during drag, Lanczos on settle/export).
- **Exit criteria:** rotating a 3:2 crop through ±10° keeps a valid, blank-free, correctly-ratioed crop at all times; masks (if present) remain aligned.

### Phase 4 — Overlay/guide engine ("frames") (2 weeks)
- Standalone guide-rendering subsystem with plugin-style guide definitions.
- Guides: grid (configurable), thirds, diagonals, golden ratio, golden spiral, golden triangles, center cross.
- `O` cycles, `Shift+O` flips asymmetric guides; display mode Auto/Always/Never; color/opacity/weight settings (global, darktable-style).
- **Aspect-Ratio Preview overlay** (multi-ratio ghost frames inside the crop) — flagship differentiator.
- **Exit criteria:** guide engine consumed by crop via public API (proving reusability for future tools); all guides render correctly on rotated crops.

### Phase 5 — Precision, print & niche ratios (1–2 weeks)
- Numeric X/Y/W/H fields + margin sliders (%), bidirectionally synced with canvas.
- PPI readout → physical print size line in HUD (RawTherapee).
- Extended ratio pack: XPan 65:24, DIN/A-series 1.414, US Letter, Tabloid, 45:35 ePassport **with crown/nostril/chin guide lines**.
- Configurable crop-mask color/opacity preference.
- **Exit criteria:** a passport photo and an A4 print crop can be produced entirely by spec.

### Phase 6 — Workflow integration (2–3 weeks)
- Copy/paste crop (with/without angle) between images; include-crop checkbox in copy-settings dialog.
- Crop in presets (save/apply); batch apply to selection with per-image recentering pass.
- Auto-level button (horizon detection on downscaled luma; ship behind a flag, measure accuracy).
- Filmstrip/thumbnail update deferred to commit; performance audit.
- **Exit criteria:** sports-photographer workflow (RapidRAW discussion #501): crop+ratio+rotation copy-paste across 200 images without UI stalls.

### Phase 7 — Perspective/keystone (separate follow-on project)
Explicitly **out of scope for the crop tool** but designed-for: rotation/perspective becomes its own tool earlier in the pipeline (darktable model) with straighten shared, manual H/V line drawing, perspective rectangle, and (later) automatic structure detection. The crop tool's constrain-to-image logic already generalizes to warped bounds.

### Cross-phase workstreams
- **Testing:** geometry property tests (P0), pixel-diff export tests (P1+), interaction e2e tests (P2+), mask-alignment regression suite (P3+), performance budget CI gate: crop-drag frame time p95 < 8 ms on reference hardware.
- **Docs:** shortcut cheat-sheet, user guide page per phase.
- **Telemetry (opt-in):** which ratios/guides get used → informs default ordering.

---

## 7. Acceptance checklist (definition of "world-class")

- [ ] Crop never destroys data; fully re-editable; survives restart & export round-trip.
- [ ] Full image visible while cropping, with configurable dim mask.
- [ ] Original-ratio default; ratio changes never recenter the crop.
- [ ] Four ways to straighten (slider, ruler, freehand, auto) — at least three shipped.
- [ ] No blank corners ever visible with constrain-to-image on.
- [ ] `O`-cycled guide set ≥ 7 types incl. multi-aspect preview; guide engine reusable.
- [ ] Numeric parity for every mouse interaction; PPI print readout.
- [ ] Masks, presets, copy/paste, batch, thumbnails all crop-aware with zero misalignment.
- [ ] 60 fps drag on 60 MP raw; no thumbnail thrash mid-drag.

---

## 8. Source notes

- Lightroom crop behavior: Adobe Help (Crop/Rotate/Geometry), Julieanne Kost's LR Classic crop tips, photographylife overlay guide.
- darktable: official user manual — `crop`, `rotate and perspective`, deprecated `crop and rotate`, and `guides & overlays` module references.
- RawTherapee: RawPedia "Crop" page (ratios, ePassport, PPI, mask preferences, custom ratios since 5.1).
- RapidRAW: CyberTimon/RapidRAW README, changelog, releases (v1.5 presets w/ crop), issue #26 and discussion #501 (user-reported crop UX demands).
