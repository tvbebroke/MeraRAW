# 14 — Feature Summary | Lightroom CC | 2019 Releases (Build Matrix)

**Coverage:** desktop **v2.0** (Oct 2018) and later 2.x; mobile **v4.0** — **[BUILD: 2018‑10 →]**

## Purpose
Changelog for the Oct 2018 v2.0 baseline and 2019 increments. Authoritative build tags for
the newest features in the document.

## Build matrix

| Build | Date | Introduced / Changed |
|---|---|---|
| **Desktop v2.0 / Mobile v4.0 / Web** | **Oct 2018** | **People View** (ML face grouping — file 15). **Auto-update**; single-instance constraint; settings migrate forward. |
| **2019 (v2.x)** | 2018‑10 → | **Improved search** — type-ahead suggestions; metadata-scoped filters (`camera:` etc.); filter chips in search box. **Local color** — Brush / Linear Gradient / Radial Gradient can apply a **color (hue)**, not just tonal edits. **Share any selection** as a web gallery (not only whole albums). |

## Local-adjustment color (detail)
- For Brush / Linear Gradient / Radial Gradient, a **Color** control is added at the bottom
  of the tool's options; sets a hue applied within the masked region (e.g. recoloring
  irises). This extends the local-adjustment model beyond tone into **hue**.

## IMPL
- Extend the local-adjustment mask object with a `color` (hue/sat) parameter:
  ```
  LocalMask { type: brush|linearGrad|radialGrad, geometry, adjustments:{...tone}, color?:{h,s} }
  ```
- Search upgrade = faceted query parser + suggestion service (see file 07).
- Share scope becomes `album | selection` (see file 08).
