# MeraRAW — UI / UX Interactive Element Inventory

**Audience:** Figma Pro intern recreating / refining the editor UI  
**Source of truth:** live React UI in `src/` (verified against code + Develop screenshot)  
**Date:** 2026-07-13  

### How to use this in Figma
- Treat each row as a **component** (or component variant).
- Column **Type** → Figma component kind (Button, Icon button, Slider, etc.).
- Column **Category** → organize frames / pages (Navigation, Edit adjust, …).
- Column **States to design** → variants you should include (default / hover / active / disabled / selected).
- Prefer **Lightroom-classic density**: small type, dark chrome, subtle active states (white text, blue selection, white border on swatches).

### Shared patterns (design once, reuse everywhere)

| Pattern | Type | Behavior | States to design |
|---|---|---|---|
| Panel header + chevron | Chevron collapse | Expands / collapses a section | Expanded, collapsed |
| Param slider row | Label + range slider + value field | Scrub live; click value to type; **double-click label resets** | Default, dragging, editing value, disabled |
| Tab (top / rail) | Text or icon tab | Switches module or right-rail panel | Default, hover, active, disabled |
| List / tree row | Selectable row | Opens / filters / selects | Default, hover, selected |
| Thumbnail | Image button | Select / open image | Default, selected (blue border), hover |
| Modal | Dialog + backdrop | Backdrop click usually closes | Open, busy/disabled primary |
| Chip / swatch | Toggle chip | Selects a filter or hue band | Default, selected (white border) |
| Icon button | Compact control | Tool toggle or one-shot action | Default, hover, active/on, disabled |

---

## 1. Top navigation

| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Logo + **MeraRAW** + **BETA** badge | Brand mark (non-interactive) | Product identity | Brand | — |
| **Library** | Tab | Switch to Library module | Navigation | Default, active |
| **Develop** | Tab | Switch to Develop (disabled until an image is open) | Navigation | Default, active, disabled |
| **Education** | Tab | Switch to Education / guides | Navigation | Default, active |
| **Open…** | Button | Native file picker → open one image | File I/O | Default, hover |
| **Export…** (+ upload icon) | Button | Open Export dialog (disabled until image open) | Export | Default, hover, disabled |

---

## 2. Left sidebar (Develop)

### Navigator
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| **Navigator** header + chevron | Chevron collapse | Show / hide navigator | Navigation | Expanded, collapsed |
| Mini preview thumbnail | Clickable canvas | Click → fit image in viewport | View / zoom | Default, hover |

### Snapshots
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| **Snapshots** header + chevron | Chevron collapse | Show / hide snapshots | History | Expanded, collapsed |
| “No snapshots.” | Empty state text | Placeholder when list empty | History | — |
| Snapshot name row | List item button | Restore that snapshot | History | Default, hover, selected |
| `snapshot name…` | Text input | Type name for new snapshot | History | Default, focus |
| **+** (or Enter in field) | Icon button / submit | Create snapshot from current edit | History | Default, hover, disabled |

### History
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| **History** header + chevron | Chevron collapse | Show / hide history list | History | Expanded, collapsed |
| “No edits yet.” / history rows | Read-only list | Shows edit steps (not clickable undo targets in UI) | History | — |

### Local file browser
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| **LOCAL** / Browse section | Section header | Local disk browsing | Catalog / files | — |
| **Browse…** / **…** | Button / icon button | Pick another folder to browse | File I/O | Default, hover |
| Folder row + ▸/▾ chevron | Tree row + collapse | Expand / collapse folder | Catalog / files | Collapsed, expanded, hover |
| File row (e.g. `_DSC4843.ARW`) + green file icon | Tree file row | Open that file in Develop | File I/O | Default, hover, **selected (blue)** |

---

## 3. Center viewport (Develop)

| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Main image canvas | Drag / scroll / double-click surface | Pan (drag), zoom (wheel), double-click toggles fit ↔ 1:1 | View / zoom | Default, panning, WB-sample cursor, brush cursor |
| Canvas (WB eyedropper on) | Click target | Sample neutral point → set white balance | Edit adjust | Crosshair / pipette cursor |
| Canvas (Brush mask on) | Paint surface | Paint into brush mask | Masking | Brush cursor |
| Crop frame + 8 handles | Overlay + handles | Move / resize crop (when Crop tool active) | Crop | Idle, dragging, handle hover |
| Outside crop drag | Rotate gesture | Rotate crop angle | Crop | Rotating |
| ⌘/Ctrl + drag inside crop | Straighten gesture | Draw straighten line | Crop | Drawing |

