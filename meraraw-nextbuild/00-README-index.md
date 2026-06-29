# Lightroom CC — Technical Implementation Spec (chapter set)

> **Source document:** Adobe "Lightroom CC Help" PDF, 155 pp, **last updated 2018‑10‑15**.
> Author: Adobe Systems Incorporated.

## ⚠️ Product-identity correction (read first)

You described this as **Lightroom Classic**. The document is actually for
**Lightroom CC** — the *cloud-native* app (now branded simply "Lightroom"), **not**
Lightroom Classic. They are different products and the distinction drives almost every
architectural decision below:

| | **Lightroom CC** (this doc) | **Lightroom Classic** |
|---|---|---|
| Data model | Cloud-first; originals uploaded to Creative Cloud; **no user-visible catalog** | Local `.lrcat` catalog database |
| Storage | Cloud + local cache/Smart Previews | Local files on disk |
| Organizing unit | **Albums** | Collections + Folders |
| Sync | Native across desktop/mobile/web | Optional, via cloud-synced collections |
| Edit storage | Cloud-synced edit records | XMP sidecars / catalog / DNG |

If your implementation target is genuinely a *local-catalog* editor, treat the **edit
controls** chapters as fully reusable (the develop pipeline is shared between CC and
Classic), but treat the **storage/sync/album** chapters as CC-specific and substitute a
local catalog model.

## Build coverage in this document

- **Desktop:** Lightroom CC **v1.3 → v2.0** (v2.0 = October 2018). Key inflection builds:
  **v1.4 (June 2018)**, **v1.5 (August 2018)**, **v2.0 (October 2018)**, plus 2019 (v2.x).
- **Mobile:** iOS **v3.4** / Android **v3.6** (Aug 2018) → **v4.0** (Oct 2018).
- **Web:** lightroom.adobe.com (continuous).

Each file tags features with the build that introduced or changed them. Where the doc says
"latest version," it means the Oct 2018 desktop v2.0 baseline.

## File index (process in order)

| # | File | Implementation weight |
|---|------|----------------------|
| 01 | `01-whats-new.md` | Context only |
| 02 | `02-system-requirements.md` | Platform/runtime constraints |
| 03 | `03-common-questions.md` | Context only |
| 04 | `04-cc-photography-plans.md` | Context only |
| 05 | `05-add-photos.md` | **Core** — import pipeline |
| 06 | `06-edit-photos.md` | **Core (largest)** — develop engine |
| 07 | `07-organize-photos.md` | **Core** — metadata/albums/search |
| 08 | `08-save-or-share.md` | **Core** — export/share |
| 09 | `09-migrate-from-classic.md` | Data model / catalog import |
| 10 | `10-set-preferences.md` | Settings/storage management |
| 11 | `11-mobile-appletv-faq.md` | Platform parity notes |
| 12 | `12-feature-summary-2018.md` | **Build matrix** |
| 13 | `13-migrate-apple-photos.md` | Importer (macOS) |
| 14 | `14-feature-summary-2019.md` | **Build matrix** |
| 15 | `15-people-view.md` | Face detection/clustering |

## Conventions used in every file

- **`[BUILD: …]`** tags mark the version a behavior belongs to.
- **`IMPL:`** blocks are notes for the implementing agent (data model, state, deps) —
  these are inferred from documented behavior, not quoted from Adobe.
- Slider ranges/defaults are stated where the document specifies them; where it doesn't,
  they're marked `range: unspecified in source`.
