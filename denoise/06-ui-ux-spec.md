# 06 — UI / UX Specification

Denoise panel design for MeraRAW's React frontend. Philosophy: **Lightroom's simplicity by
default, darktable/RawTherapee's power behind a disclosure.**

---

## 1. Panel anatomy (Detail section of the edit sidebar)

```
▾ NOISE REDUCTION                                   [⏻]
 ┌───────────────────────────────────────────────────┐
 │  ◉ AI Denoise                                     │
 │     Amount            ────────●────────   50      │
 │     [ Model: MeraNoise v1 ▾ ]     status chip     │
 │     ( Run Denoise )   ⏱ est. 12 s   [cancel]      │
 │                                                   │
 │  ◉ Manual                                         │
 │     Luminance         ──●──────────────   18      │
 │     Detail            ─────────●───────   55      │
 │     Color             ────────●────────  auto     │
 │                                    [Auto ✓]       │
 │  ▸ Advanced                                       │
 └───────────────────────────────────────────────────┘
```

### Modes
- **Off / Manual / AI / AI + Manual.** AI and Manual can stack (AI as base, manual
  refinement on top — LR's post-Denoise manual sliders pattern). UI: two toggle groups,
  each independently enableable; the panel header power toggle kills both.

### AI section
- **Amount** (0–100, default 50). Real-time after first inference (blend design,
  doc 05 §5) — communicate this: before first run the slider shows a subtle
  "runs after Denoise" hint; after, it's live like any other slider.
- **Run Denoise** button with ETA estimate (from a stored per-device tile benchmark);
  during run: determinate progress bar (tiles), cancel, and the rest of the app stays
  interactive.
- **Model chip states:** `not downloaded (size)` → click = download w/ progress →
  `ready` → `processing`. Model picker hidden unless >1 model installed.
- Unsupported source (e.g. mosaic-only model + JPEG): section disabled with one-line
  reason and automatic fallback suggestion.

### Manual section (simple)
- **Luminance** (0–100, default 0) — luma engine strength.
- **Detail** (0–100, default 50) — detail recovery (residual DCT + center weight).
  Disabled/dim when Luminance = 0.
- **Color** (0–100) with **Auto** checkbox (default on) — auto value comes from the
  noise-profile estimate (RT auto-chroma behavior; LR's "color 25 by default"
  philosophy). Moving the slider unchecks Auto.

### Advanced (disclosure, hidden by default)
- **Engine:** `Wavelet (auto)` | `Wavelet` | `Non-local means` — with one-line tooltips
  ("NLM: smoother on heavy noise, slower").
- **Strength** (0.25–4.0×, default 1.0): global scale on the noise profile — the
  darktable master dial.
- **Wavelet curves:** two mini curve editors (Luma / Chroma), x = coarse→fine, y =
  strength; the simple sliders drive these curves when untouched; touching a curve
  switches that channel to custom (shown by a dot). Reset per curve.
- **NLM params** (visible when engine = NLM): patch size, search radius, original-detail
  (center weight).
- **Impulse noise** (0–100): conditional median for salt & pepper.
- **Hot pixel removal** toggle (default on) + "detected: N" count readout.
- **Mode:** Conservative | Aggressive (texture-mask threshold).
- **Fit noise profile from image** button → runs estimation, shows (a, b)/ISO readout,
  updates auto values.

## 2. Preview behaviors

- **100% zoom nudge:** when the user first touches any denoise control below 50% zoom,
  show a dismissible toast "Zoom to 100% to judge noise" with a click-to-zoom action
  (all four reference apps' docs insist on this).
- **Before/after:** panel-scoped compare (hold-to-see-original on the preview, plus the
  app's global compare). AI preview: after inference, clicking the image toggles
  enhanced/original (LR's Enhance dialog behavior).
- **Split preview** (optional v1.1): draggable divider, denoise-only diff view
  (show what's being removed — the residual — helpful for tuning; darktable users
  simulate this with blend modes).
- **Processing at preview scale:** silently uses scaled σ (doc 03 §5); at <100% zoom a
  tiny "preview approximation" dot appears on the panel, gone at 1:1.

## 3. Cross-feature interactions

- **Sharpening guard:** if NR luminance > 40 while sharpening detail/texture boost fine
  scales, show a passive hint in the Sharpening panel ("high noise reduction active —
  fine sharpening may re-amplify noise"). Never auto-change user values; RT docs make
  this a workflow rule, we make it a hint.
- **Demosaic hint:** if ISO ≥ 6400 and demosaicer is detail-maximizing, suggest the
  noise-tolerant one (only if MeraRAW exposes demosaic choice).
- **Masks/local adjustments:** manual denoise (Luminance/Color only) available inside
  local adjustment layers — e.g. extra NR on shadow mask (LR masking workflow,
  RT Local Adjustments). AI is global-only in v1.
- **Batch:** copy/paste settings and preset support like every other panel; "Run AI
  Denoise on N selected" queues jobs with an aggregate progress toast.
- **Auto ISO presets:** optional setting "apply default noise reduction on import based
  on ISO" — table: <800 off; 800–3200 chroma auto only; >3200 chroma auto + luma 15;
  ≥12800 suggest AI (badge on the panel).

## 4. Defaults summary

| Control | Default | Rationale |
|---|---|---|
| Hot pixels | on | free, always correct |
| Color (chroma) | auto (profile-driven) | LR=25 default, RT auto: chroma removal is assumed |
| Luminance | 0 | grain is a taste choice; never smooth by default |
| Detail | 50 | LR parity |
| AI Amount | 50 | LR parity |
| Engine | Wavelet (auto) | darktable's current default; fast |
| Mode | Conservative | safe |

## 5. Copy & accessibility

- Slider tooltips explain *effect*, not algorithm ("Reduces color speckles" not "chroma
  wavelet shrinkage").
- All progress states announced via aria-live; cancel reachable by keyboard.
- Panel state fully keyboard-operable; curve editors have +/- keyboard fallback fields.
