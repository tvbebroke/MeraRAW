# MeraRaw — Build Project

**Goal:** Bring MeraRaw's image import pipeline to parity with Adobe Lightroom's format coverage — standard images, Adobe formats, and camera RAW from all major manufacturers.

**Approach:** Don't reverse-engineer proprietary codecs. Nearly every format below has a mature, actively-maintained open-source decoder. The engineering work is building a clean abstraction layer, a robust format-detection/dispatch system, and correct color/metadata handling on top of those libraries — not reimplementing demosaicing algorithms or Adobe's internal file structures from scratch.

---

## 1. Architecture Overview

```
meraraw/
├── src/
│   ├── core/
│   │   ├── image_buffer.{h,cpp}       # unified internal image representation
│   │   ├── color_management.{h,cpp}   # ICC/ACES profile handling, LCMS2 wrapper
│   │   ├── metadata.{h,cpp}           # EXIF/XMP/IPTC unified model
│   │   └── format_registry.{h,cpp}    # extension/magic-byte -> decoder dispatch
│   ├── decoders/
│   │   ├── standard/                  # jpeg, png, tiff, webp, avif, heif, jxl
│   │   ├── adobe/                     # dng, psd, psb
│   │   ├── raw/                       # libraw-backed camera raw
│   │   └── cinema/                    # r3d, braw (SDK-gated, see §5)
│   ├── pipeline/
│   │   ├── demosaic.{h,cpp}
│   │   ├── highlight_recovery.{h,cpp}
│   │   └── tone_mapping.{h,cpp}
│   └── ingest/
│       └── file_probe.{h,cpp}         # sniff real format regardless of extension
├── third_party/                       # vendored or submodule'd libs (see §2)
├── tests/
│   └── fixtures/                      # one sample per format, per §6
├── CMakeLists.txt
└── vcpkg.json / conanfile.txt
```

**Core design principle:** every decoder returns the same internal type — a 32-bit float linear buffer + attached ICC profile + metadata struct. Format-specific weirdness gets normalized at the decoder boundary so the rest of the app never branches on file type.

```cpp
struct DecodedImage {
    std::vector<float> pixels;   // linear light, RGB or RGBA
    int width, height, channels;
    ColorProfile profile;        // embedded or inferred
    ImageMetadata meta;          // EXIF/XMP/IPTC + camera-specific tags
    BitDepth sourceDepth;        // 8/16/32, for UI display purposes
};
```

---

## 2. Library Dependencies

| Format group | Library | License | Notes |
|---|---|---|---|
| JPEG | libjpeg-turbo | BSD-style | SIMD-accelerated baseline |
| PNG | libpng + zlib | zlib/libpng | |
| TIFF | libtiff | BSD-style | Also underlies DNG (subset) |
| WebP | libwebp | BSD | Google |
| AVIF | libavif + dav1d/libaom | BSD-2 | dav1d for decode speed |
| HEIC/HEIF/HIF | libheif | LGPL 3 | Wraps libde265 or built-in decoder |
| JPEG XL | libjxl | BSD-3 | Reference implementation |
| DNG | LibRaw or Adobe DNG SDK | LGPL/BSD (LibRaw), custom (Adobe SDK) | See §4 |
| PSD/PSB | libpsd or custom parser | LGPL / write your own | Spec is documented, see §4 |
| Camera RAW (all manufacturers) | **LibRaw** | LGPL 2.1 / CDDL dual | Single library covers ~95% of the list below |
| RED R3D | REDCODE SDK | Proprietary, licensed from RED | No open alternative; SDK required |
| Blackmagic RAW | BRAW SDK | Proprietary, free from Blackmagic | Same situation as R3D |
| GoPro .gpr | libgpr | BSD (Apache-licensed via GoPro) | GoPro's own open-source repo |
| Color management | Little CMS 2 (LCMS2) | MIT | ICC profile conversion |
| Metadata | Exiv2 or exiftool (shell out) | GPL-2 (Exiv2) | XMP/EXIF/IPTC read-write |

**Build system:** CMake + vcpkg (or Conan) for dependency management. Pin exact versions in `vcpkg.json` — RAW format support is a moving target and library updates regularly add new camera models.

---

## 3. LibRaw: The Workhorse

