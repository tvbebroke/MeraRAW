# Meratech — Frontend UI Schema & Docs

A complete map of the current React/TypeScript UI: component tree, state, the IPC surface to the Rust core, data shapes, and the conventions that make it tick. Hand this to Claude (or any designer) as the brief for improving the interface.

> **The one rule that shapes everything:** the Rust core is the single source of truth for the edit. The UI never owns edit state — it dispatches **ops** and reconciles a **mirror** of the canonical EditDoc from `doc-updated` events. Every control, including the AI assistant, writes through the same guard-walled op path. Keep that invariant when redesigning.

---

## 1. Stack

- **React 18** + **TypeScript** + **Vite**
- **Zustand** for state (two small stores; no Redux)
- **Tauri 2** bridge: `@tauri-apps/api` `invoke` (commands) + `listen` (events)
- Plain CSS with CSS variables (no framework). Dark, neutral theme (must not bias color judgement).
- Custom URI protocols for binary pixels: `frame://` (viewport) and `thumb://` (grid).

No component library, no router, no CSS-in-JS. ~17 source files.

---

## 2. Layout (current)

```
┌─ topbar ────────────────────────────────────────────┐  34px
│  [▦ Library]  [◐ Develop]                            │
├──────────────────────────────────────────────────────┤
│                                                       │
│   mode === "library"           mode === "develop"     │  1fr
│   ┌─────────────────┐          ┌────┬────────┬─────┐  │
│   │ Library         │          │File│Viewport│Right│  │
│   │  toolbar+filters│          │220 │ +film- │ 280 │  │
│   │  virtualized    │          │ px │ strip  │ px  │  │
│   │  thumb grid     │          └────┴────────┴─────┘  │
│   └─────────────────┘                                 │
├──────────────────────────────────────────────────────┤
│  statusbar: engine · gpu · zoom · message            │  24px
└──────────────────────────────────────────────────────┘
```

Two top-level modes (`App.tsx` local `mode` state): **Library** (lighttable) and **Develop**.
The Develop view is a 3-column grid: `[220px file] [1fr viewport+filmstrip] [280px right rail]`.
The **right rail** stacks, top to bottom: Histogram · Info · Assistant · Masks · Develop sliders · Export & Presets. (This rail is the most crowded surface and the prime candidate for redesign — see §9.)

---

## 3. Component tree & responsibilities

```
App.tsx ............. mode switch, open-image flow, event wiring, status bar
├─ Library.tsx
│   ├─ <Library>     grid: import, filters (rating/flag/blur/dupe/text),
│   │                keyboard culling (0-5 rate, P pick, X reject, ⏎ develop)
│   └─ <Filmstrip>   horizontal strip shown under the Develop viewport
├─ viewport/Viewport.tsx
│                    canvas; draws frame:// bytes; wheel-zoom, drag-pan,
│                    dbl-click fit/1:1; WB-eyedropper + brush-paint pointer modes
├─ components/Histogram.tsx   RGB histogram canvas + clip % (settle-driven)
├─ components/AssistantPanel.tsx  chat: text input, Auto, Explain; tool-call ticker
├─ components/MasksPanel.tsx  create (subject/bg/sky/radial/linear/brush),
│                             select, overlay (👁), opacity/feather/invert, delete
├─ components/DevelopPanel.tsx  registry-driven sliders grouped in <details>;
│                               undo/redo; WB eyedropper toggle; history list
└─ components/ExportPanel.tsx  format/space/size, Export, save/apply presets
```

State + IPC live outside components:
```
state/uiStore.ts ... UI-only state (Zustand)
state/docStore.ts .. mirror of the canonical EditDoc (Zustand)
ipc/commands.ts .... 33 typed invoke() wrappers
ipc/events.ts ...... typed listen() helpers
ipc/types.ts ....... all shared TS types (mirror Rust serde shapes)
```

---

## 4. State stores

### `uiStore` (UI-only — never holds edit state)
| field | type | meaning |
|---|---|---|
| `engineReady` | bool | engine actor up |
| `gpuAdapter` | string\|null | e.g. "Apple M1 Max" |
| `statusMessage` | string | status-bar text |
| `lastOpenedPath` | string\|null | current image path |
| `imageOpen` | bool | an image is loaded |
| `imageDims` | {w,h}\|null | working-image pixel size |
| `decodeState` | "idle"\|"preview"\|"ready"\|"error" | decode lifecycle |
| `zoomLabel` | string | "fit" / "100%" / etc |
| `tool` | "pan"\|"wb"\|"brush" | active viewport pointer tool |
| `selectedMask` | string\|null | mask id whose scoped params the sliders edit |

