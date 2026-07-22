# Phase 07 — Filters & Effects

**Goal:** A catalog of creative filters/effects (film looks, duotone, split-tone, grain, vignette, channel mixers, convolutions) implemented as pipeline passes, plus a preset/LUT system.

**Dependencies & licenses**
| Dep | Role | License | Verdict |
|---|---|---|---|
| your WGSL passes | the effects | yours | 🟢 |
| `photon-rs` | effect **reference / checklist** (or light dep) | Apache-2.0 | 🟢 embed freely (verify) |

**How to use `photon` as a checklist:** its filter catalog is a ready-made feature list. Go down it, mark what you have, implement the gaps. Better mined for ideas + reference implementations than embedded wholesale (it's "effects," not a color-accurate core).

**Reference (study only):** `joaopfsilva/patina` (film sims), `jeremieLouvaert/ComfyUI-Darkroom` (film-emulation curves) for look design.

---

## Architecture

Effects are **pipeline passes** (Phase 04) parameterized by the `EditStack`. Two kinds:
- **Per-pixel** (curves, channel mixer, duotone, grain, vignette) — trivial shader passes.
- **Neighborhood** (blur, sharpen, clarity, bloom) — convolution/separable passes.

**LUT support** is high-leverage: a 3D LUT (`.cube`) applied as a shader texture lets you implement film looks and let users import third-party looks.

```rust
// crates/effects/src/lib.rs
pub enum Effect {
    Curves(CurveSet), ChannelMixer(Matrix3), SplitTone(Shadows, Highlights),
    Grain(GrainParams), Vignette(VignetteParams), Clarity(f32), Bloom(BloomParams),
    Lut3D(LutHandle),
}
```

---

## Sub-phases

### 07.1 — Per-pixel effects
- Curves (RGB + per-channel), channel mixer, black-and-white mix, duotone/split-tone, color balance, vibrance/saturation refinements.

### 07.2 — Neighborhood effects
- Gaussian/box blur (separable), unsharp mask / clarity, structure, bloom/glow. Do these in linear light.

### 07.3 — Film/texture
- Film grain (luminance-aware, size/roughness controls), vignette (exposure + midpoint + roundness).

### 07.4 — 3D LUT system
- Load `.cube` LUTs into a 3D texture; apply as a pass. Ship a few built-in looks; allow user import.

### 07.5 — Effect presets
- Bundle effect stacks into named looks (ties into Phase 01 presets). Export/import.

---

## Evaluate against your editor / RapidRAW
- Run `photon`'s catalog as a checklist against your feature set — list the gaps.
- Do you support **3D LUT import**? It's a big compatibility/creative win and a common gap.
- Are neighborhood effects done in linear light? Sharpening/blur in gamma space looks wrong.

## Testing & acceptance criteria
- [ ] Each effect is non-destructive and reversible via the EditStack.
- [ ] Blur/sharpen operate in linear light (verify no gamma halos).
- [ ] A `.cube` LUT imported from a third-party pack applies identically to a reference renderer.
- [ ] Grain looks filmic (luminance-aware), not uniform noise, at 100%.
- [ ] Effect presets round-trip (export → import → identical result).

## Risks / gotchas
- Effect **order** matters (grain before vs after tone changes the look) — make ordering explicit.
- Performance: many passes add up; fuse where possible or cache intermediate textures.
- LUT interpolation (trilinear) quality — verify against reference to avoid banding.