LibRaw handles the overwhelming majority of your manufacturer list in one integration:

```
CR2, CR3, CRW, NEF, NRW, ARW, SRF, SR2, RAF, ORF, RW2, PEF, SRW,
X3F, DCR, KDC, MRW, BAY, ERF, MEF, 3FR, FFF, MOS, IIQ, RWL, RWZ,
CAP, CINE, IA, KC2, MDC, PXN, QTK, STI
```

Integration pattern:

```cpp
#include <libraw/libraw.h>

DecodedImage decodeRaw(const std::string& path) {
    LibRaw processor;
    processor.open_file(path.c_str());
    processor.unpack();

    // Nuance: LibRaw gives you sensor-native Bayer/X-Trans data by default.
    // You must explicitly request demosaicing + color space conversion.
    processor.imgdata.params.output_color = 1;   // sRGB, or use ACES/ProPhoto for editing
    processor.imgdata.params.output_bps   = 16;
    processor.imgdata.params.use_camera_wb = 1;   // respect embedded white balance
    processor.imgdata.params.no_auto_bright = 1;  // don't let LibRaw guess exposure

    processor.dcraw_process();
    libraw_processed_image_t* img = processor.dcraw_make_mem_image();
    // ... copy into DecodedImage, attach embedded ICC if present
}
```

**Key nuances LibRaw does NOT solve for you:**

