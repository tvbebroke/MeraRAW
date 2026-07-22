# Phase 11 — Import, Metadata & Interop

**Goal:** Read and preserve EXIF/IPTC/XMP metadata, handle ICC profiles, and optionally import from other editors' formats (PSD/XCF/Aseprite). Lens metadata here also feeds optional lens correction.

**Dependencies & licenses**
| Dep | Role | License | Verdict |
|---|---|---|---|
| `kamadak-exif` (recommended) | EXIF read/parse | MIT/Apache | 🟢 embed freely (verify) |
| `sftsrv/exiflib` | alt EXIF parser | check repo | ⚠️ verify before embed |
| `WillKirkmanM/parse-psd` | Photoshop PSD/PSB import | check repo | ⚠️ verify before embed |
| `WillKirkmanM/parse-xcf` | GIMP XCF import | check repo | ⚠️ verify before embed |
| `EstebanAdso/colorpeek` | ICC profile inspection **reference** | check repo | study / verify |
| `lensfun` (optional) | lens correction DB | **code LGPL-3.0, data CC-BY-SA** | 🟡/⚠️ conditional |

---

## Architecture

Metadata rides alongside the non-destructive Document. On import, extract it; on export (Phase 05), re-attach per policy.

```rust
// crates/metadata/src/lib.rs
pub struct DocMetadata {
    pub exif: ExifData, pub iptc: Option<IptcData>, pub xmp: Option<XmpData>,
    pub icc: Option<IccProfile>, pub lens: Option<LensInfo>,
}
```

---

## Sub-phases

### 11.1 — EXIF read
- Parse EXIF (camera, lens, ISO, shutter, aperture, orientation, GPS, timestamps) via `kamadak-exif`.
- Apply orientation on load; surface shooting data in the UI.

### 11.2 — ICC handling
- Read embedded input profiles; feed Phase 04 color management. Use `colorpeek` as a reference for reading profiles from PSD/TIFF/PNG.

### 11.3 — Preserve/strip on export
- Implement `MetadataPolicy` (Phase 05): preserve EXIF/IPTC/XMP, optionally strip GPS/personal fields, write copyright/software tags.

### 11.4 — Interop import (optional)
- PSD/XCF/Aseprite import via the parser crates (flatten or import layers). **Verify each parser's license before embedding**; several small repos may have no license (= all rights reserved).

### 11.5 — Lens correction data (optional)
- If adding auto lens correction (distortion/vignetting/CA), `lensfun` is the DB. Note: **code is LGPL-3.0 and the correction data is CC-BY-SA** — the share-alike data terms are separate from code and need their own check.

---

## Evaluate against your editor / RapidRAW
- Do you preserve EXIF/ICC on export, or drop it? Dropping is a common quiet bug.
- Auto lens correction present? RapidRAW uses lensfun; weigh the LGPL/CC-BY-SA obligations vs building your own profiles.
- Import from PSD/other editors — a migration/onboarding advantage if your users come from Photoshop.

## Testing & acceptance criteria
- [ ] EXIF (incl. orientation) reads correctly; orientation auto-applied on load.
- [ ] Embedded ICC read and honored by the pipeline; wide-gamut inputs render correctly.
- [ ] Export preserves metadata per policy; GPS-strip actually removes GPS.
- [ ] PSD import (if built) opens a test file with correct dimensions/layers.
- [ ] License check recorded for every parser + lensfun data before shipping.

## Risks / gotchas
- **No-license parser repos = all rights reserved.** Do not embed without a license; reimplement or find a permissive alternative.
- **lensfun CC-BY-SA data** has share-alike implications distinct from the code — get this checked.
- Orientation handling bugs (rotate twice / not at all) are common — test all 8 EXIF orientations.