### `docStore` (mirror of the canonical EditDoc)
| field | type | meaning |
|---|---|---|
| `doc` | EditDocMirror\|null | last reconciled doc |
| `docVersion` | number | bump on every change (re-render trigger) |
| `undoDepth` / `redoDepth` | number | stack depths (enable/disable buttons) |
| `lastLabel` | string\|null | last op label |
| `history` | string[] | op labels for the history panel |

`reconcile(delta)` is called on every `DocDelta` (from any op, undo, redo, restore, the assistant). **All edit mutations flow through here** — the UI is always a follower.

Helper: `docParam(doc, module, param)` → the value or `undefined` (= registry default).

---

## 5. IPC command surface (33 wrappers in `ipc/commands.ts`)

Grouped by concern. All are `async`, return typed results, throw typed `AppError` (`{kind, message}`).

**App / files:** `appInfo` · `pingEngine` · `pickFile` · `pickFolder` · `readFileMeta` · `listDir`
**Image lifecycle:** `openImage(path) → ImageMeta` · `requestFrame(view) → FrameInfo` · `getMetadata` · `closeImage`
**Edit ops (the core write path):** `applyOp(op) → DocDelta` · `setParam(path,value)` · `undo` · `redo` · `getDoc` · `getHistory` · `getRegistry → ParamSpec[]`
**Doc management:** `snapshot(name)` · `listSnapshots` · `restoreSnapshot(name)` · `virtualCopy` · `switchDoc(id)`
**Develop helpers:** `getStats → FrameStats` · `wbFromPoint(x,y) → DocDelta` · `setMaskOverlay(id\|null)`
**Catalog (P5):** `importFolder(path)` · `getGrid(query) → GridItem[]` · `setAssetMeta(ids,patch)` · `rebuildIndex`
**Export/presets (P7):** `export_image(settings)` · `save_preset(name,modules)` · `list_presets` · `apply_preset(name)` · `get_perf_stats`
**Assistant (P6):** `assistant_available() → bool` · `assistant_send(message,mode) → string`

`requestFrame` is how the viewport asks the engine to render a specific view; the engine renders (proxy-fast) and returns a `FrameInfo{version}`, then the canvas fetches `frame://localhost/current?v={version}`.

---

## 6. IPC events (`ipc/events.ts`, pushed by the engine)

| event | payload | UI reaction |
|---|---|---|
| `engine-ready` | {adapter, gpuReady} | enable UI |
| `preview-ready` | version | status "preview (decoding…)" |
| `image-ready` | version | full render arrived; redraw |
| `frame-ready` | version | redraw the canvas |
| `decode-error` | message | error state |
| `doc-updated` | DocDelta | **reconcile the mirror** |
| `mask-ready` | mask id | segmentation finished; redraw |
| `import-progress` | {done,total} | progress text |
| `import-done` | total | refresh grid |
| `catalog-changed` | — | refresh grid/filmstrip |
| `file-opened`/`folder-opened` | path | menu-driven open |
| `assistant-progress` | {kind,label} | tool-call ticker (kind: "tool"\|"text"\|"done") |

---

## 7. Key data shapes (`ipc/types.ts`)

```ts
ImageMeta   { cameraMake, cameraModel, lens, iso, shutter, aperture,
              focalMm, capturedAt, width, height, orientation,
              asShotWb:[r,g,b], estimatedCct }

ViewParams  { outW, outH, scale:number|null /*null=fit*/, centerX, centerY }
FrameInfo   { version, width, height }

ParamSpec   { path:"exposure.stops", ty:"f32"|"bool"|"enum"|"curve"|"color",
              min, max, default, enumValues?, ui:{label,step,scale,group} }

EditDocMirror { schema_version, doc_id, source_ref:{path},
                modules?: Record<module, Record<param, value>>,
                masks?: MaskMirror[], meta?: {...} }
MaskMirror  { id, kind, opacity, invert, feather, source:{type,...}, modules? }

DocDelta    { doc:EditDocMirror, label, undoDepth, redoDepth, newMaskId? }

Op          // discriminated union the UI dispatches:
  | {op:"set_param", path, value}
  | {op:"add_mask", kind, source}
  | {op:"remove_mask", id}
  | {op:"refine_mask", id, opacity?, feather?, invert?}
  | {op:"set_mask_source", id, source}
  | {op:"reset_module", module} | {op:"reset_all"}
  | {op:"apply_preset", preset:{modules}}

GridItem    { id, path, filename, width, height, rating, flag, label,
              hasEdits, capturedAt, cameraModel, blurScore, hasThumb }
GridQuery   { text?, ratingMin?, flag?, hasEdits?, blurryOnly?, dupesOnly?,
              sort?, offset?, limit? }
FrameStats  { bins, r[], g[], b[], luma[], clipHighPct, clipLowPct }
```

