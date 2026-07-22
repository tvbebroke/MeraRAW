# Phase 04 — GPU Processing Pipeline & Color Management

**Goal:** A GPU-accelerated, float-precision pipeline that takes a decoded image + an `EditStack` and produces the rendered output. This is the engine every adjustment plugs into, and the core of "feels instant."

**Dependencies & licenses**
| Dep | Role | License | Verdict |
|---|---|---|---|
| `wgpu` | GPU abstraction (Vulkan/Metal/DX/GL) | Apache/MIT | 🟢 embed freely |
| `bytemuck` | POD casting for GPU buffers | Zlib/Apache/MIT | 🟢 embed freely |
| your WGSL shaders | the actual ops | yours | 🟢 |
| `lcms2` bindings *or* own | ICC transforms (optional) | check (lcms2 = MIT) | 🟢 (verify) |

**Reference (study only):** RapidRAW offloads its entire pipeline to GPU via WGPU + custom WGSL, 32-bit float (AGPL — architecture study only). `zenraw`'s linear-f32 scene-referred output is the color target to match.

---

## Architecture

**Work in linear light, float precision, all the way through; convert to display space only at the end.** Every adjustment is a shader pass over a float texture.

```
input (linear f32 RGBA texture)
  → pass: exposure / contrast / white balance
  → pass: tone curve / highlights / shadows
  → pass: HSL / color grading
  → pass: local adjustments (masked — Phase 09)
  → pass: effects (Phase 07) / denoise (Phase 08)
  → output transform: linear → working ICC → display (sRGB/Display-P3)
  → display OR encode (Phase 05)
```

```rust
// crates/pipeline/src/lib.rs
pub struct Pipeline { device: wgpu::Device, queue: wgpu::Queue, passes: Vec<Box<dyn Pass>> }
pub trait Pass { fn record(&self, enc: &mut wgpu::CommandEncoder, io: &TexIO, params: &Params); }

impl Pipeline {
    pub fn render(&self, input: &GpuImage, edits: &EditStack) -> GpuImage { /* run passes */ }
}
```

**Two render paths:**
- **Preview** — render at display resolution for interactivity (target < 16ms per edit).
- **Full-res** — render at native resolution on export (Phase 05).

---

## Sub-phases

### 04.1 — WGPU bootstrap
- Init device/queue; upload an image as an `rgba32float` texture; render it back to screen unchanged (identity pass). This proves the plumbing.

### 04.2 — Pass framework
- Define the `Pass` trait, a texture ping-pong (read tex A → write tex B → swap), and a uniform/params upload path.
- Implement the first real pass: **exposure** (a multiply in linear space). Confirm slider → shader → preview updates.

### 04.3 — Core tonal adjustments (WGSL)
- Exposure, contrast, white balance (temp/tint), highlights/shadows/whites/blacks, tone curve (as a LUT texture), saturation/vibrance, HSL per-channel.
- Each as its own pass with params from `EditStack.global`.

### 04.4 — Color management
- Define your **working space** (linear). Implement the output transform: working → display (sRGB and Display-P3 minimum).
- ICC input/output profile handling (via `lcms2` or your own) so monitor + export profiles are honored. Study Phase 11's `colorpeek` reference for reading embedded profiles.

### 04.5 — Preview/full-res split & caching
- Cache the decoded input texture; re-run only downstream passes when a param changes.
- Consider caching intermediate textures at pass boundaries so a late-stage slider doesn't re-run early passes.

---

## Evaluate against your editor / RapidRAW
- **Precision:** are you 8-bit or float through the pipeline? Float is the correctness/quality baseline RapidRAW and zenraw set.
- **Latency:** measure edit-slider → updated-preview. RapidRAW's selling point is fluid preview even with many masks — benchmark against that feel.
- **Color correctness:** does neutral stay neutral end-to-end? Does linear-light editing match a reference?
- **Pass architecture:** is your pipeline a fixed function or a composable pass graph? A graph makes adding features (07/08/09) cheap.

## Testing & acceptance criteria
- [ ] Identity pass is pixel-exact (no unintended color shift from the round trip).
- [ ] Exposure +1 EV in linear = exactly 2× linear values (verify numerically).
- [ ] Neutral gray stays neutral through the full pass chain.
- [ ] Preview render < 16ms at display res on target hardware; full-res render completes and matches preview (scaled).
- [ ] sRGB and Display-P3 output transforms verified against reference swatches.

## Risks / gotchas
- **Doing math in sRGB/gamma space** instead of linear is the most common color bug — blends and blurs must be linear.
- **GPU/driver variance:** test on Vulkan, Metal, and DX backends; WGSL precision can differ.
- **VRAM on huge images:** tile or downscale-for-preview; don't hold multiple full-res float textures needlessly.
- Keep the CPU fallback in mind for machines without capable GPUs (RapidRAW notes slowness on old GPUs).
