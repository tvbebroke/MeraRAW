// HSL / color mixer (slot 6) — 8 overlapping hue bands in Oklab LCh.
// Smooth gaussian band weights (no banding), chroma-gated so neutrals
// don't pick up hue shifts. Orange = the skin band (spec 3.11): slightly
// narrower so skin moves stay surgical.

struct HslUniforms {
  k0: vec4<f32>, k1: vec4<f32>, k2: vec4<f32>,
  ki0: vec4<f32>, ki1: vec4<f32>, ki2: vec4<f32>,
  m0: vec4<f32>, m1: vec4<f32>, m2r: vec4<f32>,
  mi0: vec4<f32>, mi1: vec4<f32>, mi2: vec4<f32>,
  // 8 bands: x = hue shift [-100..100], y = sat, z = lum, w = unused
  bands: array<vec4<f32>, 8>,
  width: u32,
  height: u32,
  _p0: u32,
  _p1: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> u: HslUniforms;

// band centers in Oklab hue degrees (red orange yellow green aqua blue purple magenta)
const CENTERS = array<f32, 8>(30.0, 60.0, 95.0, 140.0, 195.0, 260.0, 305.0, 345.0);
const SIGMAS  = array<f32, 8>(25.0, 20.0, 25.0, 28.0, 28.0, 28.0, 25.0, 25.0);

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

fn gamut_compress(rgb: vec3<f32>) -> vec3<f32> {
  let luma = dot(max(rgb, vec3<f32>(0.0)), LUMA_W);
  let m = min(rgb.r, min(rgb.g, rgb.b));
  if (m >= 0.0 || luma <= 0.0) {
    return max(rgb, vec3<f32>(0.0));
  }
  let t = clamp(-m / max(luma - m, 1e-6), 0.0, 1.0);
  return max(mix(rgb, vec3<f32>(luma), t), vec3<f32>(0.0));
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let p = textureLoad(src, vec2<i32>(gid.xy), 0);
  let lab = to_oklab(max(p.rgb, vec3<f32>(0.0)));
  var l = lab.x;
  var c = sqrt(lab.y * lab.y + lab.z * lab.z);
  var h = degrees(atan2(lab.z, lab.y));
  if (h < 0.0) {
    h += 360.0;
  }

  // neutrals don't get hue-band edits
  let gate = smoothstep(0.01, 0.06, c);

  for (var i = 0; i < 8; i++) {
    let band = u.bands[i];
    var d = abs(h - CENTERS[i]);
    d = min(d, 360.0 - d); // wrap
    let w = exp(-(d * d) / (2.0 * SIGMAS[i] * SIGMAS[i])) * gate;
    if (w < 1e-4) {
      continue;
    }
    h += w * band.x * 0.20;          // hue ±100 → ±20°
    c *= 1.0 + w * band.y * 0.01;    // sat ±100 → ±100%
    l *= 1.0 + w * band.z * 0.006;   // lum ±100 → ±60%
  }

  let hr = radians(h);
  let outc = gamut_compress(from_oklab(vec3<f32>(max(l, 0.0), max(c, 0.0) * cos(hr), max(c, 0.0) * sin(hr))));
  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(outc, p.a));
}
