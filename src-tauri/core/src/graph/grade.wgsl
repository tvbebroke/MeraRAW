// Color grade (slot 5) — 3-way wheels + global chroma + perceptual sat,
// colorbalancergb-structured, in Oklab (constant-hue chroma moves), with
// constant-hue gamut compression on output. Matrices supplied CPU-side
// (computed from pinned constants, never hand-keyed in WGSL).

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
  _p0: u32,
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

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let p = textureLoad(src, vec2<i32>(gid.xy), 0);
  var lab = to_oklab(max(p.rgb, vec3<f32>(0.0)));
  var l = lab.x;
  var a = lab.y;
  var b = lab.z;

  // zone weights on clamped lightness
  let t = clamp(l, 0.0, 1.0);
  let w_sh = 1.0 - smoothstep(0.0, max(u.ranges.x, 0.01) * 2.0, t);
  let w_hi = smoothstep(u.ranges.y, 1.0, t);
  let w_mid = max(1.0 - w_sh - w_hi, 0.0);

  // per-zone chroma push toward the wheel hue + lum scale.
  // strength scaled by lightness so deep blacks don't blow up.
  let prot = mix(0.25, 1.0, t);
  var zones = array<vec4<f32>, 3>(u.shadows, u.midtones, u.highlights);
  var weights = array<f32, 3>(w_sh, w_mid, w_hi);
  for (var i = 0; i < 3; i++) {
    let z = zones[i];
    let w = weights[i];
    a += w * z.y * 0.002 * cos(z.x) * prot;
    b += w * z.y * 0.002 * sin(z.x) * prot;
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

  let outc = gamut_compress(from_oklab(vec3<f32>(max(l, 0.0), a, b)));
  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(outc, p.a));
}
