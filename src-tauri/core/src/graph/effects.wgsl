// Late creative effects (after sharpen): atmospheric dehaze, midtone clarity,
// radial vignette, luminance-aware film grain. Identity when all amounts are 0.
// Dehaze math must stay in lockstep with `crate::dehaze`.

struct EffectsUniforms {
  clarity: f32,          // 0..1 effective midtone highpass gain
  grain_amount: f32,     // 0..1
  grain_size: f32,       // grain cell size in px (≈1..8)
  vignette_amount: f32,  // 0..1 exposure darkening at corners
  vignette_midpoint: f32,// 0..1 — where falloff starts (higher = tighter)
  dehaze: f32,           // −1..1 Koschmieder amount (pos = remove, neg = add)
  _pad0: f32,
  _pad1: f32,
  width: u32,
  height: u32,
  frame_index: u32,
  _pad2: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> u: EffectsUniforms;

const LUMA_W = vec3<f32>(0.2627, 0.6780, 0.0593);

fn hash21(p: vec2<f32>) -> f32 {
  var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.x + p3.y) * p3.z);
}

fn min3(v: vec3<f32>) -> f32 {
  return min(v.r, min(v.g, v.b));
}

fn max3(v: vec3<f32>) -> f32 {
  return max(v.r, max(v.g, v.b));
}

fn neutralize_airlight(A: vec3<f32>) -> vec3<f32> {
  let y = dot(A, LUMA_W);
  return max(mix(A, vec3<f32>(y), 0.35), vec3<f32>(0.08));
}

fn apply_dehaze(rgb: vec3<f32>, dark: f32, atmos: vec3<f32>, amount: f32) -> vec3<f32> {
  if (abs(amount) < 1e-6) {
    return rgb;
  }
  let a = neutralize_airlight(atmos);
  let omega = abs(amount) * 0.92;
  let a_min = max(min3(a), 0.08);
  let t_raw = 1.0 - omega * dark / a_min;
  let t0 = 0.18 + (0.10 - 0.18) * abs(amount);
  let t = clamp(t_raw, t0, 1.0);
  if (amount > 0.0) {
    var out = max((rgb - a) / t + a, vec3<f32>(0.0));
    let y = dot(out, LUMA_W);
    let sat = 1.0 + 0.20 * amount * (1.0 - t);
    return max(mix(vec3<f32>(y), out, sat), vec3<f32>(0.0));
  }
  let t_haze = t * 0.40 + (1.0 - 0.48 * omega) * 0.60;
  return max(rgb * t_haze + a * (1.0 - t_haze), vec3<f32>(0.0));
}

fn sample_rgb(coord: vec2<i32>) -> vec3<f32> {
  let q = clamp(
    coord,
    vec2<i32>(0, 0),
    vec2<i32>(i32(u.width) - 1, i32(u.height) - 1),
  );
  return max(textureLoad(src, q, 0).rgb, vec3<f32>(0.0));
}

// Dark-channel prior + local airlight in a 9×9 window (He / Darktable-style).
fn estimate_dark_and_airlight(coord: vec2<i32>) -> vec4<f32> {
  var dc = 1e9;
  var a_acc = vec3<f32>(0.0);
  var a_w = 0.0;
  var bright_max = vec3<f32>(0.0);
  var bright_dc = -1.0;
  for (var dy = -4; dy <= 4; dy++) {
    for (var dx = -4; dx <= 4; dx++) {
      let s = sample_rgb(coord + vec2<i32>(dx, dy));
      let mn = min3(s);
      let mx = max3(s);
      dc = min(dc, mn);
      let chroma = mx - mn;
      let w = mx * (1.0 - smoothstep(0.04, 0.28, chroma));
      a_acc += s * w;
      a_w += w;
      if (mn > bright_dc) {
        bright_dc = mn;
        bright_max = s;
      }
    }
  }
  var A = bright_max;
  if (a_w > 1e-4) {
    A = a_acc / a_w;
  }
  return vec4<f32>(A, dc);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let coord = vec2<i32>(gid.xy);
  var rgb = textureLoad(src, coord, 0).rgb;
  let a = textureLoad(src, coord, 0).a;

  // ---- dehaze: invert / apply Koschmieder veil (not midtone unsharp) ----
  if (abs(u.dehaze) > 0.0) {
    let est = estimate_dark_and_airlight(coord);
    rgb = apply_dehaze(rgb, est.w, est.xyz, u.dehaze);
  }

  // ---- clarity: midtone-weighted unsharp on luma (linear) ----
  if (u.clarity > 0.0) {
    var acc = 0.0;
    var wsum = 0.0;
    let sigma = 2.5;
    let inv2s = 1.0 / (2.0 * sigma * sigma);
    for (var dy = -3; dy <= 3; dy++) {
      for (var dx = -3; dx <= 3; dx++) {
        let q = clamp(
          coord + vec2<i32>(dx, dy),
          vec2<i32>(0, 0),
          vec2<i32>(i32(u.width) - 1, i32(u.height) - 1),
        );
        let s = textureLoad(src, q, 0).rgb;
        let w = exp(-f32(dx * dx + dy * dy) * inv2s);
        acc += dot(max(s, vec3<f32>(0.0)), LUMA_W) * w;
        wsum += w;
      }
    }
    let c_luma = dot(max(rgb, vec3<f32>(0.0)), LUMA_W);
    let blur = acc / max(wsum, 1e-6);
    let high = c_luma - blur;
    // Midtone weight: peak around 0.2 linear, fade in deep shadows / highlights.
    let mid = smoothstep(0.02, 0.12, c_luma) * (1.0 - smoothstep(0.6, 1.5, c_luma));
    let gain = 1.0 + u.clarity * high * mid / max(c_luma, 1e-4);
    rgb = max(rgb * gain, vec3<f32>(0.0));
  }

  // ---- vignette: radial exposure multiply (linear) ----
  if (u.vignette_amount > 0.0) {
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(f32(u.width), f32(u.height));
    let d = length((uv - vec2<f32>(0.5)) * vec2<f32>(1.0, f32(u.height) / max(f32(u.width), 1.0)));
    let start = clamp(u.vignette_midpoint, 0.05, 0.95);
    let t = smoothstep(start, 1.35, d * 1.414);
    let darken = 1.0 - u.vignette_amount * t;
    rgb *= max(darken, 0.0);
  }

  // ---- grain: luminance-aware, size-controlled ----
  if (u.grain_amount > 0.0) {
    let cell = max(u.grain_size, 1.0);
    let gp = floor(vec2<f32>(gid.xy) / cell)
      + vec2<f32>(f32(u.frame_index) * 19.0, f32(u.frame_index) * 7.0);
    let n = hash21(gp) * 2.0 - 1.0;
    let luma = dot(max(rgb, vec3<f32>(0.0)), LUMA_W);
    // More grain in midtones; less in deep shadows / speculars.
    let luma_w = smoothstep(0.01, 0.08, luma) * (1.0 - smoothstep(1.2, 3.0, luma));
    let amp = u.grain_amount * 0.08 * luma_w * (0.35 + 0.65 * sqrt(max(luma, 0.0)));
    rgb = max(rgb + n * amp, vec3<f32>(0.0));
  }

  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(rgb, a));
}
