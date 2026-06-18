# MeraRAW — UI Design System & Implementation Prompt

> **How to use this file:** Paste it into your MeraRAW app builder (the Lightroom-clone
> coding agent) as the design brief. It is the single source of truth for the app's
> look: a modern, dark, warm-orange photo-editor aesthetic. Match the tokens **exactly**.
> Where this doc and existing app styling disagree, this doc wins.

---

## 0. The brief in one paragraph

MeraRAW is a **dark, neutral, focused** desktop RAW editor. The chrome recedes so the
photo is the brightest thing on screen. Surfaces are near-black neutral grays that step
up in lightness as they come "forward" (canvas is darkest, panels are lighter). A single
**warm orange accent** (`#ff8a4c`) carries everything interactive — active states, the
filled portion of a slider, primary buttons, the AI assistant — and **nothing else**.
Accent is a spotlight, not a paint bucket. Type is clean and tight (Geist / system sans),
corners are soft, borders are hairline-thin white at low opacity, and elevation comes from
deep soft shadows plus the occasional orange glow. Think: Lightroom's restraint, with a
contemporary warm-neon edge.

**Three rules that define the feel**
1. **Neutral chrome, warm accent.** UI grays stay desaturated so they never bias how you
   read color in a photo. Orange appears only on interactive/active elements.
2. **The photo is the light source.** The work area is the darkest neutral; panels are a
   step lighter; the image carries a soft drop shadow so it floats.
3. **Every AI edit is a real control.** When the assistant changes something, it moves the
   actual slider and shows the value — never a hidden, baked result.

---

## 1. Color tokens

All values are literal. Use these names (or your own) but keep the values.

### Neutrals / surfaces
| Token | Value | Use |
|---|---|---|
| `--bg` | `#0a0a0b` | App background, darkest. Canvas / work area. |
| `--bg-1` | `#0f0f11` | Raised surface: cards, the app frame, input rows. |
| `--bg-2` | `#141417` | Panels (left filmstrip rail, right develop panel), top bar. |
| `--bg-3` | `#1b1b1f` | Controls: input/select fields, slider tracks, hover fills. |
| `--text` | `#f4f4f5` | Primary text, active values. |
| `--muted` | `#a6a6ad` | Secondary text, slider labels, body copy. |
| `--faint` | `#75757d` | Tertiary: section headers, captions, placeholder, disabled labels. |

> Surface logic: **canvas `--bg` (darkest) → panels `--bg-2` → controls `--bg-3` (lightest)**.
> Going "up" in the z-stack means going up ~1 step in lightness.

### Borders
| Token | Value | Use |
|---|---|---|
| `--border` | `rgba(255,255,255,0.08)` | Default hairline divider between panels, rows, cards. |
| `--border-strong` | `rgba(255,255,255,0.14)` | Emphasized edges: focused inputs, the app frame, primary surfaces. |

Borders are **white at low opacity**, never solid gray lines. This keeps edges crisp on any
surface shade.

### Accent (the only chromatic color)
| Token | Value | Use |
|---|---|---|
| `--accent` | `#ff8a4c` | The core orange. Slider fills, primary button base, active states, glows. |
| `--accent-2` | `#ffb169` | Lighter warm tint. Accent text, eyebrow labels, "boosted" slider values, gradient top. |
| `--accent-soft` | `rgba(255,138,76,0.13)` | Tinted fill behind accent elements (user chat bubble, selected highlight). |
| `--accent-edge` | `rgba(255,138,76,0.25)` | Border for accent-soft elements. |

### Status
| Token | Value | Use |
|---|---|---|
| `--good` | `#54d6a0` | Success, "picked"/kept flag, checkmarks, valid toast. |
| `--warn` | `#febc2e` | Warning, "review" flag. |
| `--danger` | `#ff5f57` | Destructive, "rejected" flag, error toast. |
| `--danger-text` | `#ff8a8a` | Inline error text on dark (softer than pure red). |

### Signature gradients
```
/* Primary button / send button */
--grad-accent: linear-gradient(180deg, #ff9a5e, #ff8a4c);

/* Big display headlines / hero numbers — warm-to-pink sweep */
--grad-headline: linear-gradient(120deg, #ffb169, #ff8a4c 55%, #ff5e7a);

/* App logo mark / small badges */
--grad-mark: linear-gradient(150deg, #ff8a4c, #ff5e7a);

/* Slider fill */
--grad-slider: linear-gradient(90deg, #ff8a4c, #ffb169);
```

