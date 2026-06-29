# 13 — Migrate Apple Photos Library to Lightroom CC

**Applies to:** Desktop v2.0 — **macOS only** — **[BUILD: October 2018]**

## Purpose
Importer for an Apple Photos library into CC.

## Behavior
- macOS-only path that reads the **Apple Photos library** and imports images into CC,
  uploading originals to the cloud like any other import.
- Albums/structure are mapped where a CC equivalent exists; edits made in Apple Photos are
  generally **not** translated (originals come across; Photos-specific adjustments are
  lossy).

## IMPL
```
ApplePhotosMigration (macOS) {
  locateLibrary(.photoslibrary)     // package bundle
  enumerateMasters()                 // original files
  import -> upload(fullRes) -> createCloudAsset()
  mapAlbums()                        // best-effort
}
```
- Read the `.photoslibrary` package read-only; pull **masters/originals**, not rendered
  derivatives.
- Treat Apple edit data as **non-portable**; document the loss to the user.
- Platform-gate to macOS.
