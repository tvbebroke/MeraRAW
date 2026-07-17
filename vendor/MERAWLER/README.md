# MERAWLER

A custom RAW **demosaic engine** for MeraRAW. It sits on top of the pure-Rust
[`rawler`](https://crates.io/crates/rawler) decoder and adds high-quality
demosaicing algorithms that don't exist in the Rust ecosystem yet — the ones
that make or break color and detail in a RAW developer.

## Why

`rawler` decodes almost every RAW format but ships only basic demosaicing.
The algorithms that photographers actually care about live in C/C++
(RawTherapee, darktable, librtprocess). MERAWLER reimplements them in Rust so
the whole MeraRAW pipeline stays pure-Rust and controllable.

## Design

The math is decoupled from the decoder. Every algorithm consumes a normalized
Bayer mosaic and produces a linear RGB image, so the algorithms have **zero
external dependencies** and are unit-tested on synthetic mosaics.

```
 rawler decode ──▶ rawler_adapter ──▶ CfaImage ──▶ Demosaic trait ──▶ RgbImage
   (feature "decode")                 (pure core, no deps, tested)
```

- `image.rs` — `CfaImage` (normalized mosaic + `CfaPattern`) and `RgbImage`.
- `demosaic/` — the `Demosaic` trait, `Algorithm` enum, and implementations.
- `rawler_adapter.rs` — `RawImage` → `CfaImage` (black/white-level
  normalization + pattern detection). Behind the `decode` feature.
- `bin/merawler.rs` — CLI preview tool. Behind the `decode` feature.

## Algorithm roadmap

| Algorithm | Status  | Notes                                                        |
|-----------|---------|-------------------------------------------------------------|
| Bilinear  | ✅ ready | Baseline + edge fallback for kernel methods.                |
| Malvar    | ✅ ready | Malvar–He–Cutler gradient-corrected bilinear.               |
| **RCD**   | ✅ ready | darktable default: excellent color, few artifacts, ~1.4s/24MP. |
| LMMSE     | ✅ ready | Best on noisy captures; directional MMSE fusion, ~1.6s/24MP.|
| AMaZE     | ✅ ready | Maximum detail (landscape/astro); tiled, ~2.8s/24MP.        |
| IGV       | ✅ ready | Integrated Gaussian vector on color differences, ~1.4s/24MP.|
| DDFAPD    | ✅ ready | Menon (2007) directional filtering + a posteriori decision.|

All seven algorithms are implemented. Scope is classic 2x2 Bayer; X-Trans
(Markesteijn) is intentionally excluded.

## Build & run

```sh
# Core lib + algorithm unit tests (fast, no heavy deps):
cargo test

# CLI (pulls rawler + image):
cargo build --release --features decode

# Demosaic a RAW to a viewable PNG preview:
./target/release/merawler photo.ARW --algo malvar -o out.png
./target/release/merawler --list
```

The CLI is a **preview** tool: it applies as-shot white balance and an sRGB
curve so you can judge demosaic quality. Full color management lives in the
MeraRAW editor, which will consume `CfaImage`/`RgbImage` directly via its B1
decoder trait.

## Benchmark

An objective benchmark ships as the `bench` binary (feature `bench`). It mosaics
known RGB images and measures PSNR / chroma-PSNR / timing against the truth,
using the engine's own `Demosaic` implementations.

```sh
cargo build --release --features bench
# synthetic tests (+ a real photo as ground truth):
./target/release/bench --truth clean_rgb.png
# average CPSNR over an image set (e.g. Kodak):
./target/release/bench --dir /path/to/kodak
# tile crops for a visual comparison:
./target/release/montage out.png <cols> <cell_px> a.png b.png ...
```

Kodak-24 CPSNR (validates the ports against published numbers): IGV 39.7,
AMaZE 39.1, DDFAPD 39.1, LMMSE 38.4, RCD 36.9, Malvar 35.6, bilinear 30.2 dB.