### Viewport chrome (bottom of preview)
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Fit / frame icon | Icon button | Fit image to viewport | View / zoom | Default, hover, active |
| **1:1** | Button | Actual-pixels zoom | View / zoom | Default, hover, active |
| **−** / **+** | Buttons | Zoom out / in | View / zoom | Default, hover, disabled at limits |
| Zoom % readout | Label (sometimes editable pattern) | Shows current zoom | View / zoom | — |
| Zoom slider (if shown) | Range slider | Continuous magnification | View / zoom | Default, dragging |

> Screenshot also shows fit / 1:1 / ± near the image; the **look toggles** and Before/After live on the bottom toolbar (next section).

---

## 4. Bottom toolbar + filmstrip (Develop)

### Look / compare toolbar
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| **Neutral** | Toggle | Flat scene-referred display | View / look | Default, **active** |
| **Camera** | Toggle | Punchy camera / JPEG-like look | View / look | Default, active |
| **Filmic** | Toggle | AgX filmic rolloff | View / look | Default, active |
| **Original** | Toggle | Demosaiced sensor, no camera profile look | View / look | Default, active |
| **Before/After** (+ split icon) | Toggle | Bypass edits for compare | View / compare | Off, on |
| Loupe / magnifier | Icon button | Jump to 1:1 | View / zoom | Default, hover |

### Filmstrip
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Filmstrip thumbnail | Thumbnail | Open sibling image in folder | Catalog / files | Default, hover, **selected (blue border)** |
| Filmstrip scroll | Scroll container | Browse siblings | Catalog / files | — |

### Always-on chrome
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| **Report a problem** (speech bubble) | FAB / button | Opens feedback modal | Feedback | Default, hover |
| Status bar (ready · GPU · ARW badge · EXIF · export msg) | Read-only bar | System + file metadata | Status | ready / error variants |

---

## 5. Right tool rail (vertical icons)

| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Presets (circle cluster) | Icon tab | Show Presets panel | Navigation | Default, active |
| Edit (slider bars) | Icon tab | Show Edit panel (**default**) | Navigation | Default, active |
| Crop | Icon tab | Show Crop panel + crop tool | Navigation | Default, active |
| Remove (eraser) | Icon tab | Spot removal (**placeholder / coming soon**) | Navigation | Default, active |
| Masking (dashed circle) | Icon tab | Show Masking panel | Navigation | Default, active |

---

## 6. Edit panel (right — default Develop view)

### Header
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| **Edit** title | Panel title | — | Navigation | — |
| **Auto** | Button | Auto-tone from image stats | Edit adjust | Default, hover, busy |
| **B&W** | Button | Convert to black & white | Edit adjust | Default, hover, active |

### Histogram
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Histogram graph | Display (mostly non-interactive) | Shows tonal distribution | Edit adjust | — |
| Clipping triangles (if present) | Icon toggles | Show highlight / shadow clipping overlays | View / overlay | Off, on |

### AI Color Grader (collapsible)
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Section chevron | Chevron collapse | Expand / collapse | Navigation | Expanded, collapsed |
| Look chips (Warmer, Cooler, Teal & orange, …) | Chip buttons | Run AI with preset prompt | AI edit | Default, hover, busy |
| `Describe the look…` | Text input | Custom AI prompt | AI edit | Default, focus |
| Apply (sparkles) | Icon button | Run typed prompt | AI edit | Default, hover, disabled |
| **Auto** (wand) | Button | Fully automatic AI grade | AI edit | Default, hover, busy |
| **Explain** | Button | AI diagnosis only (no edits) | AI edit | Default, hover |
| **Keep** / **Revert** | Buttons | Accept or undo AI pass | History | Default, hover |
| Before/After (in AI review) | Toggle | Compare during AI review | View / compare | Off, on |

### Profile
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Section chevron | Chevron collapse | Expand / collapse | Navigation | Expanded, collapsed |
| Profile dropdown (e.g. **MeraRAW Standard**) | Select / dropdown | Switch camera / look profile | Edit adjust | Closed, open, selected option |

### Demosaic (RAW only)
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Section chevron | Chevron collapse | Expand / collapse | Navigation | Expanded, collapsed |
| Algorithm dropdown | Select | Change demosaic algorithm (re-decodes) | Edit adjust | Closed, open |

