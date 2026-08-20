# Spec: Darktable-parity improvements for MeraRAW

**Author:** MeraRAW agent
**Date:** 2026-08-20
**Status:** Approved (user: research, plan, then build)
**Related:** `FOUNDATION-REBUILD.md`, `docs/pipeline-v0.1.4.md`, [darktable](https://github.com/darktable-org/darktable), [harness-skills operating model](https://github.com/harness/harness-skills)
**Implementation language:** Rust engine (`src-tauri/core`) + Tauri IPC + Svelte UI. No C, GTK, or OpenCL pixelpipe ports.

---

## 1. Title and Metadata

This spec is the contract for taking Darktable workflows that would improve MeraRAW and implementing them as native Rust/Tauri features. It is **not** a license to clone Darktable.

Phases and gates live in [`PHASES.md`](./PHASES.md). Code for a phase MUST NOT start until that phase’s dependency gate is green.

---

## 2. Context

Darktable is a scene-referred RAW developer with a **pixelpipe** (fixed IOP execution order), a **history stack** (chronological undo, stored in XMP + SQLite), a **lighttable** (collections / film rolls / culling), and ~100 IOPs (filmicrgb, sigmoid, agx, channelmixerrgb, toneequal, highlights, retouch, lens, …). Multiple pipes run in parallel: FULL, PREVIEW, PREVIEW2, THUMBNAIL, EXPORT.

MeraRAW already has the analogous spine:

| Darktable | MeraRAW today |
|---|---|
| Pixelpipe IOP order | Fixed `MODULE_ORDER` in `src-tauri/core/src/registry.rs` (contract A5: order is **not** in the doc) |
| History stack | Engine history + named snapshots (`get_history`, `snapshot`, `restore_snapshot`) |
| XMP + library DB | Sidecar `EditDoc` + SQLite catalog |
| Lighttable film rolls | Folder pins + `get_grid` / `sql_under_folder` |
| Copy/paste history | `copyGrade` / `pasteGrade` (editor + shortcuts only) |
| Virtual copies | In-session `virtual_copy` / `switch_doc` — **no UI, not catalog-persisted** |
| Module on/off | Only `lut.enabled` (plus retouch spots) |
| Collections filters | **Engine exists** (`GridQuery`: rating, flag, camera, text, hasEdits, blurry, dupes). Library UI filters filename client-side only |
| Parametric masks | Geometric masks exist; no luminance/hue parametric blend |
| Filmic / sigmoid | LUT / AgX look cubes — not first-class scene-referred tone modules |
| Soft proof | Absent |
| Highlight reconstruction | Absent as a dedicated IOP |

Porting Darktable’s C IOPs, GTK UI, OpenCL pixelpipe, or XMP compatibility would destroy MeraRAW’s GPU graph, sidecar contract, and photo+video workspace. The improvements that pay off are **workflows the catalog/engine already almost support**, then **gated color-science modules with tests**.

Harness-skills (Harness.io CI/CD) is not a photo editor. We take its **operating model**, not its YAML:

1. Establish scope first.
2. Verify dependencies before generating dependents.
3. Discover schema before writing payloads (`GridQuery`, `EditDoc`, IPC allowlist).
4. Phase-gated work with explicit exit criteria.

---

## 3. Functional Requirements

### Crosswalk — what we will take

**FR-1.** The Library (and the editor filmstrip fed by the same grid) MUST filter via catalog `get_grid`, not by hiding rows in the browser after loading an unfiltered folder.

**FR-2.** Users MUST be able to pin Darktable-style collection filters: rating ≥ N, pick/reject/unflagged, edited / not edited, text search (filename, camera, keywords).

**FR-3.** Users SHOULD be able to filter blurry-only and duplicate-only using the existing `blurryOnly` / `dupesOnly` query fields.

**FR-4.** Changing a collection filter MUST reload the current folder grid (including when the folder is a nested path already selected in the tree).

**FR-5.** Copy grade and paste grade MUST be available from the Library thumbnail context menu and the editor filmstrip context menu, targeting the clicked item (open it in the engine if it is not already the active document). Crop, masks, retouch, calibration, and detail MUST remain destination-owned (existing `GRADE_MODULES` contract).

**FR-6.** Search text MUST be sent as `GridQuery.text` so camera model and keywords match, not only filename.

### Later phases (specified now, not built in Phase 1)

**FR-7.** Each edit-panel module MUST support enable/disable (identity skip in the graph) without changing `MODULE_ORDER`.

**FR-8.** Virtual copies MUST be catalog-persisted (sidecar clone + grid row), switchable from Library and Versions UI. In-memory `virtual_copy` is insufficient.

**FR-9.** Styles MUST apply a copied grade to many selected catalog assets without opening each in the UI (batch sidecar write + `has_edits`).

**FR-10.** Parametric masks (luminance / chroma / hue range) MUST blend per-module like Darktable’s blend ops, on the existing mask stack.

**FR-11.** Highlight reconstruction MUST be a dedicated engine module with fixture tests (clipped channel recovery), not a UI slider on exposure.

**FR-12.** A scene-referred tone mapper (filmic- or sigmoid-class) MUST be an explicit module with golden-image / numeric tests before it ships in the panel.

**FR-13.** Soft-proof and gamut-check MUST render into the viewport from the export color space, without mutating the sidecar.

**FR-14.** Culling compare (2–N images) MAY use a second preview surface; it MUST NOT fork a second pixelpipe architecture.

### Product invariants (MUST NOT)

**FR-15.** Module execution order MUST remain the registry vector. Users MUST NOT reorder IOPs.

**FR-16.** The engine MUST remain Tauri-free in `src-tauri/core`. New IPC is a thin command over `EngineMsg` / catalog.

**FR-17.** Photo and video MUST stay one engine, two workspaces. Collection filters MUST apply to both; workspace still hides the other media kind.

---

## 4. Non-Functional Requirements

**NFR-1.** Filter query construction MUST be a pure function with unit tests (no Tauri). Catalog SQL for rating/flag/has_edits already has Rust tests; Phase 1 MUST NOT regress them.

**NFR-2.** Typing in search MUST debounce ≥ 200ms before `get_grid` so each keystroke does not hit SQLite.

**NFR-3.** `get_grid` for a folder of ≤ 2000 assets MUST remain the existing IPC; Phase 1 MUST NOT add a new command if `GridQuery` already expresses the filter.

**NFR-4.** Collection filter UI MUST be keyboard-reachable (buttons, not click-only divs) and MUST expose `aria-pressed` / `aria-label` on rating and flag chips.

**NFR-5.** Color-science phases (FR-11, FR-12) MUST NOT merge without `cargo test` fixtures for the new module. UI-only tone knobs are forbidden.

**NFR-6.** New IPC names MUST be added to `src/ipc/contractAllowlist.ts`, `src-tauri/permissions/default.toml`, and `build.rs` in the same change.

---

## 5. Acceptance Criteria

**AC-1.** (FR-1, FR-2, FR-4) Given a folder whose catalog contains 5-star and 0-star photos, when the user pins rating ≥ 4, then `get_grid` is invoked with `ratingMin: 4` and unrated thumbnails are not in the grid.

**AC-2.** (FR-2) Given photos flagged pick and reject, when the user pins Pick, then only `flag = "pick"` rows return. Clicking Pick again restores `flag: any`.

**AC-3.** (FR-2, FR-6) Given a photo whose camera_model is `ILCE-7M4` and filename does not contain that string, when the user searches `7M4`, then that photo is in the grid.

**AC-4.** (FR-5) Given photo A open with a copied grade, when the user right-clicks photo B in Library and chooses Paste grade, then B becomes the active document and receives `GRADE_MODULES` (not crop/masks).

**AC-5.** (FR-17) Given mixed stills and clips in one folder, when Photo workspace is active and filters are set, then clips still do not appear in the Photo grid (`libraryItems` workspace filter).

**AC-6.** (NFR-1) Given `gridQueryFromState`, unit tests cover default (no extra keys), rating, flag, hasEdits, text trim, blurry, dupes, and folder.

**AC-7.** (FR-4) Given an active filter, when catalog import upserts the open folder, then `initBrowseBridge` reloads with the same filters (not an unfiltered grid).

---

## 6. Edge Cases

**EC-1.** Empty filter result in a non-empty folder: show “No photos match these filters” and a Clear control, distinct from “No folder open”.

**EC-2.** Folder has only the other workspace’s media: keep the existing “Switch editors” empty state (workspace filter after catalog filter).

**EC-3.** Import polling (`importAndBrowse`) MUST query `limit: 1` **without** user filters, or an active “edited only” filter would make a fresh import look empty forever.

**EC-4.** Paste grade with empty clipboard: existing status message; do not open a random file.

**EC-5.** Copy/paste on an inaccessible (`accessible: false`) thumbnail: surface the engine/open error; do not crash the grid.

**EC-6.** Rating chip click on the current minimum: clear ratingMin to 0 (toggle), matching Darktable’s “any rating” default.

---

## 7. API Contracts

Phase 1 uses the existing command. No new native IPC.

```ts
interface GridQuery {
  text?: string;
  ratingMin?: number;      // >= 1
  flag?: string;           // "pick" | "reject" | "none" | omit for any
  camera?: string;
  hasEdits?: boolean;
  folder?: string;
  blurryOnly?: boolean;
  dupesOnly?: boolean;
  sort?: string;           // "captured" | "imported" | "rating"
  limit?: number;
  offset?: number;
}

function getGrid(query: GridQuery): Promise<GridItem[]>;
```

Frontend collection state (not persisted in Phase 1):

```ts
type LibraryFlag = "any" | "pick" | "reject" | "none";

interface LibraryFilters {
  ratingMin: number;       // 0 = any
  flag: LibraryFlag;
  hasEdits: boolean | null;
  blurryOnly: boolean;
  dupesOnly: boolean;
  text: string;
}

function gridQueryFromState(
  folder: string | null,
  filters: LibraryFilters,
  opts?: { limit?: number; sort?: string },
): GridQuery;
```

Copy/paste onto a path:

```ts
function copyGradeFrom(path: string): Promise<boolean>;
function pasteGradeOnto(path: string): Promise<boolean>;
```

---

## 8. Data Models

| Entity | Store | Notes |
|---|---|---|
| `assets.rating` | SQLite 0–5 | Already written by `set_asset_meta` |
| `assets.flag` | `none` / `pick` / `reject` | Already written by cull bar |
| `assets.has_edits` | bool | Set when sidecar is non-identity |
| `assets.camera_model` | EXIF | Indexed via `get_grid` LIKE |
| `keywords` | join table | `GridQuery.text` already searches it |
| `LibraryFilters` | nanostore, session | Phase 1 does not persist; Phase 2 MAY persist per folder |
| `GradeClip` | memory atom | Existing; crop/mask not included |

---

## 9. Out of Scope

| Exclusion | Reason |
|---|---|
| Porting Darktable C IOPs / OpenCL | Wrong runtime; MeraRAW is a GPU graph in Rust |
| GTK / CSS darktable UI | MeraRAW is Svelte + Tauri |
| XMP read/write compatibility with Darktable | Sidecar schema is `EditDoc`; dual format is a later product decision |
| User-reorderable module order | Contract A5 |
| In-memory virtual copies as a shipped feature | Lost on close; wait for FR-8 |
| Soft proof, filmic, highlights in Phase 1 | Depend on P0 harness + numeric tests |
| Albums UI | Catalog `album_id` exists; not a Darktable-parity P1 item |
| Multi-select batch paste | Engine paste is one open doc; FR-9 is Phase 2 |
| Rewriting the color engine | `FOUNDATION-REBUILD.md` forbids it |

---

## Phase 1 build (this change)

Implements FR-1…FR-6, AC-1…AC-7, EC-1…EC-6, NFR-1…NFR-4.

Does **not** implement FR-7…FR-14.
