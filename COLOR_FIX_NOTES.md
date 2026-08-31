# Color pipeline fixes (v0.1.8-local)

## Root causes fixed

1. **NaN white balance** (CRW, DCR, early SRF reads): `effective_wb()` falls back to calibration `daylight_wb()` instead of unity.
2. **Sony F828 RGBE / SRF**: Adobe DCP colour matrix assumes 3-channel RGB; on RGBE→RGB merge it zeroed blue. `resolve_cam2rec()` skips DCP matrix when `cpp==4` or CFA is RGBE. Force rawler FourColor demosaic (not merawler Bayer).
3. **No-DCP cameras** (S2 CR2, iPhone DNG, Hasselblad 3FR): `builtin_standard` + look **7** with luma-adaptive gain (dim scenes darker, bright scenes lifted).
4. **Orange highlight petals** (SR2, RAF): tone curve luma-blend delayed for warm hues (`blend_start = 0.85 + 0.12×warmth`, blend weight × `(1 − 0.85×warmth)`). Look 5 grade tuned to Affinity.
5. **File picker**: `PHOTO_EXTENSIONS` now lists all rawler extensions (MRW, RWL, SR2, SRF, X3F, …).

## Affinity sweep (local mirror `/Users/andrewliang/Downloads/MeraRaw/raw-test-local`)

Run: `python3 scripts/affinity_sweep.py /tmp/out`

| Status | Meaning |
|--------|---------|
| GOOD | \|Δluma\| < 6 and \|Δsat\| < 0.05 |
| OK | \|Δluma\| < 12 and \|Δsat\| < 0.10 |
| BAD | still off |
| FAIL | rawler cannot decode (needs upstream camera support) |

### Latest Affinity sweep (v9)

**GOOD=19  OK=4  BAD=1  FAIL=6**

Focus formats: MRW / NEF / NRW / ARW / RW2 all **GOOD**. Remaining BAD is Sony F828 SRF (RGBE). iPhone DNG moved to OK.

Pipeline fixes are **per camera model / stage**, not per test photo:
- FZ45→FZ40 Adobe DCP alias, highlight magenta gate (R≈B≫G only)
- Prefer Adobe Standard over RawTherapee embedded curves (A7III etc.)
- iPhone / Nikon AW1 loaded-DCP EV biases; Minolta/Kodak builtin EV
- Tone-curve near-neutral → luma remap; softened sky cyan crush

## App bundle

`MeraRAW-v0.1.8-local/.cargo-target/release/bundle/macos/MeraRAW Beta 0.1.8-local.app`