### Light
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Section chevron | Chevron collapse | Expand / collapse | Navigation | Expanded, collapsed |
| **Exposure** | Slider row | Exposure stops | Edit adjust | Shared slider states |
| **Contrast** | Slider row | Contrast | Edit adjust | Shared slider states |
| **Highlights** | Slider row | Highlight recovery | Edit adjust | Shared slider states |
| **Shadows** | Slider row | Shadow lift | Edit adjust | Shared slider states |
| **Whites** | Slider row | White point | Edit adjust | Shared slider states |
| **Blacks** | Slider row | Black point | Edit adjust | Shared slider states |

### White Balance
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Section chevron | Chevron collapse | Expand / collapse | Navigation | Expanded, collapsed |
| Eyedropper | Icon toggle | Activate WB sampler on viewport | Edit adjust | Off, **on** |
| **Temp** | Slider row | Color temperature | Edit adjust | Shared slider states |
| **Tint** | Slider row | Green ↔ magenta | Edit adjust | Shared slider states |

### Color
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Section chevron | Chevron collapse | Expand / collapse | Navigation | Expanded, collapsed |
| **Vibrance** | Slider row | Smart saturation | Edit adjust | Shared slider states |
| **Saturation** | Slider row | Global saturation | Edit adjust | Shared slider states |

### Tone Curve
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Section chevron | Chevron collapse | Expand / collapse | Navigation | Expanded, collapsed |
| **RGB / R / G / B** | Segmented tabs | Active curve channel | Edit adjust | Default, active |
| Curve canvas | Interactive graph | Add / drag points; right-click menu | Edit adjust | Idle, dragging point |
| Context: Delete / Reset / Reset All / Flatten | Context menu | Point & curve ops | Edit adjust | Menu open |

### HSL / Color Mix
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Section chevron | Chevron collapse | Expand / collapse | Navigation | Expanded, collapsed |
| Red → Magenta swatches (8) | Color swatch buttons | Select hue band (Orange = skin) | Edit adjust | Default, **selected + label** |
| **Hue** / **Saturation** / **Luminance** | Slider rows | Adjust selected band | Edit adjust | Shared slider states |

### Split Toning / Color Grading
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Section chevron | Chevron collapse | Expand / collapse | Navigation | Expanded, collapsed |
| **Perceptual / Classic / Light** | Radio / segmented | Grading model | Edit adjust | Default, active |
| Shadows / Midtones / Highlights wheels | Color wheels | Drag hue/sat; dbl-click reset | Edit adjust | Idle, dragging |
| Zone / chroma / range sliders | Slider rows | Fine-tune grade | Edit adjust | Shared slider states |

### Look LUT
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Section chevron | Chevron collapse | Expand / collapse | Navigation | Expanded, collapsed |
| **Load LUT…** / **Replace…** | Button | Pick `.cube` file | Edit adjust | Default, hover |
| **Remove** | Button | Clear LUT | Edit adjust | Default, hover, disabled if none |
| **Opacity** | Slider row | LUT blend | Edit adjust | Shared slider states |

### Detail
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Section chevron | Chevron collapse | Expand / collapse | Navigation | Expanded, collapsed |
| **Luma NR** / **Chroma NR** / **Detail Preserve** | Slider rows | Noise reduction | Edit adjust | Shared slider states |
| **Sharpen** / **Radius** / **Detail** | Slider rows | Capture sharpening | Edit adjust | Shared slider states |

### Calibration (often collapsed)
| Label / icon | Type | What it does | Category | States to design |
|---|---|---|---|---|
| Section chevron | Chevron collapse | Expand / collapse | Navigation | Expanded, collapsed |
| Red/Green/Blue Hue & Sat + Shadow Tint | Slider rows | Camera calibration | Edit adjust | Shared slider states |

### Placeholder sections (header only today)
Clarity & Dehaze, Vignette, Optics, Geometry — **chevron + copy only**, no controls yet. Design as empty/disabled panels if you want future slots.

---

## 7. Other right-rail panels (not in default screenshot)

### Presets
| Label / icon | Type | What it does | Category |
|---|---|---|---|
| Search field | Text input | Filter presets / tags | Presets |
| Tag chips (All, Natural, Moody, …) | Toggle chips | Filter by tag | Presets |
| Preset row | List button | Apply preset | Edit adjust |
| Save preset as… | Text input | Save current look | Presets |

### Crop panel
| Label / icon | Type | What it does | Category |
|---|---|---|---|
| **Start crop** / **Done** / **Cancel** | Buttons | Enter / commit / discard crop | Crop |
| Aspect lock 🔒/🔓 | Icon toggle | Lock aspect | Crop |
| Aspect dropdown | Select | As Shot, 1×1, 4×5, 16×9, … | Crop |
| Flip aspect ⇄ | Icon button | Swap orientation | Crop |
| **Angle** | Slider | Straighten | Crop |
| ↺90° / ↻90° / ↔ / ↕ | Buttons | Rotate / flip | Crop |
| **Reset crop** | Button | Clear crop module | Crop |

