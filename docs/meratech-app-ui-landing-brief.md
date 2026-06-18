# Meratech — App UI Summary for Landing Page Design

> **Purpose:** This document describes the **actual, shipped UI** of the Meratech RAW desktop editor (Tauri app) as of the current codebase. Give it to your website editor so the marketing landing page **looks like the product**, not a generic photo-editor mockup.
>
> **Source of truth:** `src/styles.css`, `src/App.tsx`, and the LR-style components under `src/components/lr/`.
>
> **Naming note:** The in-app brand reads **Meratech** (top bar, window title). The GitHub repo is **MeraRAW** (`kaimai-apex/MeraRAW`). Pick one public name for the site and use it consistently; the visual system below applies either way.

---

## 0. One-paragraph brief

Meratech is a **dark, neutral, Lightroom-inspired** desktop RAW editor. The interface is dense and professional: charcoal grays that stay **desaturated** so they never bias color judgement, a single **cool blue accent** for selection and active states, and small 12px system type. The photo dominates the center viewport; panels are collapsible with uppercase section headers. The signature differentiator in the UI is the **AI Color Grader** panel — a chat-style assistant embedded in the develop rail that moves real sliders (not baked filters). For the landing page: show a **three-column develop layout** (left utilities · center photo · right sliders), a **horizontal filmstrip** under the canvas, and emphasize **neutral chrome + blue accent + trustworthy tool density**.

---

## 1. Design principles (match these on the site)

1. **Neutral chrome, color-neutral panels.** All UI backgrounds are gray. No warm tint, no orange, no gradient panels. The only saturated UI color is the blue accent.
2. **The photo is the hero.** The viewport (`#252525`) is slightly darker than panels. On the landing page, use a real or realistic edited photo in the mockup — not a flat gray rectangle.
3. **Lightroom familiarity.** Collapsible panels, label-left / track-center / value-right sliders, library grid, filmstrip, histogram on top of the develop rail.
4. **AI is a panel, not magic dust.** Show suggestion chips, a text field, and tool-activity lines — edits are undoable and map to real develop controls.
5. **Dense, not spacious.** This is a pro tool: 12px base font, 22px slider rows, 36px top bar, 22px status bar. Avoid airy marketing whitespace *inside* the app mockup.

---

## 2. Color tokens (exact — use on landing page)

Copy these literally for CSS on the website mockup frame.

### Surfaces
| Token | Hex | Use in app |
|---|---|---|
| `--bg` | `#1c1c1c` | App root, histogram canvas background |
| `--bg-panel` | `#2b2b2b` | Left/right rails, modals, library cells |
| `--bg-elevated` | `#383838` | Buttons default, hover fills |
| `--bg-viewport` | `#252525` | Canvas work area |
| Top bar gradient | `#303030` → `#262626` | Module bar (top) |
| Status bar | `#232323` | Bottom status strip |
| Filmstrip | `#1a1a1a` | Horizontal thumb strip |
| Panel header | `#2f2f2f` | Collapsible section heads |
| AI panel header | `#2c3340` | Slightly blue-gray tint (AI only) |

### Text
| Token | Hex | Use |
|---|---|---|
| `--text` | `#d4d4d4` | Primary labels, slider names |
| `--text-dim` | `#8a8a8a` | Secondary text, inactive tabs, values |
| `--text-faint` | `#6a6a6a` | Section subheads, metadata |
| Brand label | `#bdbdbd` | “Meratech” wordmark in top bar |
| AI panel title | `#9cc1ef` | “AI COLOR GRADER” header |
| AI tool activity | `#9cc1ef` | Assistant tool-call lines in log |

### Borders & dividers
| Token | Value | Use |
|---|---|---|
| `--border` | `#3a3a3a` | Inputs, modals, histogram frame |
| Panel dividers | `#161616`, `#1f1f1f`, `#111` | Rail edges, panel separators |
| Inner dividers | `#333` | Subsection rules |