---

## 8. The registry-driven control pattern (important to preserve)

`DevelopPanel` does **not** hard-code sliders. On mount it calls `getRegistry()` → `ParamSpec[]`, groups by `ui.group`, and renders one `<SliderControl>` per `f32` spec. Each slider:
1. reads its effective value from the doc mirror (or registry default, or as-shot for WB temp),
2. on change dispatches `setParam(path, value)` → `DocDelta` → `reconcile`,
3. when a mask is selected, the path is rewritten to `mask.<id>.<param>` — the **same sliders edit mask-scoped params** (one code path, global and local).

**Consequence for redesign:** adding/removing a develop control is a Rust registry change, not a React change. New modules appear in the UI automatically. A redesign should keep generating controls from the registry rather than hand-placing them — but it's free to change grouping, ordering, widget type per `ui.scale`/`ui.step`, and layout.

Param groups currently present (from the registry): **Light** (exposure), **White Balance**, **Calibration**, **Detail** (noise + sharpen), **Color Grade** (3-way wheels as raw sliders today), **HSL** (8 bands × hue/sat/lum), **Tone** (contrast + parametric + curve points).

---

## 9. Known rough edges / redesign opportunities

Candid list — this is where a UI pass would add the most:

1. **Right rail is overloaded.** Histogram + Info + Assistant + Masks + ~45 Develop sliders + Export all stack in one 280px column. Needs real information architecture (tabs/accordions/panels, or move some to a left rail or a bottom bar).
2. **Color-grade wheels are raw sliders.** `color_grade.{zone}_{hue,sat,lum}` are 9 number sliders. They beg for actual **color wheels** (the registry exposes hue 0-360 + sat/lum, so a wheel widget can drive 3 params at once via `applyOp` batching... note: the UI currently sends one `setParam` per change; an `apply_preset`-style batch or multiple `applyOp`s would group them).
3. **HSL is 24 flat sliders.** Wants a band-picker (click a hue, edit its 3 values) or an on-image targeted-adjust ("click a color, drag").
4. **Tone curve has no curve editor.** `tone_curve.points` is a real Curve param (`ty:"curve"`) but there's no graphical curve widget yet — only the parametric sliders. A draggable curve canvas is the obvious win.
5. **No on-image interaction beyond pan/zoom/WB/brush.** Targeted adjustments (click subject → local mask), crop, straighten, before/after split, loupe are all absent.
6. **Masks panel is functional but plain.** No mask thumbnails, no on-canvas handles for radial/linear geometry (you can paint brush strokes, but radial/linear are created at fixed defaults and only refined via sliders).
7. **Assistant is a bare chat.** No streaming text, no inline "before/after" of what it changed, no one-click accept/revert of its batch, no suggestion chips.
8. **No keyboard map in Develop** (Library has culling keys; Develop has none).
9. **Status/feedback is a single text line.** No toasts, no progress affordance for export/import beyond text.
10. **Theming** is a small set of CSS variables in `styles.css` (`--bg`, `--bg-panel`, `--bg-elevated`, `--border`, `--text`, `--text-dim`, `--accent`, `--error`, `--ok`). A redesign can extend these; keep the viewport surround neutral so it doesn't bias color.

**Constraints to honor in any redesign**
- Keep the canonical-in-Rust / mirror-in-UI split — never store edit state in React.
- All edits go through `applyOp`/`setParam` (so undo, presets, and the assistant stay coherent). Batch related changes as multiple ops or a preset, not a bespoke path.
- Controls should still derive from the registry where possible.
- The viewport is a `<canvas>` fed by `frame://`; don't try to render the photo in DOM/CSS.
- Pixel-heavy work stays in Rust; the UI only sends view params and op intents.

---

## 10. Files to read when implementing

- `src/App.tsx` — composition, mode switch, event wiring
- `src/components/DevelopPanel.tsx` — the registry→sliders pattern (the model to follow)
- `src/state/docStore.ts` + `src/ipc/events.ts` — the reconcile loop
- `src/ipc/commands.ts` + `src/ipc/types.ts` — the full callable surface + shapes
- `src/viewport/Viewport.tsx` — canvas, view math, pointer tools
- `src/styles.css` — current theme tokens & layout