### Masking
| Label / icon | Type | What it does | Category |
|---|---|---|---|
| Brush tool toggle | Icon toggle | Paint mode on viewport | Masking |
| Subject / Background / Sky | Buttons | AI semantic masks | Masking |
| Radial / Linear / Brush | Buttons | Manual masks | Masking |
| Mask list row | Selectable row | Select mask for scoped edits | Masking |
| 👁 / ✕ | Icon buttons | Overlay visibility / delete | Masking |
| Opacity / Feather | Sliders | Mask edge / strength | Masking |
| Invert | Checkbox | Invert mask | Masking |

### Remove
Placeholder only — no interactive controls yet.

---

## 8. Library module (top tab)

| Label / icon | Type | What it does | Category |
|---|---|---|---|
| All Photos / folders / albums | Nav list | Filter catalog | Catalog |
| **+** (albums) | Icon button | Create album | Catalog |
| **+ Add Photos…** / **Browse…** | Buttons | Import flow | File I/O |
| **Rebuild** | Button | Rebuild index | Catalog |
| Search | Text input | Faceted search | Catalog |
| Sort / rating / flag dropdowns | Selects | Sort & filter | Catalog |
| blurry / dupes | Checkboxes | Quality filters | Catalog |
| Grid / Square | Toggle | Grid layout | View |
| Grid thumbnail | Thumbnail | Select; double-click → Develop | Catalog |
| Keyword chips + add field | Tags + input | Edit keywords | Catalog |
| **Export N…** | Button | Batch export selection | Export |
| Add to album… | Select | Assign selection | Catalog |

### Import Review modal
Select all / none, candidate cells (toggle), Cancel, **Import N photos**.

---

## 9. Education module

| Label / icon | Type | What it does | Category |
|---|---|---|---|
| Guide list items | Nav list | Switch article | Navigation |
| Previous / Next | Buttons | Browse guides | Navigation |
| Support development | Link button | Open Early Supporter modal | License / monetization |

---

## 10. Global modals & overlays

### Report a problem
Message textarea, optional email, Cancel, Send, Done; backdrop closes.

### Export dialog
Choose folder, Format, Color space, Size, Quality slider (JPEG/HEIC), Sharpen, Strip EXIF checkbox, Copyright, Watermark, Reveal in Finder, Cancel (batch), Close, **Export**.

### Settings (opened via menu / shortcut — no top-nav button)
Support Development, View on meratech.co, Close.

### Early Supporter
Email / password (optional checkout), Get Lifetime Access, Continue free, Restore purchase, Done.

### Keyboard help (`⌘+/`)
Backdrop + × close only (content is shortcut reference).

### License gate (implemented, not always mounted)
Email, password, Sign in & activate, signup links, Sign out when unlocked.

---

## 11. Keyboard-only (no dedicated button — annotate in Figma)

| Shortcut family | Examples | Category |
|---|---|---|
| Chrome visibility | Tab, F6–F8 hide panels / filmstrip / toolbar | Navigation |
| Clipping / info overlays | `J`, `I` | View / overlay |
| Fullscreen | Fullscreen toggle | View |
| Crop overlays | `O`, `Shift+O`, `H`, `L` | Crop |
| Library rates / flags | Number keys, flags, select-all | Catalog |

---

## Suggested Figma page structure

1. **Foundations** — color, type, spacing, slider / chevron / tab / swatch components  
2. **Chrome** — top nav, status bar, tool rail, filmstrip, report FAB  
3. **Develop — Edit** — full right panel (matches screenshot)  
4. **Develop — Crop / Mask / Presets**  
5. **Library + Import**  
6. **Education**  
7. **Modals** — Export, Settings, Supporter, Report, Keyboard help  
8. **Empty / disabled / placeholder** states (Remove panel, empty snapshots, disabled Develop tab)

---

## Notes for the intern

1. **Active language:** white text / underline for tabs; **blue fill** for selected file rows; **blue border** for selected filmstrip thumbs; **white border** for selected HSL swatch.  
2. **Density:** this is a tool UI, not a marketing page — keep controls compact.  
3. **Don’t invent** Remove-tool controls or Clarity/Optics sliders yet — mark as “coming soon” if you draw the slots.  
4. Code lives in `src/components/` (especially `lr/`). Screenshot path for reference: Develop → Edit tab with Light + HSL open.
