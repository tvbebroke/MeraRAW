# 11 — Lightroom CC for Mobile & Apple TV | FAQ

**Applies to:** Mobile iOS v3.4 / Android v3.6 → v4.0; Apple TV — **[BUILD: Aug–Oct 2018]**

## Purpose
Cross-platform parity + capture/sync notes. Useful for defining what a non-desktop client
must support.

## Key facts
- **Capture:** in-app camera; raw (DNG) capture on supported devices. iOS capture assumes
  a **≥12 MP** camera, **iOS 10.0+**.
- **Sync:** edits/albums sync through the cloud; mobile pulls Smart Previews, edits, then
  syncs back. Originals are cloud-backed (mobile does not depend on desktop originals).
- **Cellular control:** "Use Cellular Data" toggle gates sync/upload over cellular.
- **Apple TV:** read-only viewing client for synced albums (no editing).
- **Mobile-specific edit additions** tracked in feature summaries (e.g. depth-map support,
  lens-profile control on Android) `[BUILD: iOS v3.4 / Android v3.6, Aug 2018]`.

## IMPL
- Define a **client capability matrix**: `{ capture, edit, export, viewOnly }` per platform
  (Apple TV = viewOnly).
- Sync layer is platform-agnostic; clients differ only in capability flags + UI.
- Respect a **metered-network** flag before any upload/download.
