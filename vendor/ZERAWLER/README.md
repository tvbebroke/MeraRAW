# ZERAWLER

A **sidecar RAW engine**: instead of reimplementing demosaic algorithms (that's
[merawler](../MERAWLER)'s job), zerawler **delegates to the canonical
implementations** by spawning external executables across a process boundary —
the same pattern as a Tauri sidecar. Upstream keeps the bugs fixed and the SIMD
tuned; the process boundary keeps GPL code out of the host application.

```
 host app (proprietary)                    external workers (their licenses)
 ┌────────────────────┐   argv / files    ┌──────────────────────────────────┐
 │ zerawler crate     │ ────────────────► │ rawtherapee-cli   GPL-3.0        │
 │  Engine::decode()  │                   │   → RCD, LMMSE, AMaZE            │
 │  (spawn + parse    │ ◄──────────────── │ dcraw_emu (LibRaw) LGPL-2.1/CDDL │
 │   TIFF output)     │   16-bit TIFF     │   → DHT, AHD, DCB, AAHD          │
 └────────────────────┘                   └──────────────────────────────────┘
```

## Algorithms

| Name  | Backend | Author / citation | 24 MP time* |
|-------|---------|-------------------|------------|
| rcd   | RawTherapee | L. Sanz Rodríguez, Ratio Corrected Demosaicing | ~1.3 s |
| lmmse | RawTherapee | Zhang & Wu, IEEE TIP 2005 | ~1.3 s |
| amaze | RawTherapee | E. Martinec, 2010 | ~1.5 s |
| dht   | LibRaw | A. Petrusevich | ~0.9 s |
| ahd   | LibRaw | Hirakawa & Parks | ~0.7 s |
| dcb   | LibRaw | J. Góźdź | ~2.2 s |
| aahd  | LibRaw | modified AHD | ~4.0 s |

\* end-to-end (full decode + demosaic + encode) on a Sony A7 III ARW, M1 Max.
Each call decodes the whole file in the worker — there is no mosaic-level seam
like merawler's; that's the trade for zero reimplementation.

## Output contracts (`Mode`) — validated 2026-07-05

Certified with `validate_rt` (same RAW decoded via merawler as camera-native
ground truth; NCC alignment; affine 3×4 fit per transfer-curve candidate,
near-clip samples excluded).

* **Native, LibRaw backend** — **linear camera-native RGB, unity WB**
  (`-q N -o 0 -r 1 1 1 1 -c 0 -4 -T`, plus `-k/-S` when the caller forces its
  own black/white levels via `DecodeOpts`). Validated: linear TRC, residual
  0.97%, per-channel deviation ≤3% vs rawler normalization (≈exact with
  forced levels). EXIF orientation baked by the worker.
* **Native, RawTherapee backend** — **linear Rec.2020, camera WB**, highlights
  clamped at 1.0. RT bakes an sRGB transfer curve into its RTv4_Rec2020 output
  (even 32-bit float); the backend decodes it internally, so consumers receive
  genuinely linear data. Validated: residual 0.62%, best-fit TRC = linear — a
  pure linear relation to camera-native (the matrix = RT's WB × cam→Rec2020).
  Note: true camera-native is NOT reachable from RT via supported pp3 keys —
  differential tests show a camera→working matrix persists even with
  `InputProfile=(none)` + no-ICM output; hence this managed contract instead.
  RT's camera matrix ≠ the host's own calibration, so base rendition differs
  slightly from in-process paths; headroom >1.0 is lost.
* **Preview** — the worker's own camera WB + display rendering; for humans.

All worker invocations run under a hard deadline (default 120 s,
`DecodeOpts::timeout`); a hung worker is killed and surfaces as an error the
caller can fall back from — necessary because the bundled rawtherapee-cli
demonstrably hangs pre-`main` when its sandbox entitlement misfires.

```sh
# re-run certification (needs merawler checkout next door):
cargo build --release --features validate
./target/release/validate_rt photo.ARW rcd   # RT backend
./target/release/validate_rt photo.ARW dht   # LibRaw backend
```

## Usage

```sh
cargo build --release
./target/release/zerawler --engines          # show detected workers
./target/release/zerawler --list             # algorithms + carrying backend
./target/release/zerawler photo.ARW --algo amaze -o out.png
./target/release/zerawler photo.ARW --algo dht --native -o out.tif
```

Worker resolution: `ZERAWLER_RT_CLI` / `ZERAWLER_DCRAW_EMU` env overrides →
`ZERAWLER_WORKERS_DIR` (falls back to this checkout's `workers/`) → known
install paths → `$PATH`. Embedding hosts should skip detection and pass
resolved sidecar paths into `Engine { rt_cli, dcraw_emu }` directly.

## macOS gotcha: the bundled rawtherapee-cli deadlocks

RawTherapee's bundled `rawtherapee-cli` carries the
`com.apple.security.app-sandbox` entitlement **and** the app ships quarantined.
Exec'd directly (headless, no LaunchServices), it hangs forever inside
`_libsecinit_appsandbox` (dyld initializer) — or SIGTRAPs when launched from
inside another sandbox.

Fix (applied in `workers/`): copy the binary out, `xattr -c` the copy, and
ad-hoc re-sign it **without entitlements**:

```sh
cp /Applications/RawTherapee.app/Contents/MacOS/rawtherapee-cli workers/
xattr -c workers/rawtherapee-cli
codesign --force --sign - workers/rawtherapee-cli
```

The copy still loads RT's dylibs from the original bundle (absolute install
names). This is also exactly the preparation you'd do to bundle it as a Tauri
sidecar. The installed app is not modified.

## Tauri sidecar integration (for MeraRAW)

1. Place per-target worker binaries at e.g.
   `src-tauri/binaries/rawtherapee-cli-aarch64-apple-darwin` (Tauri requires
   the target-triple suffix), prepared as above.
2. `tauri.conf.json`:
   ```json
   { "bundle": { "externalBin": ["binaries/rawtherapee-cli", "binaries/dcraw_emu"] } }
   ```
3. Resolve the sidecar path at runtime and hand it to zerawler:
   `Engine { rt_cli: Some(resolved), dcraw_emu: Some(resolved) }` — the crate
   itself is Tauri-free (mirrors meratech-core's "no Tauri deps" rule) and
   only needs paths.
4. RCD/LMMSE/AMaZE decode whole files (~1.3–1.5 s) per call, so an editor
   integration re-decodes on algorithm switch, same as the merawler path.

## Licensing

* **zerawler crate** — proprietary (no foreign code linked; spawns processes).
* **rawtherapee-cli** — GPL-3.0. Exec'd at arm's length as a separate program
  (argv + files), which does not make the host a derivative work. If you
  *distribute* the binary with an app, GPL compliance applies **to that
  binary**: ship its license text and a source offer (it is unmodified
  upstream RawTherapee 5.12). Alternative: detect a user-installed
  RawTherapee and skip bundling entirely.
* **dcraw_emu / LibRaw** — LGPL-2.1 OR CDDL-1.0; permissive to bundle
  (include the license text). This is why DHT is the lowest-friction
  algorithm of the set — as suggested.

## vs. merawler

| | merawler | zerawler |
|---|---|---|
| Implementation | clean-room Rust, in-process | canonical upstream, subprocess |
| Seam | mosaic in → RGB out (fits rawler decode) | whole RAW file in → RGB out |
| Maintenance | ours | upstream's |
| Colour contract | exact (feeds MeraRAW landing) | native mode per-backend, RT needs validation |
| Licensing | fully proprietary-safe | process boundary; GPL applies to shipped workers |
| Latency | in-process | + process spawn + TIFF round-trip |