### Accent & status
| Token | Hex | Use |
|---|---|---|
| `--accent` | `#4a90d9` | **Primary interactive color** — selected grid cell, active tab border, filmstrip selection, slider accent (native), primary modal button fill `#2e4763` |
| `--ok` | `#7fbf7f` | Engine ready, “Keep” AI verdict |
| `--error` | `#e06c75` | Errors, “Revert” AI verdict |
| Library badge gold | `#ffd479` | Star ratings / flags on thumbnails |

### What NOT to use on the landing mockup
- Warm orange (`#ff8a4c`) — **not in the app**
- Neon glows, glassmorphism, large rounded cards
- Pure black `#000` or pure white `#fff` for large surfaces
- Light mode

---

## 3. Typography

| Setting | Value |
|---|---|
| Family | `-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif` |
| Base size | **12px** (entire UI is compact) |
| Status bar | 11px |
| Panel section titles | 11px, **uppercase**, `letter-spacing: 0.07em`, `--text-dim` |
| Subsection labels (Tone, Presence, …) | 10px, uppercase, `letter-spacing: 0.06em`, `--text-faint` |
| Slider label | 11px, `--text` |
| Slider value | 11px, `--text-dim`, **tabular numerals** |
| Brand | 600 weight, `letter-spacing: 0.04em` |
| Modal title | 15px, 600 weight |

**Landing page headline typography (outside the mockup):** You can use a larger marketing font for hero copy, but the **app screenshot / UI frame** inside the hero should use the 12px dense system above.

---

## 4. Spacing, radius, elevation

| Element | Value |
|---|---|
| Top bar height | 36px |
| Status bar height | 22px |
| Viewport toolbar | 30px |
| Left rail width | 232px |
| Right rail width | 320px |
| Filmstrip height | 72px |
| Button padding | 4px 8px |
| Button radius | **4px** (small, LR-like — not pill) |
| Input radius | 4px |
| Modal radius | 8px |
| Grid cell radius | 5px |
| Filmstrip thumb radius | 2px |
| Slider track height | 3px |
| Slider thumb | 11px circle, `#d0d0d0`, 1px `#1a1a1a` border |
| Modal shadow | `0 12px 40px rgba(0,0,0,0.5)` |
| Modal backdrop | `rgba(0,0,0,0.55)` |

No photo drop-shadow in the app viewport — the canvas sits flat on `#252525`. For landing hero art you *may* add a subtle shadow around the whole app window frame, not around the photo inside the canvas.

---

## 5. Iconography

- **Style:** Custom inline SVG, Lucide-like — stroke `currentColor`, **1.8px** stroke width, ~16px in toolbars, 12–14px in compact controls.
- **Icons present:** Chevron (disclosure), Crop, Mask, Eyedropper, Brush, Sparkles, Wand, Compare (before/after), Loupe, Fit, Reset, Export.
- **Landing page:** Use the same thin-stroke line icon style for feature bullets; do not use filled Material icons.

---

## 6. App shell layout

### Global grid (all modes)
```
┌─ topbar ───────────────────────────────────────────── 36px
│  Meratech   [Library] [Develop]          Open… Export…
├─ main content ─────────────────────────────────────── 1fr
│  (Library grid  OR  Develop three-zone + filmstrip)
├─ statusbar ────────────────────────────────────────── 22px
│  ready · Apple M1 Max · EXIF… · status message
└──────────────────────────────────────────────────────
```

**Window:** 1440×900 default, min 1024×700, native macOS decorations, dark chrome `#161618` (Tauri window background behind webview).

### Top bar
- Dark gray **gradient** bar, not flat.
- Left: **Meratech** wordmark (muted gray, not accent-colored).
- Center-left: **Library** and **Develop** tabs — inactive tabs are dim gray text; active tab is white text on `#444` fill, 4px radius.
- Right: **Open…** and **Export…** (export icon + label). Export opens a modal.

### Status bar
- Shows: engine state (`ready` in green), GPU adapter name, camera EXIF line, right-aligned status message.
- Muted 11px type on `#232323`.

