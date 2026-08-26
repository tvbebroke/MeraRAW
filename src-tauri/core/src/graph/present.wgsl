// Terminal display-transform node (contract F2 screen side), 1:1 over the
// chain output: gamut Rec2020→sRGB → pinned neutral view transform
// (Reinhard-extended, Lw == gain) → clamp → sRGB OETF. alpha 0 → letterbox bg.
// Constants mirror gpu/display.wgsl; golden tests pin them.

struct PresentUniforms {
  width: u32,
  height: u32,
  overlay: f32, // 0 = off; else mask-overlay tint strength
  look: u32,    // 0 = Neutral, 1 = Camera (punchy)
  clip_hi: u32, // 1 = highlight blinkies on
  clip_lo: u32, // 1 = shadow blinkies on
  _p0: u32,     // millis for blink pulse
  _p1: u32,     // bits 0-3 proof space (0 = off), bit 4 = gamut check
  m0: vec4<f32>,
  m1: vec4<f32>,
  m2: vec4<f32>,
  n0: vec4<f32>,
  n1: vec4<f32>,
  n2: vec4<f32>,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> u: PresentUniforms;
@group(0) @binding(3) var overlay_mask: texture_2d<f32>;

const REC2020_TO_SRGB = mat3x3<f32>(
  vec3<f32>( 1.6605, -0.1246, -0.0182),
  vec3<f32>(-0.5876,  1.1329, -0.1006),
  vec3<f32>(-0.0728, -0.0083,  1.1187),
);

const LUMA = vec3<f32>(0.2126, 0.7152, 0.0722);

// Scene luminance where the Camera look starts crossfading from the
// luminance-mapped result to the per-channel one. See view_look.
const HL_PC_LO: f32 = 0.5;

// Reinhard-extended shoulder + a gentle S-curve. The shoulder's white point is
// the gain itself, which puts display white at scene luminance 1.0 — sensor
// saturation, the brightest neutral a RAW can hold. The old fixed 4.0 placed it
// near 3.5, reserving most of a stop and a half of display range for headroom
// that RAW data does not contain, so every image rendered flat and diffuse
// white never reached white.
fn tone_curve(v: f32, g: f32, contrast: f32) -> f32 {
  let x = v * g;
  let r = x * (1.0 + x / (g * g)) / (1.0 + x);
  let s = 0.5 - 0.5 * cos(clamp(r, 0.0, 1.0) * 3.14159265);
  return clamp(mix(r, s, contrast), 0.0, 1.0);
}

// shared look operator — MIRRORED in export.rs::view_look (WYSIWYG).
fn view_look(c: vec3<f32>, look: u32) -> vec3<f32> {
  var gain = 1.15;     // Neutral: a touch of lift so the base isn't dark
  var contrast = 0.12;
  var sat = 1.0;
  var baseline_ev = 0.0; // Neutral stays colorimetric — no baseline lift
  var hl_pc = 0.0;
  if (look == 1u) {    // Camera fallback when no DCP is loaded
    gain = 1.6;
    contrast = 0.62;
    sat = 1.22;
    baseline_ev = 0.75;
    hl_pc = 1.0;
  }
  // Baseline exposure rides on the gain. Because the white point tracks the
  // gain, scene luminance 1.0 still lands exactly on display white for any
  // lift: this opens the midtones without burning highlights. Cameras expose
  // RAW to protect the highlights, so a purely colorimetric render sits about
  // a stop under what every other converter shows — the same correction Adobe
  // ships as the DCP BaselineExposure tag, which we only get when a profile is
  // loaded. The contrast bump keeps the lifted blacks off the floor.
  let g = gain * exp2(baseline_ev);
  let cc = max(c, vec3<f32>(0.0));
  let l = dot(cc, LUMA);
  if (l <= 1e-8) {
    return vec3<f32>(0.0);
  }
  let ld = tone_curve(l, g, contrast);
  var outc = cc * (ld / l);
  let l2 = dot(max(outc, vec3<f32>(0.0)), LUMA);
  outc = max(mix(vec3<f32>(l2), outc, sat), vec3<f32>(0.0));
  // Mapping luminance alone holds the scene's chromaticity all the way up, so
  // a coloured illuminant keeps its cast at display white and bright neutrals
  // never go neutral. Crossfade into a per-channel curve near white: each
  // channel then rolls into its own shoulder and converges, which is what
  // makes highlights desaturate. Gated on luminance rather than applied
  // throughout so saturated midtones keep their chroma.
  if (hl_pc > 0.0) {
    let pc = vec3<f32>(tone_curve(cc.r, g, contrast),
                       tone_curve(cc.g, g, contrast),
                       tone_curve(cc.b, g, contrast));
    outc = mix(outc, pc, smoothstep(HL_PC_LO, 1.0, ld) * hl_pc);
  }
  // A luminance-only shoulder can still leave a single channel above display
  // white. Fade such a pixel toward its own luminance so the final clamp cuts
  // brightness rather than hue.
  let mx = max(outc.r, max(outc.g, outc.b));
  if (mx > 1.0) {
    let l3 = dot(outc, LUMA);
    outc = mix(outc, vec3<f32>(l3), clamp((mx - 1.0) / mx, 0.0, 1.0));
  }
  return max(outc, vec3<f32>(0.0));
}

fn oetf_srgb(c: vec3<f32>) -> vec3<f32> {
  let lo = c * 12.92;
  let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
  return select(hi, lo, c <= vec3<f32>(0.0031308));
}

// ---- AgX filmic display transform (look == 2) ----
// "Minimal AgX" (Benjamin Wrensch / iolite-engine.com), the implementation
// Blender ships. Input: linear sRGB (Rec.709 primaries). Output: sRGB
// display-encoded — graceful highlight desaturation + clean out-of-gamut,
// unlike the luminance-only Reinhard above. Matrices are the published AgX
// inset/outset (column-major here for `M * v`); min/max log2 exposure pinned.
const AGX_INSET = mat3x3<f32>(
  vec3<f32>(0.8424790622, 0.0423282423, 0.0423756549),
  vec3<f32>(0.0784336000, 0.8784686365, 0.0784336000),
  vec3<f32>(0.0792237451, 0.0791661275, 0.8791429738),
);
const AGX_OUTSET = mat3x3<f32>(
  vec3<f32>( 1.1968790051, -0.0528968518, -0.0529716355),
  vec3<f32>(-0.0980208811,  1.1519031299, -0.0980434501),
  vec3<f32>(-0.0990297441, -0.0989611768,  1.1510736726),
);
const AGX_MIN_EV: f32 = -12.47393;
const AGX_MAX_EV: f32 = 4.026069;

// 6th-order polynomial approximation of the AgX log-encoded contrast sigmoid.
fn agx_contrast(x: vec3<f32>) -> vec3<f32> {
  let x2 = x * x;
  let x4 = x2 * x2;
  return 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4
       - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x - 0.00232;
}

fn agx(srgb_lin: vec3<f32>) -> vec3<f32> {
  var v = AGX_INSET * srgb_lin;
  v = clamp((log2(max(v, vec3<f32>(1e-10))) - AGX_MIN_EV) / (AGX_MAX_EV - AGX_MIN_EV),
            vec3<f32>(0.0), vec3<f32>(1.0));
  v = agx_contrast(v);
  v = AGX_OUTSET * v;
  return clamp(v, vec3<f32>(0.0), vec3<f32>(1.0)); // already display-encoded
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let p = textureLoad(src, vec2<i32>(gid.xy), 0);
  var encoded = vec3<f32>(0.0863, 0.0863, 0.0941); // app bg
  if (p.a > 0.0) {
    var c: vec3<f32>;
    var paint_gamut = false;
    let space = u._p1 & 7u;
    if (space == 0u) {
      c = REC2020_TO_SRGB * max(p.rgb, vec3<f32>(0.0));
    } else {
      let M = mat3x3<f32>(u.m0.xyz, u.m1.xyz, u.m2.xyz);
      let N = mat3x3<f32>(u.n0.xyz, u.n1.xyz, u.n2.xyz);
      let t = M * p.rgb;
      if ((u._p1 & 16u) != 0u && (t.x < 0.0 || t.y < 0.0 || t.z < 0.0)) {
        paint_gamut = true;
      }
      c = N * max(t, vec3<f32>(0.0));
    }
    if (paint_gamut) {
      encoded = vec3<f32>(1.0, 0.0, 1.0);
    } else if (u.look == 2u) {
      // AgX already outputs display-encoded sRGB — no second OETF.
      encoded = agx(max(c, vec3<f32>(0.0)));
    } else if (u.look == 3u || u.look == 4u) {
      // 3 = raster zero-edit (JPEG/PNG already display-referred).
      // 4 = Original RAW (demosaic only). Rec.2020→sRGB + OETF, no view look.
      encoded = oetf_srgb(clamp(max(c, vec3<f32>(0.0)), vec3<f32>(0.0), vec3<f32>(1.0)));
    } else {
      let looked = view_look(max(c, vec3<f32>(0.0)), u.look);
      encoded = oetf_srgb(clamp(looked, vec3<f32>(0.0), vec3<f32>(1.0)));
    }
    if (u.overlay > 0.0) {
      let m = textureLoad(overlay_mask, vec2<i32>(gid.xy), 0).r;
      encoded = mix(encoded, vec3<f32>(1.0, 0.15, 0.15), clamp(m, 0.0, 1.0) * u.overlay);
    }
    // Clipping blinkies on display-encoded output (matches histogram clip %).
    // Pulse via _p0 = millis so warnings flash while frames keep updating.
    if (!paint_gamut) {
      let pulse = 0.55 + 0.45 * abs(sin(f32(u._p0) * 0.012566)); // ~2 Hz
      if (u.clip_hi != 0u) {
        let hi = encoded.r >= 0.995 || encoded.g >= 0.995 || encoded.b >= 0.995;
        if (hi) {
          encoded = mix(encoded, vec3<f32>(1.0, 0.05, 0.05), pulse);
        }
      }
      if (u.clip_lo != 0u) {
        let lo = encoded.r <= 0.004 && encoded.g <= 0.004 && encoded.b <= 0.004;
        if (lo) {
          encoded = mix(encoded, vec3<f32>(0.15, 0.45, 1.0), pulse);
        }
      }
    }
  }
  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(encoded, 1.0));
}