- **Sensor pattern varies by manufacturer.** Bayer (RGGB) for most; X-Trans for Fujifilm (6x6 pattern, needs a different demosaic algorithm — LibRaw's AHD/DCB work but dedicated X-Trans demosaicing (Markesteijn) gives better results and is worth implementing separately for `.raf`).
- **Foveon sensors (Sigma `.x3f`)** are not Bayer at all — full RGB per photosite, stacked. LibRaw's generic demosaic path is wrong for these; use LibRaw's dedicated Foveon interpolation path (`processor.imgdata.idata.filters == 0`).
- **White balance multipliers** differ per camera and are stored in proprietary maker notes. LibRaw parses most of these but new camera models lag behind LibRaw releases — expect to occasionally patch or wait for upstream fixes when a brand-new camera ships.
- **Highlight recovery** behaves differently depending on how many channels are clipped (1 vs 2 vs 3) — implement as a distinct pipeline stage, not inside the decoder.
- **Dual-pixel/dual-gain sensors** (recent Canon/Sony) can carry two exposures in one RAW file; naive decode picks one, losing dynamic range. Check `imgdata.rawdata.color.dng_levels` for dual-gain flags on affected models.

---

## 4. Adobe Formats

### DNG
DNG is TIFF-based and **openly specified** (Adobe publishes the full spec + a free DNG SDK). Two integration paths:
1. **LibRaw** — treats DNG as just another RAW format, good enough for basic linear-DNG and mosaiced-DNG.
2. **Adobe DNG SDK** — needed for full fidelity: opcode lists (per-image corrections baked in by the camera), DNG 1.6 gain maps (HDR reconstruction — required for **Apple ProRAW**), and dual-illuminant color matrices.

**Apple ProRAW nuance:** it's DNG under the hood but includes Apple-specific gain map tags for HDR and has already been partially demosaiced/computationally processed on-device before being written. Treat it as a distinct code path from "normal" DNG — a naive RAW pipeline (full manual WB/tone curve control) will look wrong on ProRAW files because a lot of processing is already baked in.

### PSD / PSB
Both are documented by Adobe (public "Photoshop File Format" spec, unchanged in structure since PSB just extends PSD to 64-bit offsets for files >2GB or >30,000px).

Structural nuances to handle explicitly:
- **Layered vs flattened data**: PSD stores a merged composite *and* individual layers. For a photo editor, decode the composite (fast path) but expose layer access as an optional deeper parse.
- **Color mode header**: PSD can be RGB, CMYK, Lab, Indexed, Duotone, or Multichannel. Support RGB and CMYK at minimum; convert CMYK → RGB via ICC profile (Lightroom does the same — CMYK PSD/TIFF import but internal editing space is always RGB).
- **Compression**: RAW, RLE (PackBits), ZIP, or ZIP+prediction — must handle all four per-channel.
- **Smart objects / adjustment layers**: not renderable without the Photoshop engine; for import purposes, just read the flattened composite and ignore editable layer intelligence.

A minimal from-spec parser is very feasible (the format is well documented); `libpsd` exists but is unmaintained — budget time to fork and patch it, or write a focused reader covering just the composite-image path if full layer fidelity isn't a v1 requirement.

---

## 5. Cinema RAW Formats (R3D, BRAW)

These are the two formats that genuinely can't be handled with an open-source library:

- **RED R3D** requires the REDCODE SDK, licensed directly from RED Digital Cinema. It's free to obtain but requires a developer agreement.
- **Blackmagic RAW (.braw)** requires the free Blackmagic RAW SDK, available from Blackmagic Design's developer site, no cost but requires accepting their SDK license.

Both SDKs are precompiled, closed-source libraries you link against — you call their decode APIs, you don't touch internal format details. Gate these behind a separate optional build flag (`MERARAW_ENABLE_CINEMA_RAW`) since they add licensing obligations and platform-specific binary dependencies that shouldn't block the core photo workflow.

`.ari` (ARRI) is similarly SDK-gated via ARRI's own reference tools; lower priority unless you're specifically targeting cinematographers.

---

## 6. Nuance Summary by Category

| Category | Key gotcha |
|---|---|
| JPEG XL | Can wrap a JPEG losslessly (`.jxl` re-encode of `.jpg`) — detect and handle the "transcoded JPEG" case for perfect round-trip |
| HEIC/HEIF/HIF | Container format — can hold HEVC, AVC, or AVIF-coded image data; also often multi-image (burst/depth map). Decode primary image by default, expose auxiliary images as opt-in |
| AVIF | Check for AV1 profile support (Main vs High) — some encoders produce 10/12-bit that a naive 8-bit pipeline will clip |
| TIFF | "TIFF" is a container family, not one format — BigTIFF, tiled vs stripped, and compression (LZW/ZIP/JPEG-in-TIFF/uncompressed) all vary |
| X-Trans (Fuji RAF) | Standard Bayer demosaic algorithms produce visible artifacts; needs X-Trans-aware demosaicing |
| Foveon (X3F) | Not a Bayer sensor — full RGB per pixel, wrong demosaic path will corrupt colors entirely |
| DNG | "DNG" isn't one format — linear DNG, mosaiced DNG, and ProRAW-style DNG-with-gain-maps all need different handling |
| CMYK TIFF/PSD | Must convert via ICC to RGB before entering the editing pipeline — never edit natively in CMYK |
| Maker notes (all RAW) | Proprietary per-manufacturer EXIF blocks; Exiv2/LibRaw parse known ones, expect gaps on brand-new camera models until libraries update |

---

## 7. Testing Strategy

- One real-world sample file per extension in `tests/fixtures/`, sourced from public camera sample-file repositories (e.g., rawsamples.ch, manufacturer press-kit RAWs) — never synthetic files, since real sensor quirks only show up in real captures.
- Golden-image regression tests: decode → render → compare against a reference PNG within a perceptual diff threshold (small demosaic/library-version drift is expected and shouldn't fail CI).
- Fuzz the format-detection/probe layer (`file_probe.cpp`) with malformed/truncated files — this is the code most likely to crash on corrupt user files in production.

---

## 8. Suggested Build Milestones

1. **Core pipeline + standard formats** (JPEG, PNG, TIFF, WebP) — validates the `DecodedImage` abstraction
2. **AVIF, HEIC/HEIF/HIF, JPEG XL** — modern container formats, exercises the color-profile/HDR paths
3. **LibRaw integration** — unlocks ~30 RAW extensions at once
4. **DNG deep support** (opcode lists, ProRAW gain maps) + PSD/PSB
5. **X-Trans and Foveon dedicated demosaic paths** — quality pass on the two RAW outliers
6. **Cinema RAW (R3D, BRAW) behind optional SDK flag** — only if targeting video-adjacent users

---

## 9. Licensing Note

LGPL components (LibRaw, libheif, libpng) are safe to dynamically link without triggering copyleft on your own codebase — just ship as shared libraries and don't statically link into a closed binary without complying with LGPL's relinking requirement. GPL components (Exiv2) are stricter; consider exiftool via subprocess call instead if you want to avoid GPL linkage entirely. Review each license before your first public release.
