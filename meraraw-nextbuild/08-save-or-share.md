# 08 — Save or Share Your Photos

**Applies to:** Desktop v2.0 / Mobile v4.0 / Web — **[BUILD: October 2018]**

## Purpose
Output paths: local export ("Save to…"), web-gallery sharing, and Adobe Portfolio publish.

## 8.1 Save / export locally
- Export selected photos to disk. Documented output formats across the ecosystem: **JPEG,
  PNG, TIFF, DNG, original**. Mobile supports JPEG/PNG/DNG.
- Export controls (where exposed): dimension/size preset (Small / Large / Full /
  custom pixels), JPEG quality, include-metadata toggle, watermark, output sharpening.
  `range: several of these are surfaced contextually; treat as an export-settings object.`

## 8.2 Share as web gallery `[BUILD: 2019]`
- Share an **album** OR **any arbitrary selection** of photos as a hosted web gallery.
- Gallery options include **Show metadata** (allow viewers to see EXIF/IPTC) and link
  sharing.

## 8.3 Adobe Portfolio
- Publish selected photos/albums to **Adobe Portfolio** as an online project/site.
- Add to / remove from a public gallery via right-click or drag onto the Portfolio target
  under the Portfolio node. Removing from a public gallery does **not** delete from
  All Photos.

## IMPL
```
ExportSettings {
  format: jpeg|png|tiff|dng|original
  size: { mode: small|large|full|custom, longEdgePx? }
  quality?: 0..100            // jpeg
  includeMetadata: bool
  watermark?: { text|image, opacity, position }
  outputSharpen?: screen|matte|glossy
}
ShareGallery { scope: album|selection, showMetadata:bool, publicUrl }
```
- Export and Share are **distinct pipelines**: export writes files; share creates a hosted
  remote resource with its own permission flags.
- Portfolio is an external publish target — model as a connector, not core storage.