---

## 7. Library mode

```
┌─ toolbar ─────────────────────────────────────────────
│  Import Folder · Rebuild · search · filters · counts
├─ grid (scroll) ───────────────────────────────────────
│  [thumb] [thumb] [thumb] …
│  square cells, blue border when selected
└──────────────────────────────────────────────────────
```

- **Toolbar:** Import, rebuild index, text search, rating filter, flag filter, blurry/dupes checkboxes, photo count hint.
- **Grid:** `repeat(auto-fill, minmax(150px, 1fr))`, **1:1 aspect** thumbnails, `#2b2b2b` cell background.
- **Selection:** 2px **`#4a90d9`** border on selected cell.
- **Badges:** Gold `#ffd479` stars, pick/reject symbols, edit pencil — bottom overlay with dark text-shadow.
- **Keyboard culling (for copy):** 0–5 rate, P pick, X reject, Enter to develop.

**Landing suggestion:** Use Library mode for a secondary screenshot or “organize thousands of RAWs” section — grid of diverse photos with a few starred/picked.

---

## 8. Develop mode (primary landing mockup)

```
┌──────────┬─────────────────────────────┬──────────────┐
│ LEFT     │         CENTER              │ RIGHT RAIL   │
│ 232px    │                             │ 320px        │
│          │      Viewport (#252525)     │              │
│ Navigator│      [photo canvas]         │ Histogram    │
│ Presets  │                             │ Tool strip   │
│ Snapshots│─────────────────────────────│ AI Grader    │
│ History  │  viewport toolbar (zoom)    │ Basic        │
│          │                             │ Tone Curve   │
├──────────┴─────────────────────────────┤ HSL / Color  │
│  filmstrip (horizontal, 72px)          │ Color Grade  │
│  [▣][▣][▣][▣] current = blue border    │ Detail       │
└────────────────────────────────────────┴ Calibration  │
```

### Left rail (232px, `#2b2b2b`)
Collapsible **panels** (accordion):
1. **Navigator** — mini live preview of current frame; click to fit.
2. **Presets** — list + “new preset…” input.
3. **Snapshots** — named restore points.
4. **History** — last 30 edit labels.

Panel pattern: gray header bar `#2f2f2f`, uppercase title, chevron disclosure rotates 90° when open.

### Center
- **Viewport:** Full-width canvas, pan/zoom, double-click fit ↔ 1:1, neutral gray surround.
- **Toolbar below canvas:** Fit, 1:1, −, zoom %, +, Before/After toggle, Loupe. Dark `#1e1e1e` strip.

### Right rail (320px, `#2b2b2b`) — top to bottom
1. **Histogram** — pinned at top, `#262626` pin area. RGB channel overlays (red/green/blue translucent bars). Clip % below (▼ low / ▲ high).
2. **Tool strip** — Crop (disabled), Masking, WB eyedropper, Brush. Active tool gets blue accent border.
3. **AI Color Grader** — blue-tinted panel header (see §10).
4. **Basic** — WB Temp/Tint, Tone (Exposure, Contrast, Highlights, Shadows, Whites, Blacks), Presence (Vibrance, Saturation).
5. **Tone Curve** — graphical curve canvas + parametric sliders (collapsed by default).
6. **HSL / Color** — 8-band hue swatches + targeted sliders.
7. **Color Grading** — three interactive color wheels (shadows / midtones / highlights).
8. **Detail** — noise + sharpen (registry-driven).
9. **Calibration** — color calibration sliders.

### Filmstrip (bottom, horizontal)
- `#1a1a1a` background, 72px tall, horizontal scroll.
- Thumbs at 60% opacity; hover 100%; **current** photo: full opacity + **`#4a90d9` border**.

---

## 9. Core UI components (for mockup fidelity)

### Collapsible panel
- Header: 7px 10px padding, `#2f2f2f`, hover `#343434`.
- Title: uppercase 11px dim gray.
- Body: 8–10px padding.