### Glows (accent bloom)
```
--glow-accent:  0 0 12px 1px rgba(255,138,76,0.60);   /* dots, active markers */
--glow-button:  0 8px 22px rgba(255,138,76,0.28);     /* primary button lift  */
```

---

## 2. Typography

**Family:** `Geist`, then `-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`.
Tabular numerals **on** for any numeric readout (slider values, histogram, metadata, EXIF).

| Role | Size | Weight | Letter-spacing | Color |
|---|---|---|---|---|
| Display / hero | `clamp(2.4rem, 6vw, 4rem)` | 600 | `-0.035em` | `--text` (or `--grad-headline`) |
| Section title | `clamp(1.7rem, 3.4vw, 2.4rem)` | 600 | `-0.03em` | `--text` |
| Subsection / feature title | `clamp(1.6rem, 3vw, 2.1rem)` | 600 | `-0.025em` | `--text` |
| Body | `16px` / line-height `1.6` | 400 | `-0.011em` | `--muted` |
| Body strong | `16px` | 500 | — | `--text` |
| Control label (slider name) | `0.70–0.74rem` | 400 | — | `--muted` |
| **Panel section header** | `0.68rem` | 500 | `0.12em` **UPPERCASE** | `--faint` |
| Eyebrow / tag label | `0.78rem` | 500 | `0.02em` | `--accent-2` |
| Caption / meta | `0.82–0.83rem` | 400 | — | `--faint` |
| Numeric value | `0.70rem` | 400–500 | tabular-nums | `--text` (or `--accent-2` when boosted) |

**Panel headers are the signature type detail**: tiny, uppercase, wide-tracked, faint
(`BASIC`, `TONE`, `HISTOGRAM`, `DETAIL`, `LIBRARY · 248 PHOTOS`).

---

## 3. Spacing, radius, elevation

**Spacing scale (rem):** `0.25 · 0.375 · 0.5 · 0.6 · 0.75 · 0.85 · 1 · 1.25 · 1.5 · 2 · 2.5 · 3 · 4.5`.
Panel inner padding ~`0.85rem`; card padding `1.1–1.75rem`; slider row gap `0.6rem`.

**Radius**
| Token | Value | Use |
|---|---|---|
| `--r-pill` | `999px` | Buttons, chips, badges, input fields, slider tracks, the AI input bar. |
| `--r-lg` | `14–18px` | Cards, panels, the app window frame, media tiles. |
| `--r-md` | `10–12px` | Inputs/selects, chat bubbles. |
| `--r-sm` | `5–7px` | Thumbnails, histogram box, small chips, the logo mark. |

> Note: the landing page uses **pill** inputs for marketing flair. **In the app, prefer
> `--r-md` (10px) for text inputs/selects** — pills read oddly in dense panels. Keep pills
> for buttons, chips, and the AI command bar.

**Borders:** `1px solid var(--border)` everywhere by default; `--border-strong` for focus
and the outer app frame.

**Elevation (shadows)**
```
--shadow-card:   0 30px 80px -50px rgba(0,0,0,0.90);   /* panels, cards          */
--shadow-window: 0 40px 120px -40px rgba(0,0,0,0.90);  /* app frame / big modals */
--shadow-photo:  0 18px 50px -18px rgba(0,0,0,0.85);   /* the image on canvas    */
--shadow-knob:   0 1px 4px rgba(0,0,0,0.60);           /* slider handle          */
```
Light comes from above; shadows are large, soft, and very dark. No hard 1px drop shadows.

---

## 4. Iconography & motion

- **Icons:** thin line icons, `1.5–2px` stroke, `currentColor`, ~`16px` in buttons,
  ~`14–15px` inside small controls. (Lucide-style is a perfect match.)
- **Motion:** quick and subtle. Transitions `0.12–0.2s ease`. Buttons lift `translateY(-1px)`
  on hover. Disclosure markers rotate `+45°`. No bouncy/long animations — this is a tool.
- **Focus:** never remove focus rings; use `--border-strong` or a `1px` accent ring.

---

