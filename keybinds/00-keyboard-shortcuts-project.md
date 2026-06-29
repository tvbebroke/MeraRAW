# Project: Keyboard Shortcut System — meraraw

**Goal:** Implement a complete, Lightroom-Classic-compatible keyboard shortcut layer for
the meraraw raw editor. This document is the spec for the implementing agent (Cursor),
the debugger reference, and the user-facing guide.

**Source of truth:** Adobe Lightroom Classic shortcut reference (helpx, last updated
2025‑10‑30), normalized into machine-readable form here. Key→action mappings are factual;
all action descriptions in these files are original.

## Deliverables in this folder
| File | Audience | Purpose |
|---|---|---|
| `00-keyboard-shortcuts-project.md` (this) | Cursor / debugger | Architecture, scope model, conflicts, checklist |
| `keyboard-shortcuts-catalog.md` | Users / debugger | Human-readable directory of all 287 bindings |
| `keymap.json` | Cursor | Machine-readable bindings to load at runtime |

**Counts:** 287 bindings — **core 117**, optional 116, out_of_scope 54.

---

## 1. Implementation priority
Implement by `relevance` field, in order:
1. **core** — `global`, `module:library`, `module:develop`. This is the whole raw-editor
   surface: browse/cull, rate/flag, view modes, all Develop tools/sliders, copy-paste
   settings, masking, crop, undo/redo, import/export.
2. **optional** — collections, keywords/metadata painting, secondary window, snapshots.
3. **out_of_scope** — Book/Slideshow/Print/Web/Map. These are Lightroom *output modules*;
   skip unless meraraw grows them. Retained in the catalog for completeness.

A raw editor reaches feature-parity-on-shortcuts with just the **core** set.

---

## 2. Architecture: a scoped keymap dispatcher

Do **not** hardcode `keydown` handlers per component. Build one central dispatcher.

```
KeyboardManager
 ├─ load(keymap.json) -> normalized Binding[]
 ├─ activeScopes: ordered stack, most-specific first
 │     e.g. ["module:develop", "tool:crop", "global"]
 ├─ onKeyDown(e):
 │     chord = normalize(e)                  // see §3
 │     binding = resolve(chord, activeScopes) // see §4
 │     if binding: e.preventDefault(); dispatch(binding.action)
 └─ help(): render overlay filtered to activeScopes  // Ctrl//
```

### Binding shape (from `keymap.json`)
```json
{
  "id": "develop-module-tools:crop-tool",
  "action": "Crop tool",
  "keys": { "windows": "R", "macos": "R" },
  "scope": "global",
  "relevance": "core",
  "description": "...",
  "note": ""
}
```

**IMPL:** map each `action` string to a concrete command in your command registry.
Recommend a `commandId` indirection: `binding.action → commandId → handler()`. Keep the
keymap declarative so users can later remap without touching handlers.

---

## 3. Modifier normalization (cross-platform)

Parse the `keys.windows` / `keys.macos` strings into a normalized chord:

| Token in data | Windows | macOS |
|---|---|---|
| `Ctrl` | Control | **Command** (the data already splits where they differ — trust the per-OS string) |
| `Cmd` | — | Command |
| `Alt` / `Option` | Alt | Option |
| `Shift` | Shift | Shift |

- **Pick the field for the current OS** (`keys.macos` on mac, else `keys.windows`).
- Some entries are **click-modifiers or drag actions** (e.g. `Alt-click panel`,
  `Alt-drag`, `Drag pin right/left`), not keychords. The `note` flags these
  ("Click-modifier"/"Click action"/"Drag action"). Route them through pointer handlers,
  not the key dispatcher.
- Ranges like `1–5`, `Ctrl+0–9`, `Alt+1–9` expand to one binding per number at load time.
- `n/a` means no binding on that OS — skip.

---

## 4. Scope resolution (the important part)

Lightroom heavily **overloads keys by context**. The same key does different things
depending on the active module and tool. Resolution is **most-specific-scope-wins**:

```
resolve(chord, scopes):
  for scope in scopes (specific → general):
     if (chord, scope) in index: return it
  return null
```

### 4.1 Documented context collisions to honor
These are real one-key/many-action cases from the source. Your scope stack must
disambiguate them:

| Key | Resolves to… | by scope |
|---|---|---|
| **O** | crop-grid overlay / mask overlay | `module:develop` |
|  | People View | `module:library` |
|  | location-preset overlay | `module:map` |
| **X** | toggle crop orientation | `module:develop` |
|  | flag as reject | `module:library` |
| **S** | expand/collapse soft proofing | `module:develop` |
|  | toggle stack | `module:library` |
| **J** | show clipping | `module:develop` |
|  | cycle grid views | `module:library` |
| **H** | show/hide local pin (and "never show overlay") | `module:develop` |
| **Ctrl+N** | new snapshot | `module:develop` |
|  | new collection | `module:library` |
| **Ctrl+R** | show in Finder/Explorer | `module:library`/`module:develop` |
|  | reload/double-page/rulers | book/web/print |
| **Ctrl+M** | pano merge | `global` |
|  | update metadata captions / margins | book/print |
| **] / [** | brush size | `tool:brush` (develop) |
|  | increase/decrease rating | `module:library` |

### 4.2 Tool-overloaded **Shift+T** (single binding, tool-dependent)
`Shift+T` is **one chord** whose meaning depends on the *active Develop tool*:
- no tool → **Guided Upright**
- Spot Removal active → **toggle Clone/Heal**
- Graduated/Radial mask active → **toggle Mask Edit/Brush**

**IMPL:** push a `tool:*` scope when a tool is engaged so `Shift+T` (and the brush
`] [`, feather `Shift+] [`, eraser `Alt-drag`) resolve against the tool first.

### 4.3 Modifier-key OS swap to watch
On macOS, a few Develop merge ops use **Control**, not Command (`Ctrl+H` HDR,
`Ctrl+M` pano, Enhance `Ctrl+Alt+I`). The per-OS strings in the data already encode this —
don't "fix" them to Command.

---

## 5. Destructive-action safety

Bindings whose `note` contains **DESTRUCTIVE** must route through a confirm/guard:
- Delete photo(s), Delete & move to Trash, Delete rejected (batch), Remove from catalog.

**IMPL:** wrap these in a `confirmDestructive()` gate; never let a raw keypress hard-delete
files. Provide an undo path where the OS allows (trash, not hard-delete).

---

## 6. Help overlay (`Ctrl+/` / `Cmd+/`)
Lightroom shows a context cheat-sheet. Reuse the same data:
- On `Ctrl+/`, render bindings filtered to the current `activeScopes`, grouped by section.
- Dismiss on click. This is why `description` lives in the keymap — one dataset feeds the
  dispatcher **and** the overlay **and** the docs.

---

## 7. Debugger context
- Every binding has a stable `id` — log `"<id> fired in scope <resolved-scope>"` on
  dispatch for traceable input handling.
- When a keypress does nothing, log the **normalized chord + active scope stack** so you
  can see whether it was a scope miss vs an unmapped chord.
- Conflicts in §4 are the usual suspects for "wrong action fired" bugs — check which scope
  won resolution.

---

## 8. Implementation checklist
- [ ] Load `keymap.json`; expand numeric ranges; split key-chords vs click/drag actions.
- [ ] Build the normalized-chord index keyed by `(chord, scope)`.
- [ ] Implement the scope stack with most-specific-first resolution (§4).
- [ ] Push/pop `module:*` and `tool:*` scopes on navigation/tool changes.
- [ ] Wire `action → commandId → handler`; start with `relevance: core`.
- [ ] Gate every `DESTRUCTIVE` action behind confirmation.
- [ ] Implement the `Ctrl+/` help overlay from the same dataset.
- [ ] Add dispatch logging (id + resolved scope) for the debugger.
- [ ] (Later) expose the keymap for user remapping.

---

## 9. Caveats
- This mirrors Lightroom Classic **as of Oct 2025**; Adobe revises bindings between
  releases. Treat `keymap.json` as your editable source, not a frozen contract.
- Output-module bindings (54) are included for completeness but most collide harmlessly
  with core ones precisely because they live in scopes meraraw won't implement.
- Cloning Lightroom's *exact* shortcut scheme is a UX-compatibility choice; the mappings
  themselves are functional, but your action implementations should be your own.