### Slider row (most common control)
Three-column grid on one line:
```
Exposure    [━━━━━━━━●━━━━]    +0.45
  74px label    flex track         42px value
```
- Track: 3px tall, `#4a4a4a`, 2px radius.
- Thumb: 11px light gray circle.
- Value: click to type; double-click label or track to reset.
- Native `accent-color: #4a90d9` on generic range inputs in mask panel.

### Histogram
- ~252×72 canvas, `#1c1c1c` fill, 1px `#3a3a3a` border, 4px radius.
- RGB overlays: red `rgba(255,90,90,0.55)`, green `rgba(110,230,110,0.55)`, blue `rgba(110,140,255,0.55)`.

### Color grading wheels
- Conic hue ring + dark center hole — the **only** place saturated rainbow appears in the UI (it's a control, not chrome).

### HSL band picker
- Row of saturated hue swatches; active band gets **white 2px outline**.

### Export modal
- 380px wide, `#2b2b2b` panel.
- Grid: label | control rows (format, color space, long edge, quality, sharpen).
- Primary button: `#2e4763` background, blue accent border.

---

## 10. AI Color Grader (key marketing surface)

Visually distinct from other panels — slightly **blue-gray header** (`#2c3340` / title `#9cc1ef`).

**Contents:**
- **Suggestion chips** (pill-ish, 11px): Warmer, Cooler, Teal & orange, Lift shadows, Filmic contrast, Fix skin, Moody, Bright & airy.
- **Text input** + Sparkles send button.
- **Auto** and **Explain** buttons (Explain = diagnose only, no edits).
- **Activity log** — dark `#222` box; tool lines in light blue `#9cc1ef`.
- After a run: **Keep** (green border), **Revert** (red border), **Before/After** compare.

**Messaging for landing page:**
- “Describe the look — AI moves your real sliders”
- “Every change is undoable”
- “Your RAW never leaves the machine — only a small preview goes to the API” (when API key set)

**Mockup copy example:**
> User: “Warm it up, lift the shadows”  
> Activity: `set exposure.stops +0.3` · `set tone_curve.shadows +18`  
> Assistant: “Warmed exposure slightly and opened shadows…”

---

## 11. Interaction states

| State | Treatment |
|---|---|
| Hover (button) | Border `#555`, background `#404040` |
| Active / selected | Blue `#4a90d9` border; tab gets `#444` fill |
| Disabled | 40% opacity |
| Active tool (develop) | Blue border on tool button |
| Filmstrip current | Blue border, full opacity |
| Grid selected | 2px blue border |

Transitions are minimal (~0.12s on chevron rotation). No bounce animations.

---

## 12. Landing page composition suggestions

### Hero (recommended)
- **Headline:** Pro RAW editing on your Mac. GPU-powered. AI-assisted.
- **Sub:** Lightroom-style develop tools + local AI that edits through real sliders — not filters.
- **Visual:** Full **Develop mode** mockup at ~1440px wide, scaled down. Show:
  - A compelling portrait or landscape in the viewport
  - Right rail scrolled to **Basic** + partial **AI Color Grader**
  - Histogram with visible RGB curves
  - Filmstrip with 5+ thumbs, one selected in blue
- **Frame:** Optional dark outer glow; inside the mockup keep flat LR grays.

### Feature sections (pair copy + cropped UI)
| Feature | Show in mockup |
|---|---|
| GPU develop | Viewport + Basic sliders + histogram |
| AI grader | AI panel with chips + activity log |
| Masks | Tool strip + Masks panel expanded |
| Library / cull | Library grid with stars/flags |
| Export | Export modal overlay |
| Color tools | Color grading wheels or HSL bands |

### Social proof / specs strip
- Metal GPU · Scene-referred pipeline · sRGB / P3 / Adobe RGB export · Tauri native app

### CTA buttons on site (outside mockup)
- Primary download: can use **`#4a90d9`** to match app accent (or slightly brighter `#5a9fef` for web contrast).
- Secondary: ghost outline `#3a3a3a` border on dark bg — mirrors app buttons.

---

## 13. Ready-to-paste CSS (landing page mockup frame)

```css
:root {
  --bg: #1c1c1c;
  --bg-panel: #2b2b2b;
  --bg-elevated: #383838;
  --bg-viewport: #252525;
  --border: #3a3a3a;
  --text: #d4d4d4;
  --text-dim: #8a8a8a;
  --text-faint: #6a6a6a;
  --accent: #4a90d9;
  --ok: #7fbf7f;
  --error: #e06c75;
  --font-ui: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  color-scheme: dark;
}

/* Example app window chrome for hero mockup */
.app-frame {
  font-family: var(--font-ui);
  font-size: 12px;
  color: var(--text);
  background: var(--bg);
  border: 1px solid #111;
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 24px 80px rgba(0, 0, 0, 0.6);
}

.topbar-mock {
  height: 36px;
  padding: 0 12px;
  display: flex;
  align-items: center;
  gap: 14px;
  background: linear-gradient(#303030, #262626);
  border-bottom: 1px solid #111;
}

.tab-mock.active {
  color: #fff;
  background: #444;
  border-radius: 4px;
  padding: 5px 12px;
}

.rail-mock {
  background: var(--bg-panel);
  border-left: 1px solid #161616;
}

.slider-track-mock {
  height: 3px;
  background: #4a4a4a;
  border-radius: 2px;
  position: relative;
}
.slider-track-mock::after {
  content: "";
  position: absolute;
  width: 11px;
  height: 11px;
  border-radius: 50%;
  background: #d0d0d0;
  border: 1px solid #1a1a1a;
  top: 50%;
  left: 55%;
  transform: translate(-50%, -50%);
}

.panel-title-mock {
  font-size: 11px;
  letter-spacing: 0.07em;
  text-transform: uppercase;
  color: var(--text-dim);
}
```

---

## 14. Do / Don’t for the website editor

**Do**
- Match **cool blue** `#4a90d9` as the only strong accent in UI mockups.
- Keep panel backgrounds **desaturated gray**.
- Show **dense** 12px UI in product screenshots/mockups.
- Include **histogram + sliders + filmstrip** — they signal “real Lightroom-class tool.”
- Show the **AI panel** as differentiated (slight blue header tint).
- Use **uppercase panel headers** in mockups (BASIC, TONE CURVE, AI COLOR GRADER).

**Don’t**
- Don’t use orange/warm neon (that’s a separate aspirational doc — not the shipped app).
- Don’t use pill buttons / 16px rounded cards inside the app mockup.
- Don’t show a minimal 3-button UI — the product is panel-heavy by design.
- Don’t put heavy color casts on panel backgrounds — it undermines the “neutral develop environment” story.
- Don’t claim features not in UI (Map module, crop tool — crop button exists but disabled).

---

## 15. Product facts to align copy with UI

| Fact | Detail |
|---|---|
| Platform | macOS desktop (Tauri 2), Metal GPU |
| Modes | Library (grid/cull) + Develop (edit) |
| AI | Optional; requires `ANTHROPIC_API_KEY` |
| Export | JPEG / PNG / TIFF16 → sRGB, Display P3, Adobe RGB |
| Masks | Subject / background / sky / radial / linear / brush |
| Undo | ⌘Z / ⌘⇧Z in develop; full history panel |
| Before/after | `\` key or toolbar toggle |

---

## 16. Asset checklist for landing page

- [ ] Hero: full Develop layout mockup or real screenshot
- [ ] Crop: AI Color Grader panel close-up
- [ ] Crop: Color grading wheels or HSL bands
- [ ] Crop: Library grid with ratings
- [ ] Crop: Export modal
- [ ] Optional: short screen recording — slider move → histogram update → filmstrip navigation

---

*Document generated from the Meratech editor codebase. When the app UI changes, regenerate from `src/styles.css` + `src/App.tsx`.*