## 5. Core components

### Buttons
- **Primary** — `background: var(--grad-accent)`, text `#1a0e06` (near-black warm), weight
  600, radius pill, padding `0.7rem 1.1rem`, `box-shadow: inset 0 1px 0 rgba(255,255,255,0.25), var(--glow-button)`.
  Hover: `translateY(-1px)`.
- **Ghost / secondary** — `background: rgba(255,255,255,0.04)`, `border: 1px solid var(--border-strong)`,
  text `--text`. Hover: bg `rgba(255,255,255,0.08)`.
- **Disabled** — `opacity: 0.55`, `cursor: not-allowed`, no hover transform. (Use this for
  "coming soon" / unavailable actions.)
- **Small** — padding `0.5rem 0.85rem`, font `0.85rem`.

### Text inputs & selects
- `background-color: var(--bg-3)`, `border: 1px solid var(--border-strong)`, text `--text`,
  radius `10px`, padding `0.625rem 0.75rem`.
- Placeholder `--faint`. **Focus:** `border-color: var(--accent)` (no glow needed).
- Custom select chevron, `--faint` colored, `no-repeat` right-center. (Don't use the
  `background` shorthand to recolor — it resets the chevron's repeat/position. Set
  `background-color` only.)

### Sliders (the heart of the develop panel)
- **Bipolar / center-anchored.** Track is a thin `3px` pill, `background: var(--bg-3)`.
  The fill grows **from the center (0)** outward toward the handle:
  `background: var(--grad-slider)`.
- **Handle:** `10px` white circle, `box-shadow: var(--shadow-knob)`.
- **Value readout** sits right-aligned, tabular-nums, `--text`. When the value is a positive
  "boost," tint it `--accent-2`. Negative values stay `--text`/`--muted`.
- Row layout: `label … value` on top line, full-width track beneath.
- Example states to support: `Exposure +0.45`, `Contrast +12`, `Highlights −38`,
  `Shadows +24`, `Whites +8`, `Blacks −14`, `Temp +8`, `Vibrance +16`.

### Histogram
- Boxed: `var(--bg-1)` fill, `1px var(--border)`, radius `5px`, height ~`56px`.
- Curve: fill `rgba(255,138,76,0.35)`, stroke `rgba(255,177,105,0.8)` ~`0.8px`.
  (Per-channel R/G/B overlays can use translucent red/green/blue, but the master/luma
  curve uses the accent.)

### Module switcher / tabs (top bar)
- Top bar `--bg-2`, height ~`38px`, hairline bottom border. macOS traffic lights at left
  (`#ff5f57 / #febc2e / #28c840`).
- Tabs are small (`0.72rem`), `--faint` when idle; **active tab** gets `background: var(--bg-3)`
  and `color: var(--text)`. Right side shows the current file name in `--faint`.
- Labels: `Library · Develop · Map · Export`.

### Filmstrip / thumbnails
- Tiles: `aspect-ratio: 3/2`, radius `4–5px`, `1px` transparent border by default.
- **Selected:** `border-color: var(--accent)` + `box-shadow: 0 0 0 1px var(--accent)`.
- Cull flags as a small `7px` dot top-left: kept `--good`, review `--warn`, rejected `--danger`.

### Canvas / work area
- Darkest surface (`--bg`). For transparency/letterboxing use a **22px checkerboard**:
  `repeating-conic-gradient(#1a1a1d 0% 25%, #161618 0% 50%) 0 0 / 22px 22px`.
- The photo sits centered with `--shadow-photo` and a `6px` radius so it floats.

### AI assistant (signature surface)
- Lives as a docked bar (bottom of the develop view) or a side panel.
- **User message bubble:** `background: var(--accent-soft)`, `border: 1px solid var(--accent-edge)`,
  text `--accent-2`, radius `12px` with a tightened bottom-right corner (`3px`).
- **Assistant bubble:** `background: var(--bg-3)`, `border: 1px solid var(--border)`,
  text `--text`, tightened bottom-left corner.
- **Edit chips** (what it changed): small `0.72rem` tabular pills inside the bubble,
  `background: rgba(255,255,255,0.06)`, text `--accent-2` — e.g. `Temp +8` `Shadows +24`.
