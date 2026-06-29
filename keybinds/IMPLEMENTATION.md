# Keyboard shortcuts — implementation notes

This folder is the **source of truth** for MeraRAW’s Lightroom Classic–compatible shortcut layer.

| File | Purpose |
|---|---|
| `00-keyboard-shortcuts-project.md` | Architecture spec (scopes, conflicts, checklist) |
| `keyboard-shortcuts-catalog.md` | Human-readable catalog (287 bindings) |
| `keymap.json` | Machine-readable bindings loaded at runtime |

## Runtime code

Implementation lives under `src/keyboard/`:

- `loadKeymap.ts` — imports `keymap.json`, expands numeric ranges (`1–5`, `Ctrl+0–9`), skips click/drag actions
- `chord.ts` — normalizes OS-specific key strings and `KeyboardEvent`s
- `dispatcher.ts` — scope-first resolution + dispatch logging (dev)
- `commands.ts` — `binding.id → handler` registry (core handlers + stubs)
- `useKeyboardShortcuts.ts` — mount once from `App.tsx`

Help overlay: `src/components/KeyboardHelpOverlay.tsx` (`Cmd+/` / `Ctrl+/`).

## Editing shortcuts

1. Change `keymap.json` (or the catalog for documentation).
2. Add/update handler in `src/keyboard/commands.ts` keyed by binding `id`.
3. Rebuild — no per-component `keydown` handlers.

## Scope stack (most specific wins)

1. `tool:*` (brush, crop, spot when relevant)
2. `module:library` or `module:develop`
3. `global`

## Unimplemented features

Bindings whose features are not built yet show a status-bar message (“coming soon”) or a confirm dialog for destructive actions. Check the dev console for `[keybind] unmapped` / `[keybind] miss` traces.
