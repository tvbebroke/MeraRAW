# Phase 02 — Codec Layer (Decode & Encode)

**Goal:** A unified decode/encode boundary that turns files (JPEG/PNG/TIFF/WebP/…) into your `core` image buffer and back, with room to swap in faster codecs where it pays.

**Dependencies & licenses**
| Dep | Role | License | Verdict |
|---|---|---|---|
| `image` (image-rs) | broad decode/encode, the backbone | Apache-2.0 OR MIT | 🟢 embed freely |
| `zune-image` / `zune-jpeg` | faster decode where needed | MIT/Apache/Zlib | 🟢 embed freely (verify) |
| `image-png` / `image-tiff` / `image-webp` | per-format control | MIT/Apache | 🟢 embed freely (verify) |

RAW is **not** here — it has its own phase (03).

---

## Architecture

One trait, many backends. This lets you start on `image` and later route hot formats through `zune` without touching callers.

```rust
// crates/codec/src/lib.rs
pub trait Decoder {
    fn probe(&self, bytes: &[u8]) -> Option<Format>;
    fn decode(&self, bytes: &[u8]) -> Result<CoreImage, CodecError>; // -> core buffer
}
pub trait Encoder {
    fn encode(&self, img: &CoreImage, opts: &EncodeOpts) -> Result<Vec<u8>, CodecError>;
}
```

`CoreImage` (defined in `crates/core`) should carry **bit depth and color space**, not just RGBA8 — you need ≥16-bit and linear/sRGB awareness for a serious editor. Don't collapse everything to 8-bit at the codec boundary.

---

## Sub-phases

### 02.1 — Baseline decode/encode via `image`
```rust
use image::io::Reader as ImageReader;
let img = ImageReader::open(path)?.with_guessed_format()?.decode()?;
img.save("out.png")?;
```
- Cover JPEG, PNG, TIFF, WebP, BMP, GIF read; JPEG/PNG/TIFF/WebP write.
- Map `image`'s `DynamicImage` into your `CoreImage` (preserve bit depth: `Rgb16`, `Rgba16`, `Rgb32F` where present).

### 02.2 — Format probing & routing
- Implement content-based format detection (magic bytes), not extension-based.
- Build the backend router: default to `image`, route JPEG decode to `zune-jpeg` if the benchmark (02.4) says it wins.

### 02.3 — Encode options
- Expose quality (JPEG/WebP), compression level (PNG), bit depth, chroma subsampling.
- Ensure metadata pass-through hooks exist (actual EXIF/ICC handling lands in Phase 11).

### 02.4 — Benchmark & decide
- Micro-benchmark decode throughput + peak memory: `image` vs `zune` on a representative set (large JPEGs, 16-bit TIFFs).
- Keep whichever wins per format behind the router. Document the numbers.

---

## Evaluate against your editor / RapidRAW
- Format breadth: list what you decode/encode today vs this set. Gaps = quick wins.
- Bit-depth fidelity: do you carry 16-bit through decode, or truncate to 8? Truncating early is a silent quality loss.
- Decode speed on large files vs `zune-jpeg` — run the head-to-head.

## Testing & acceptance criteria
- [ ] Round-trip decode→encode→decode is lossless for lossless formats (PNG/TIFF) — pixel-identical.
- [ ] 16-bit TIFF stays 16-bit through the boundary (no silent 8-bit truncation).
- [ ] Content-based probe correctly IDs files with wrong/missing extensions.
- [ ] Benchmark report committed; router uses the faster backend per format.
- [ ] Corrupt/truncated file returns a clean `CodecError`, never panics.

## Risks / gotchas
- **Silent bit-depth loss** is the classic bug — assert bit depth in tests.
- `zune`'s buffer types differ from `image`'s; keep the conversion in one place.
- Untrusted input: prefer decoders that return errors over panics (relevant if you ever decode server-side).
