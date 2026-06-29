# 12 — Feature Summary | Lightroom CC | 2018 Releases (Build Matrix)

**Coverage:** desktop v1.3 → v1.5; mobile iOS v3.x / Android v3.x — **[BUILD: 2017‑10 → 2018‑08]**

## Purpose
Changelog chapter. Use as the **authoritative build-tag table** for features introduced
before the Oct 2018 v2.0 baseline.

## Desktop build matrix

| Build | Date | Introduced / Changed |
|---|---|---|
| **v1.4** | **Jun 2018** | Preset **&** profile **sync** across CC desktop+mobile (incl. custom/third-party); **does not** sync to Classic. Tone Curve: **removed** Medium/Strong Contrast presets and in-panel save/apply (curves now saved as presets). Profile **Import** (XMP). |
| **v1.5** | **Aug 2018** | **Store album locally** for offline (replaces per-photo offline, which was **removed**). **Album membership** view in Info panel. **HEIC** support on **Windows**. Square Grid shows file extension. New camera/lens support. Bug fixes (Presets-tab crash, filtering inside stacks, Show Original crop revert). |

## Mobile build matrix

| Build | Date | Notes |
|---|---|---|
| iOS **v3.4** / Android **v3.6** | Aug 2018 | iOS: org UI improvements, updated filter menu, **depth-map support**. Android: **lens-profile control**. Both: new camera/lens support. |

## IMPL
- Treat this table as the **feature-flag → minimum-build** map. If targeting a specific
  build for parity, gate features accordingly.
- Note the **v1.4 sync boundary**: CC↔CC syncs presets/profiles; CC↔Classic does not. Any
  interop layer must respect that asymmetry.
- The **per-photo → per-album offline** change (v1.5) is a data-model migration, not just
  UI — see file 10.
