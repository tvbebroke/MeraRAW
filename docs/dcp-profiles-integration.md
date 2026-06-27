# DCP Camera Profiles — Integration Spec (for Cursor)

**Audience:** the Cursor agent working in this repo (`~/Desktop/meratech-editor`).
**Goal:** make MeraRAW (a) auto-recognize the right Adobe camera profile from a raw
file's metadata, and (b) apply that profile as the *base rendering* the user grades on
top of — matching how Lightroom/ACR render the "Camera Standard / Portrait / Landscape"
looks.

Read `docs/color-science-reference.md` and respect Contract A4 / B1 before touching the
pipeline. Working space everywhere is **linear Rec.2020, scene-referred, D65** — do not
break that invariant.

---

## 0. What's on disk

- **`camera_profiles/`** at repo root — **4,304 `.dcp` files, flat**, named
  `<UniqueCameraModel> <ProfileName>.dcp`, e.g.:
  - `Sony ILCE-7 Adobe Standard.dcp`
  - `Sony ILCE-7 Camera Portrait.dcp`
  - `Canon EOS 77D Camera Landscape.dcp`
- Profile-name families present: **Adobe Standard** (1,373, every camera) plus
  **Camera Matching** looks — Standard, Portrait, Landscape, Vivid, Neutral, Monochrome,
  Faithful, Natural, Deep, Light, Clear, Flat, etc. (Sony/Canon/Fuji/etc. each expose
  their own subset.)
- These are TIFF-container `.dcp` (DNG Camera Profile) files. **No `.xmp` here** — this
  set is camera profiles only, not creative `.xmp` presets.

> **Do NOT commit this folder to git.** It's ~847 MB and Adobe-licensed (local use only).
> Add `camera_profiles/` to `.gitignore`. Ship it as a **Tauri resource bundle** instead
> (see §7) and build a small committed *index* (§3) so the app finds profiles at runtime.

---

## 1. What a DCP actually contains (the parts that matter)

A `.dcp` is a TIFF IFD with Adobe DNG profile tags. The ones we use:

| Tag | Name | Role |
|---|---|---|
| 50708 | `UniqueCameraModel` | the camera this profile is locked to — **use for matching** |
| 50936 | `ProfileName` | e.g. "Camera Portrait" — **use for the look label** |
| 50778/50779 | `CalibrationIlluminant1/2` | the two illuminants (usually StdA ≈2856K, D65 ≈6504K) |
| 50721/50722 | `ColorMatrix1/2` | XYZ(D50)→camera, per illuminant (same role as rawler's matrices) |
| 50964/50965 | `ForwardMatrix1/2` | **camera→XYZ(D50)**, per illuminant — preferred over inverting ColorMatrix |
| 50982/50983 | `ProfileHueSatMapDims` + `…Data1/2` | 3D HSV delta LUT — the per-hue color "look" |
| 50940 | `ProfileToneCurve` | base tone curve baked into the look |
| 50981/51036 | `ProfileLookTableDims` + `…Data` | second HSV LUT applied after tone curve (creative look) |
| 51109 | `BaselineExposureOffset` | exposure nudge baked into some profiles |

The **base color** comes from ForwardMatrix (dual-illuminant, interpolated by CCT). The
**recognizable Adobe look** comes from HueSatMap + ToneCurve + LookTable. Adobe Standard
profiles typically carry matrices + a mild tone curve; Camera Matching profiles add a
HueSatMap (and sometimes a LookTable) to mimic the camera maker's JPEG rendering.

---

## 2. Where this plugs into the existing pipeline

Current decode→render flow (verified):

- `src-tauri/core/src/raw/mod.rs` — `Decoder` trait, `ImageMeta` (already exposes
  `camera_make`, `camera_model`, `as_shot_wb`, `estimated_cct`), `DecodedImage`.
- `src-tauri/core/src/raw/rawler_decoder.rs` — decode loop does:
  `as-shot WB → CameraCalibration::cam_to_rec2020(wb) → linear Rec.2020` (lines ~204–225).
- `src-tauri/core/src/color.rs` — `CameraCalibration { from_rawler, xyz_to_cam_at(cct),
  estimate_cct(wb), cam_to_rec2020(wb) }`, plus all matrix math, `bradford_adapt`,
  Oklab, `REC2020_TO_XYZ`, etc.
- `src-tauri/core/src/graph/` — render graph (extract → ~8 nodes → mask stage → present).
- `src-tauri/core/src/engine.rs` — owns the `RawlerDecoder`, current doc, metadata API.

**Two seams:**

1. **Base color (matrix) seam** — in `rawler_decoder.rs`, where `cam_to_rec2020` builds
   the camera→working matrix. When a profile is selected, derive that matrix from the
   DCP **ForwardMatrix** instead of rawler's ColorMatrix. (cam→XYZ_D50 → Bradford
   D50→D65 → `XYZ_TO_REC2020`.)

