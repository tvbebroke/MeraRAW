# Changelog

All notable changes to MeraRAW are documented here.

## [0.1.7] — 2026-08-18

Edit-panel rebuild: one scrollable accordion instead of a wall of open sections, plus shell polish. Tag: `v0.1.7`.

### Highlights

- **Edit rail** — ten sections (Light → Color → Curve → Detail → Grading → Crop → Mask → Retouch → Camera → Presets). First paint opens Light and Color; histogram dock is open. Numeric controls share one ParamRow layout.
- **Cuts** — Optics, Effects, and AI Adjust are off the rail. Unwired Masking / split-toning wheels / dead prompt UI are gone. Camera nests Profile, Demosaic, LUT, and Calibration.
- **Shell** — command palette (⌘K), versions popover, jobs pill, AI agent tab. Collapsed left sidebar shows a chevron on the canvas to reopen it (`L` still toggles).
- **License check on boot** — the Svelte shell now calls `license_check_local`.

---

## [0.1.6] — 2026-07-21

Small polish release on top of the 0.1.5 cross-platform shell. Tag: `v0.1.6`.

### Highlights

- **Opt-in telemetry** — PostHog product analytics, frontend-only, off until the user consents. Prop whitelist so filenames/paths never leave the machine; no autocapture or session replay.
- **Window sizing** — rejects runaway restored geometry (oversized / below-minimum) and opens maximized when there is no usable saved size.
- **Single frameless title bar** — native macOS decorations stay off so only the Svelte traffic lights show (fixes “seeing double” when an old window-state file re-enabled decorations).
- **Denoise model-dir fix** — `ModelRegistry::with_dir()` grants unpinned-model permission so a caller-supplied models directory is marked ready.
- **Download server** — `get-download-url` probes spaced and dotted R2 keys and returns a clear JSON 404 instead of raw S3 `NoSuchKey` XML.

### Known limitations (carried)

- Svelte shell still does not call `license_check_local` (in-app license gate lost in the React → Svelte migration). Download gating on meratech.co / R2 is separate.

---

## [0.1.5] — 2026-07-19

Beta release focused on a full UI shell overhaul, cross-platform installers, Windows reliability, and demosaic engine clarity. Tag: `v0.1.5` (`bb70495` on `svelte-ui`).

### Highlights

- **UI overhaul** — React shell replaced by a Svelte 5 editor (intern-built layout, nanostores, spa-router).
- **Classic Look** — Settings → General → “Classic Look” restores denser Lightroom-style chrome alongside the Modern shell (not a separate app; toggle anytime).
- **Windows + Linux installers** — CI builds ship beside the notarized macOS DMG.
- **Merawler demosaic everywhere** — In-process algorithms (RCD default, AMaZE, LMMSE, IGV, etc.) work on Mac, Windows, and Linux without external tools.
- **Windows RAW workflow fixes** — Folder import, filmstrip, and preview transport now work with `.ARW` and other RAWs on Windows.

---

### UI

- Replaced the React application shell with **Svelte 5** (`feat(ui): replace React shell with intern Svelte UI`).
- Added **Classic Look** mode next to Modern (`feat(ui): add Classic look alongside Modern shell`) — same engine, alternate chrome density/theme.
- Chrome themes, window clamp, and general polish from the 0.1.5 UI track.
- Removed the psychedelic background video.
- Collaboration docs / `sync-ui.sh` for front-end hand-off (intern workflow).

### Platform — Windows

- **Preview transport** — WebView2 cannot use `frame://` / `thumb://`; URLs now use `http(s)://frame.localhost` / `thumb.localhost` so develop previews and filmstrip thumbs load.
- **Folder import empty grid** — Catalog queries assumed Unix `path LIKE '…/%'`. Windows stores `C:\…\file.ARW`. Queries now match `/` and `\` prefixes, and the `folder` column.
- **`\\?\` path mismatch** — After `canonicalize()`, Windows paths become `\\?\C:\…` while the picker returns `C:\…`. Paths are simplified before storage/query; legacy verbatim rows still match.
- **Import race** — UI used to call `getGrid` before import workers finished (and ignored `import-done` / `catalog-changed`). Import now waits/polls and refreshes live as thumbs land.
- **Menu Import** — File → Import / Open Folder now run full import-and-browse (previously only opened the picker).
- **App data dir** — Catalog uses `%LOCALAPPDATA%\MeraRAW` instead of a macOS `Library/Application Support` path.

### Platform — macOS & Linux

- macOS: signed + notarized DMG workflow (local Developer ID + notarize scripts).
- Linux: AppImage, `.deb`, and `.rpm` via GitHub Actions.
- Vendored **MERAWLER** / **ZERAWLER** under `vendor/` so Win/Linux CI does not need Desktop sibling checkouts.

### Demosaic & engines

- **Merawler (in-process)** — Always available on every OS: Rawler, Bilinear, Malvar, **RCD** (default), LMMSE, AMaZE, IGV, DDFAPD.
- **Zerawler (sidecar)** — RawTherapee (`rt-rcd` / `rt-lmmse` / `rt-amaze`) and LibRaw DHT remain optional. They appear in the UI only when `rawtherapee-cli` / `dcraw_emu` are found (app `workers/` folder, PATH, or common install locations). Not bundled in the installer (GPL/LGPL binaries).
- Demosaic picker filters by `availableDemosaic`; engine rejects unavailable sidecar choices instead of silent fallback.
- Improved worker discovery on Windows/Linux (Program Files, LOCALAPPDATA, `/usr/bin`, etc.).

### Library / import

- Folder scan includes RAW **and** common rendered formats (JPEG/PNG/TIFF/…), not RAW-only.
- Import progress streams into the grid via `catalog-changed`.
- Path denylist / validation hardening (canonicalization gap closed for sensitive system paths).

### Security & licensing plumbing

- Path-denylist canonicalization gap closed.
- License `iss` / `aud` validation gating tightened on the Rust side.
- **Known limitation:** the Svelte shell does not yet call `license_check_local` (license gate was lost in the React → Svelte migration). Download gating on meratech.co / R2 is separate from in-app activation.

### Crop, denoise, other engine work (0.1.5 line)

- Crop tool completion and related hardening.
- Classical denoise track + AI denoise end-to-end wiring.
- Dual UI shell prep that led into Classic Look.

### Installers (GitHub `v0.1.5`)

| Platform | Artifact |
|----------|----------|
| macOS | `MeraRAW.Beta.0.1.5.dmg` |
| Windows | `MeraRAW.Win.0.1.5.exe` |
| Linux | `MeraRAW.Linux.0.1.5.AppImage`, `.deb`, `.rpm` |

> **Note for publishers:** Rebuild and re-notarize the macOS DMG from current `svelte-ui` before promoting to R2 if the published DMG predates the catalog/demosaic UI fixes. Windows/Linux CI artifacts on the tag already include those fixes.

---

### Upgrade notes

- Overwriting tag `v0.1.5` means the same version string can contain different builds over time; prefer fresh download after each publish.
- Windows: SmartScreen may warn (unsigned beta). Use More info → Run anyway.
- Classic Look: Settings → General after first launch.

---

## Earlier

See git tags `v0.1.4`, `v0.1.2`, `v0.1.1`, `v0.1.0` and commit history for prior beta notes.