- **Command input:** pill bar, `--bg-3`, `--border-strong`, placeholder `--faint`
  ("Ask MeraRAW to adjust this photo…"), with a circular **send button** using
  `var(--grad-accent)` and a `14px` arrow.
- **Principle:** every assistant action also moves the real sliders and is fully
  undoable/editable. Surface the chips so the user sees exactly what moved.

### Cards / panels
- `--bg-1` or `--bg-2` fill, `1px --border` (or `--border-strong` for hero cards),
  radius `14–18px`, `--shadow-card`. Optional radial accent wash for "featured":
  `radial-gradient(400px 200px at 50% -60%, rgba(255,138,76,0.16), transparent 70%)`.

### Chips / badges / pills
- Neutral chip: `0.78rem`, `--muted`, `1px --border`, `--bg-1`, pill radius, padding `0.35rem 0.75rem`.
- Accent tag: `--accent-2` text, `1px var(--accent-edge)` border, transparent fill.
- Eyebrow (section kicker): an `--accent` dot with `--glow-accent` + `--accent-2` label.

### Checklist items
- Custom check drawn with two borders rotated `-45°` in `--good` (no icon font needed),
  or a Lucide check in `--good`.

### Toasts (you already use `sonner`)
- Dark surface `--bg-2`, `1px --border`, radius `--r-md`. Success accent `--good`,
  error `--danger`/`--danger-text`. Keep them compact, top-right or bottom-right.

### Menus, popovers, tooltips
- `--bg-2`/`--bg-1`, `1px --border`, `--shadow-card`, radius `--r-md`. Hover row:
  `rgba(255,255,255,0.06)`. Tooltips tiny (`0.78rem`), `--bg-3`, `--faint`/`--text`.

### Scrollbars (panels get long)
- Thin (`8–10px`), transparent track, thumb `rgba(255,255,255,0.12)` →
  `rgba(255,255,255,0.22)` on hover, fully rounded.

---

## 6. App layout (how the editor screen composes)

```
┌───────────────────────────────────────────────────────────────────────┐
│  ● ● ●   Library  [Develop]  Map  Export        MeraRAW — file.CR3      │ top bar (--bg-2)
├──────┬──────────────────────────────────────────────┬───────────────────┤
│ film │                                              │  HISTOGRAM        │
│ strip│              CANVAS (--bg, checker)          │  ┌────────────┐   │
│      │            ┌──────────────────────┐          │  └────────────┘   │
│ [▣]  │            │      the photo       │  shadow  │  BASIC            │
│ [▣]  │            │   (floats, --bg)     │          │  Exposure  +0.45  │
│ [▣]  │            └──────────────────────┘          │  ────●──────────  │
│      │                                              │  Contrast   +12   │
│      │                                              │  ──────●────────  │  develop
├──────┴──────────────────────────────────────────────┴───────────────────┤  panel
│  AI:  "warm it up, lift the shadows"   →  Temp +8 · Shadows +24    [↗]   │ (--bg-2)
└───────────────────────────────────────────────────────────────────────┘
```

- **Left rail** (`--bg-2`): filmstrip / library navigation.
- **Center** (`--bg`): canvas, the only place the photo's brightness dominates.
- **Right panel** (`--bg-2`): collapsible sections (`BASIC`, `TONE CURVE`, `HSL`,
  `COLOR`, `DETAIL`, `LENS`, `MASKS`) with uppercase faint headers and slider stacks.
- **Bottom** (`--bg-1`/`--bg-2`): the AI command bar, full width.
- Every region separated by a single `--border` hairline.

---

## 7. Interaction states (apply consistently)

| State | Treatment |
|---|---|
| Hover (interactive) | Surface lightens one step (`rgba(255,255,255,0.04→0.08)`); buttons lift `-1px`. |
| Active / selected | `--accent` border + `box-shadow: 0 0 0 1px var(--accent)`; or `--bg-3` fill for tabs. |
| Focus (keyboard) | `border-color: var(--accent)` or a `1px` accent ring — always visible. |
| Disabled | `opacity: 0.55`, `cursor: not-allowed`, no hover effects. |
| Boosted value | Numeric readout tinted `--accent-2`. |
| Dragging a slider | Handle scales subtly / fill brightens; show live value. |

---

## 8. Accessibility & color discipline