2. **Look seam** — a NEW graph node placed **first** in the chain (right after extract,
   before any user-facing node), applying HueSatMap → ToneCurve → LookTable. This is the
   "foundation layer" the user's edits sit on top of. It must be cache-keyed like the
   other nodes (dirty-from index) so changing the profile re-runs from node 0.

---

## 3. Build a committed profile index (cheap matching, no 847 MB scan at runtime)

Add `scripts/build-profile-index.ts` (or a small Rust `examples/index_profiles.rs`) that
walks `camera_profiles/*.dcp`, parses just the TIFF header for `UniqueCameraModel` +
`ProfileName`, and emits **`src-tauri/core/models/profile_index.json`** (committed; tiny):

```json
{
  "Sony ILCE-7": [
    { "name": "Adobe Standard",  "file": "Sony ILCE-7 Adobe Standard.dcp" },
    { "name": "Camera Portrait", "file": "Sony ILCE-7 Camera Portrait.dcp" },
    { "name": "Camera Landscape","file": "Sony ILCE-7 Camera Landscape.dcp" }
  ]
}
```

Key by **`UniqueCameraModel` read from inside the DCP**, not the filename (filenames are
convenient but the in-file model string is authoritative and is what we match against).
The actual `.dcp` bytes are loaded lazily, only for the profile the user selects.

---

## 4. Recognize the profile from metadata (the matching logic)

Add `src-tauri/core/src/profile/mod.rs`:

```
fn resolve_profiles_for(meta: &ImageMeta, index: &ProfileIndex) -> Vec<ProfileRef>
```

Matching order (first hit wins), using `meta.camera_make` + `meta.camera_model`:

1. **Exact `UniqueCameraModel` match.** rawler's `camera_model` for Sony is e.g.
   `ILCE-7M3`; DCPs use the same DNG model string. Normalize both (trim, collapse spaces,
   case-insensitive) before comparing.
2. **Make + model fallback** if the bare model collides across makes (rare).
3. **No match** → return empty; the UI shows only MeraRAW's native render (current
   behavior) and logs `no DCP for "<make> <model>"`. This is expected for unsupported
   bodies — **not an error**.

**Default selection** when profiles exist:
- Prefer the **as-shot creative style** if the raw's metadata names one (Sony "Creative
  Look"/"Picture Profile", Canon "Picture Style", Fuji "Film Simulation" in MakerNotes) —
  map it to the closest `Camera <X>` profile.
- Else default to **`Adobe Standard`** for that camera.
- Always expose the full list for the camera so the user can switch.

> A DCP is **camera-model-locked**: never apply `Canon …` profile to a Sony raw. If the
> selected profile's `UniqueCameraModel` ≠ the open raw's model, refuse and fall back.

---

## 5. Apply the profile (rendering math)

Per the DNG spec, in this order. Do it in the new look node operating on the
linear Rec.2020 working buffer (convert in/out of the spaces below as needed):

1. **Base matrix (already at decode):** use DCP **ForwardMatrix**, dual-illuminant
   interpolated by the as-shot CCT exactly like `xyz_to_cam_at(cct)` already interpolates
   rawler's matrices — reuse that interpolation helper. cam→XYZ(D50) → Bradford(D50→D65)
   → `XYZ_TO_REC2020`. If ForwardMatrix is absent, invert ColorMatrix (fallback).
2. **HueSatMap:** convert pixel to **HSV in linear ProPhoto** (DNG reference space),
   trilinearly sample the `HueDiv×SatDiv×ValDiv` table, apply **hue shift (deg), sat
   scale, value scale**, convert back. Dual-illuminant: interpolate the two maps by CCT.
3. **ProfileToneCurve:** apply the baked tone curve (encoding-gamma domain per spec).
4. **ProfileLookTable:** second HSV map, applied **after** the tone curve (creative look).
5. **BaselineExposureOffset:** apply as an EV offset if present.

Then the user's existing edit nodes (exposure, WB, tone, color_grade in Oklab, HSL, etc.)
run on top — the profile is the floor, not the finish.

> **Phase this.** Don't build all of step 1–5 at once:
> - **Phase 1 (matrix only):** ForwardMatrix base + profile picker UI. Already a visible,
>   testable win; no LUT code. Ship this first.
> - **Phase 2:** ProfileToneCurve + BaselineExposureOffset.
> - **Phase 3:** HueSatMap + LookTable (the trilinear HSV LUTs — the bulk of the "look").
> Each phase is independently verifiable.

