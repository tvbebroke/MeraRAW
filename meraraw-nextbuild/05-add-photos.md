# 05 — Add Photos (Import Pipeline)

**Applies to:** Desktop v2.0 (import + review), Mobile v4.0 — **[BUILD: October 2018]**

## Purpose
Defines the ingest pipeline: source → review/select → copy → cloud upload.

## Flow (desktop)
1. **Source selection.** Add from (a) connected camera/card reader, or (b) hard-drive
   folder/files. Entry points: top-left **Add** (`+`) button or `File > Add Photos…`.
   - If a camera/card is connected, the `+` button opens a context menu listing the
     device; a **Browse** option falls back to the filesystem picker.
2. **Review-for-import screen.** Show previews of candidate images. User toggles each
   photo's inclusion via a circular selection overlay; supports select-all / range.
3. **Copy + upload.** On confirm, Lightroom **copies** imported photos into its managed
   store and **uploads full-resolution originals to the cloud**. After import the source
   (e.g. memory card) can be safely cleared.

## Behavioral specs
- Import is **copy**, never in-place reference (contrast with Classic's "Add" that can
  reference in place). Originals become cloud-backed.
- Supported on import: raw (DSLR/mirrorless), JPEG, TIFF, PNG, DNG; HEIC where the
  platform supports it `[BUILD: v1.5+ on Windows]`.
- On import, default profiles are auto-applied: **Adobe Color** (color) / **Adobe
  Monochrome** (B&W). (See file 06 → Profile.)

## IMPL
```
ImportJob {
  sources: [CameraSource | FolderSource]
  candidates: [{ path, selected: bool, previewThumb }]
  onConfirm(): copyToManagedStore() -> enqueueCloudUpload(fullRes)
}
```
- Build the **review screen** as a separate state from the library grid.
- Decouple `copyToManagedStore` (sync, fast) from `cloudUpload` (async, resumable).
- Apply the default profile at ingest so the first render isn't the flat raw.
- Generate a **Smart Preview** proxy at import for offline editing (see file 03).
