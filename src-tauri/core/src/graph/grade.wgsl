// Color grade (slot 5) — 3-way wheels + global chroma + perceptual sat.
// The wheels can inject color three different ways, selected by u.model:
//   0 Perceptual — Oklab constant-hue chroma moves (clean, no hue crosstalk).
//   1 Classic    — additive colored offsets in linear RGB (lift/gain wheels;
//                  hue crosstalk = punchy, "Resolve-style" look).
//   2 Light      — von Kries multiply in LMS cone space (tints behave like
//                  colored illumination; filmic, physically grounded).
// All three are identity at zero wheels and share the same global chroma +
// perceptual-saturation + gamut-compress finish. Matrices supplied CPU-side.

struct GradeUniforms {
  // Rec.2020 → LMS rows
  k0: vec4<f32>, k1: vec4<f32>, k2: vec4<f32>,
  // LMS → Rec.2020 rows
  ki0: vec4<f32>, ki1: vec4<f32>, ki2: vec4<f32>,
  // LMS' → Lab rows
  m0: vec4<f32>, m1: vec4<f32>, m2r: vec4<f32>,
  // Lab → LMS' rows
  mi0: vec4<f32>, mi1: vec4<f32>, mi2: vec4<f32>,
  // per zone: x = hue (radians), y = sat strength, z = lum, w = unused
  shadows: vec4<f32>,
  midtones: vec4<f32>,
  highlights: vec4<f32>,
  // x = shadow_range, y = highlight_range, z = global_chroma, w = perceptual_sat
  ranges: vec4<f32>,
  width: u32,
  height: u32,
  model: u32,
  _p1: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> u: GradeUniforms;

fn cbrt_s(v: f32) -> f32 {
  return sign(v) * pow(abs(v), 1.0 / 3.0);
}

fn to_oklab(rgb: vec3<f32>) -> vec3<f32> {
  let lms = vec3<f32>(dot(u.k0.xyz, rgb), dot(u.k1.xyz, rgb), dot(u.k2.xyz, rgb));
  let lp = vec3<f32>(cbrt_s(lms.x), cbrt_s(lms.y), cbrt_s(lms.z));
  return vec3<f32>(dot(u.m0.xyz, lp), dot(u.m1.xyz, lp), dot(u.m2r.xyz, lp));
}

fn from_oklab(lab: vec3<f32>) -> vec3<f32> {
  let lp = vec3<f32>(dot(u.mi0.xyz, lab), dot(u.mi1.xyz, lab), dot(u.mi2.xyz, lab));
  let lms = lp * lp * lp;
  return vec3<f32>(dot(u.ki0.xyz, lms), dot(u.ki1.xyz, lms), dot(u.ki2.xyz, lms));
}

const LUMA_W = vec3<f32>(0.2627, 0.6780, 0.0593);

// soft chroma clip back into Rec.2020 at constant hue + luma
fn gamut_compress(rgb: vec3<f32>) -> vec3<f32> {
  let luma = dot(max(rgb, vec3<f32>(0.0)), LUMA_W);
  let m = min(rgb.r, min(rgb.g, rgb.b));
  if (m >= 0.0 || luma <= 0.0) {
    return max(rgb, vec3<f32>(0.0));
  }
  // smallest t in [0,1] with mix(rgb, luma, t) >= 0 for all channels
  let t = clamp(-m / max(luma - m, 1e-6), 0.0, 1.0);
  return max(mix(rgb, vec3<f32>(luma), t), vec3<f32>(0.0));
}

const TWO_PI = 6.28318530718;

// Fully-saturated RGB for a hue angle (radians). Used as a tint direction by
// the Classic and Light models.
fn hue_rgb(h_rad: f32) -> vec3<f32> {
  let hh = fract(h_rad / TWO_PI) * 6.0;
  let r = clamp(abs(hh - 3.0) - 1.0, 0.0, 1.0);
  let g = clamp(2.0 - abs(hh - 2.0), 0.0, 1.0);
  let b = clamp(2.0 - abs(hh - 4.0), 0.0, 1.0);
  return vec3<f32>(r, g, b);
}

// shadow / midtone / highlight weights from Oklab lightness.
fn zone_weights(l: f32) -> vec3<f32> {
  let t = clamp(l, 0.0, 1.0);
  let w_sh = 1.0 - smoothstep(0.0, max(u.ranges.x, 0.01) * 2.0, t);
  let w_hi = smoothstep(u.ranges.y, 1.0, t);
  let w_mid = max(1.0 - w_sh - w_hi, 0.0);
  return vec3<f32>(w_sh, w_mid, w_hi);
}

// Shared finish for the Classic/Light models: global chroma + perceptual
// saturation (skin-guarded, constant-hue in Oklab) + gamut compression.
// Mirrors the Perceptual model's chroma/sat block exactly.
fn finish_chroma_sat(rgb: vec3<f32>) -> vec3<f32> {
  let lab = to_oklab(max(rgb, vec3<f32>(0.0)));
  let l = lab.x;
  var a = lab.y;
  var b = lab.z;
  var c = sqrt(a * a + b * b);
  let h_cos = select(a / c, 1.0, c < 1e-6);
  let h_sin = select(b / c, 0.0, c < 1e-6);
  c *= 1.0 + u.ranges.z * 0.01;
  let hue = atan2(b, a);
  let skin_guard = mix(0.6, 1.0, smoothstep(0.0, 0.6, abs(hue - 1.0)));
  let percep = 1.0 + u.ranges.w * 0.01 * (1.0 - min(c / 0.25, 1.0)) * skin_guard;
  c = max(c * percep, 0.0);
  a = c * h_cos;
  b = c * h_sin;
  return gamut_compress(from_oklab(vec3<f32>(max(l, 0.0), a, b)));
}

// Model 1 — Classic: additive colored offsets in linear RGB per zone. The
// offset is centered on gray (hue_rgb - 0.5) so channels cross-talk, giving
// the punchy hue-shifting feel of lift/gamma/gain color wheels.
fn grade_classic(rgb0: vec3<f32>, w: vec3<f32>) -> vec3<f32> {
  var rgb = rgb0;
  var zones = array<vec4<f32>, 3>(u.shadows, u.midtones, u.highlights);
  for (var i = 0; i < 3; i++) {
    let z = zones[i];
    let wi = w[i];
    let dir = hue_rgb(z.x) - vec3<f32>(0.5);
    rgb += wi * (z.y * 0.01) * 0.15 * dir; // colored offset
    rgb *= 1.0 + wi * z.z * 0.005;         // per-zone luminance gain
  }
  return max(rgb, vec3<f32>(0.0));
}

// Model 2 — Light: von Kries multiply in LMS cone space. Each zone's tint is
// converted to a mean-normalized cone gain and multiplied in, exactly how a
// colored light source would reshape the scene. Filmic / physically grounded.
fn grade_light(rgb0: vec3<f32>, w: vec3<f32>) -> vec3<f32> {
  var lms = vec3<f32>(dot(u.k0.xyz, rgb0), dot(u.k1.xyz, rgb0), dot(u.k2.xyz, rgb0));
  var zones = array<vec4<f32>, 3>(u.shadows, u.midtones, u.highlights);
  for (var i = 0; i < 3; i++) {
    let z = zones[i];
    let wi = w[i];
    let tint = hue_rgb(z.x);
    var tlms = vec3<f32>(dot(u.k0.xyz, tint), dot(u.k1.xyz, tint), dot(u.k2.xyz, tint));
    let mean = max((tlms.x + tlms.y + tlms.z) / 3.0, 1e-4);
    tlms = tlms / mean; // luminance-neutral chromatic gain (ratios around 1)
    let k = wi * (z.y * 0.01) * 0.5;
    lms *= max(mix(vec3<f32>(1.0), tlms, k), vec3<f32>(0.0));
    lms *= 1.0 + wi * z.z * 0.005; // per-zone luminance gain
  }
  return max(
    vec3<f32>(dot(u.ki0.xyz, lms), dot(u.ki1.xyz, lms), dot(u.ki2.xyz, lms)),
    vec3<f32>(0.0),
  );
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let p = textureLoad(src, vec2<i32>(gid.xy), 0);
  let rgb0 = max(p.rgb, vec3<f32>(0.0));
  var outc: vec3<f32>;

  if (u.model == 0u) {
    // ===== Model 0 — Perceptual (Oklab, constant-hue) =====
    var lab = to_oklab(rgb0);
    var l = lab.x;
    var a = lab.y;
    var b = lab.z;

    // zone weights on clamped lightness
    let t = clamp(l, 0.0, 1.0);
    let w_sh = 1.0 - smoothstep(0.0, max(u.ranges.x, 0.01) * 2.0, t);
    let w_hi = smoothstep(u.ranges.y, 1.0, t);
    let w_mid = max(1.0 - w_sh - w_hi, 0.0);

    // per-zone chroma push toward the wheel hue + lum scale.
    // Only guard the deepest blacks (< ~0.08 L) from going neon — shadows,
    // mids and highlights should all take visible color, so the classic
    // teal-shadow / warm-highlight split actually reads.
    let tint_prot = mix(0.6, 1.0, smoothstep(0.0, 0.08, t));
    // Stronger per-1% wheel strength than before so the wheels have real range.
    let TINT = 0.0032;
    var zones = array<vec4<f32>, 3>(u.shadows, u.midtones, u.highlights);
    var weights = array<f32, 3>(w_sh, w_mid, w_hi);
    for (var i = 0; i < 3; i++) {
      let z = zones[i];
      let w = weights[i];
      a += w * z.y * TINT * cos(z.x) * tint_prot;
      b += w * z.y * TINT * sin(z.x) * tint_prot;
      l *= 1.0 + w * z.z * 0.005;
    }

    // global chroma + perceptual saturation (low-chroma boosted more)
    var c = sqrt(a * a + b * b);
    let h_cos = select(a / c, 1.0, c < 1e-6);
    let h_sin = select(b / c, 0.0, c < 1e-6);
    c *= 1.0 + u.ranges.z * 0.01;
    // Skin guard: warm tones (faces) sit ~1.0 rad in Oklab hue. Ease the
    // perceptual-saturation push there so portraits don't go orange/plastic —
    // a vibrance-style skin protect, but constant-hue in Oklab (no hue shift).
    // Soft + conservative (keeps 60% of the push on skin); SKIN_HUE tuned by eye.
    let hue = atan2(b, a);
    let skin_guard = mix(0.6, 1.0, smoothstep(0.0, 0.6, abs(hue - 1.0)));
    let percep = 1.0 + u.ranges.w * 0.01 * (1.0 - min(c / 0.25, 1.0)) * skin_guard;
    c = max(c * percep, 0.0);
    a = c * h_cos;
    b = c * h_sin;

    outc = gamut_compress(from_oklab(vec3<f32>(max(l, 0.0), a, b)));
  } else {
    // ===== Models 1 & 2 — inject color, then shared chroma/sat finish =====
    let w = zone_weights(to_oklab(rgb0).x);
    var graded: vec3<f32>;
    if (u.model == 1u) {
      graded = grade_classic(rgb0, w);
    } else {
      graded = grade_light(rgb0, w);
    }
    outc = finish_chroma_sat(graded);
  }

  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(outc, p.a));
}