---

## 6. State, UI, persistence

- Add `profile: Option<ProfileSelection>` to the **doc state** (the snapshot type used by
  the undo system — undo is doc-state snapshots, so profile changes get undo for free).
  Store `{ camera_model, profile_name, file }`, not the parsed bytes.
- Tauri command + event: `list_profiles_for_current()` → the matched list; `set_profile(name)`
  → marks the look node dirty from index 0 and re-renders.
- UI: a dropdown in the render/basic panel showing the matched profiles, default
  preselected per §4, with the camera model shown so the user knows why the list is what
  it is. Empty list → "No camera profile available for <model>".
- Parse + cache the chosen DCP once per open (it's small, <100 KB); the look node reads
  the cached parsed struct.

---

## 7. Parsing DCPs in Rust (don't hand-roll TIFF if you can avoid it)

1. **Check `rawler` first.** It already decodes DNG; it may expose DCP/profile structs or
   tag readers we can reuse (it's already our dependency). If it does, use that.
2. **Else** use the `tiff` crate to read the IFD and pull the tags in §1 by number;
   matrices are `SRATIONAL`/`FLOAT` arrays, HueSatMap data is `FLOAT`. Keep this in
   `profile/dcp.rs`, isolated behind a `CameraProfile` struct so the parser can be swapped
   (mirror the Contract B1 pattern used for the decoder).
3. Reference parser for cross-checking values during dev: `dcpTool -d <file> file.xml`
   (installed at `~/.local/bin/dcpTool`) dumps any DCP to readable XML — use it to verify
   your Rust parser reads the same matrices/curves.

---

## 8. Bundling (Tauri)

- `.gitignore` → add `camera_profiles/`.
- Add `camera_profiles/` to `tauri.conf.json` `bundle.resources` so it ships inside the
  app. Resolve at runtime via the Tauri resource dir (`app.path().resource_dir()`), with a
  dev fallback to the repo-root `camera_profiles/`.
- Commit only `profile_index.json` (§3), not the `.dcp` payload.
- Licensing: Adobe profiles are redistributable for the app's local use to render the
  user's own raws; keep them bundled, not re-published as a standalone download.

---

## 9. Testing (use the existing rigs)

- **Selftest rig:** `MERATECH_OPEN=~/Desktop/test-claude-raw/DSC07078.ARW
  MERATECH_SELFTEST=1 MERARAW_SKIP_LICENSE=1 npx tauri dev` → grep log for `selftest-pass`.
  Extend the selftest to assert: a profile was matched for the test camera, and switching
  Adobe Standard → Camera Vivid changes the rendered histogram.
- **Color harness:** `src-tauri/core/tests/color_harness.rs` — add a test that a neutral
  patch stays neutral after the ForwardMatrix path (parallels
  `neutral_camera_pixel_lands_neutral_in_rec2020`), and that HueSatMap with an identity
  table is a no-op (round-trip guard).
- The test ARW is **Sony** — confirm `Sony ILCE-7…` profiles match it. If rawler reports
  `ILCE-7M3` and only `ILCE-7` exists in the set, that's a real (expected) miss — log it;
  don't force a wrong-model profile.

---

## 10. Acceptance criteria

- [ ] `camera_profiles/` gitignored; `profile_index.json` committed.
- [ ] Opening a raw auto-selects a sensible default profile (as-shot style or Adobe Standard).
- [ ] Profile dropdown lists exactly the profiles whose `UniqueCameraModel` matches the raw.
- [ ] Switching profile visibly changes the base render and is undoable.
- [ ] Wrong-model profiles can never be applied.
- [ ] Unsupported camera → graceful empty list, logged, no crash.
- [ ] Working-space invariant (linear Rec.2020 D65) preserved; color_harness passes.
- [ ] Phases shippable independently (matrix → tone curve → HueSatMap/LookTable).

---

## TL;DR for Cursor

1. Gitignore the folder; build a committed `profile_index.json` keyed by in-file
   `UniqueCameraModel`.
2. Match the open raw's `meta.camera_model` to the index; default to as-shot style or
   Adobe Standard.
3. Phase 1: derive the camera→Rec.2020 base matrix from the DCP **ForwardMatrix** in the
   decoder, add the picker UI, make it undoable.
4. Phase 2/3: add a first-in-chain "profile look" graph node doing ToneCurve, then
   HueSatMap + LookTable — the part that reproduces Adobe's recognizable looks.
5. Verify with the selftest rig (Sony test ARW) and color_harness; cross-check your DCP
   parser against `dcpTool -d`.