- **Contrast:** `--text` on any surface ≥ 7:1; `--muted` ≥ 4.5:1; `--faint` is for
  non-essential labels only (don't put critical info in `--faint`).
- **Accent on dark:** `--accent` text on `--bg` is legible, but for long text prefer
  `--accent-2`. On the orange button, text is the near-black `#1a0e06` for max contrast.
- **Don't rely on color alone** for cull flags / status — pair with icon or position.
- **Protect color perception:** keep all chrome desaturated. Never tint the canvas,
  panels, or histogram background with the accent — it skews how the user reads the photo.
  Accent is for controls and the active item, full stop.

---

## 9. Do / Don't

**Do**
- Keep the canvas the darkest, brightest-photo surface.
- Use orange only for interactive/active/AI elements and slider fills.
- Use hairline white-opacity borders and big soft shadows for separation.
- Show AI edits as real, labeled, undoable slider moves.
- Use tabular numerals for every value readout.

**Don't**
- Don't add a second accent hue. One orange family only.
- Don't use pure black (`#000`) or pure white (`#fff`) for large surfaces/text — use the
  tokens.
- Don't paint panels/backgrounds with the accent or any saturated color.
- Don't use heavy/long animations; this is a precision tool.
- Don't use solid mid-gray `1px` borders — use white-at-low-opacity.

---

## 10. Ready-to-paste tokens

### CSS custom properties (framework-agnostic)
```css
:root {
  /* surfaces */
  --bg: #0a0a0b;
  --bg-1: #0f0f11;
  --bg-2: #141417;
  --bg-3: #1b1b1f;

  /* text */
  --text: #f4f4f5;
  --muted: #a6a6ad;
  --faint: #75757d;

  /* borders */
  --border: rgba(255, 255, 255, 0.08);
  --border-strong: rgba(255, 255, 255, 0.14);

  /* accent */
  --accent: #ff8a4c;
  --accent-2: #ffb169;
  --accent-soft: rgba(255, 138, 76, 0.13);
  --accent-edge: rgba(255, 138, 76, 0.25);

  /* status */
  --good: #54d6a0;
  --warn: #febc2e;
  --danger: #ff5f57;
  --danger-text: #ff8a8a;

  /* gradients */
  --grad-accent: linear-gradient(180deg, #ff9a5e, #ff8a4c);
  --grad-headline: linear-gradient(120deg, #ffb169, #ff8a4c 55%, #ff5e7a);
  --grad-mark: linear-gradient(150deg, #ff8a4c, #ff5e7a);
  --grad-slider: linear-gradient(90deg, #ff8a4c, #ffb169);

  /* effects */
  --glow-accent: 0 0 12px 1px rgba(255, 138, 76, 0.6);
  --glow-button: 0 8px 22px rgba(255, 138, 76, 0.28);
  --shadow-card: 0 30px 80px -50px rgba(0, 0, 0, 0.9);
  --shadow-window: 0 40px 120px -40px rgba(0, 0, 0, 0.9);
  --shadow-photo: 0 18px 50px -18px rgba(0, 0, 0, 0.85);
  --shadow-knob: 0 1px 4px rgba(0, 0, 0, 0.6);

  /* radius */
  --r-pill: 999px;
  --r-lg: 16px;
  --r-md: 10px;
  --r-sm: 6px;

  /* type */
  --font-sans: "Geist", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;

  color-scheme: dark;
}
```

### Tailwind v4 (`@theme`) — if the app uses Tailwind v4
```css
@theme {
  --color-bg: #0a0a0b;
  --color-bg-1: #0f0f11;
  --color-bg-2: #141417;
  --color-bg-3: #1b1b1f;
  --color-text: #f4f4f5;
  --color-muted: #a6a6ad;
  --color-faint: #75757d;
  --color-accent: #ff8a4c;
  --color-accent-2: #ffb169;
  --color-good: #54d6a0;
  --color-warn: #febc2e;
  --color-danger: #ff5f57;
  --radius-pill: 999px;
  --radius-lg: 16px;
  --radius-md: 10px;
  --font-sans: "Geist", ui-sans-serif, system-ui, sans-serif;
}
```

---

*MeraRAW design system — distilled from the meratech.co landing page. Keep it dark, keep it
neutral, let the orange and the photo do the talking.*
