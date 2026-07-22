// Late creative effects (after sharpen): midtone clarity, radial vignette,
// luminance-aware film grain. Identity when all amounts are 0.

struct EffectsUniforms {
  clarity: f32,          // 0..1 effective midtone highpass gain
  grain_amount: f32,     // 0..1
  grain_size: f32,       // grain cell size in px (≈1..8)
  vignette_amount: f32,  // 0..1 exposure darkening at corners
  vignette_midpoint: f32,// 0..1 — where falloff starts (higher = tighter)
  width: u32,
  height: u32,
  _pad: u32,
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

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let coord = vec2<i32>(gid.xy);
  var rgb = textureLoad(src, coord, 0).rgb;
  let a = textureLoad(src, coord, 0).a;

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
    let gp = floor(vec2<f32>(gid.xy) / cell);
    let n = hash21(gp) * 2.0 - 1.0;
    let luma = dot(max(rgb, vec3<f32>(0.0)), LUMA_W);
    // More grain in midtones; less in deep shadows / speculars.
    let luma_w = smoothstep(0.01, 0.08, luma) * (1.0 - smoothstep(1.2, 3.0, luma));
    let amp = u.grain_amount * 0.08 * luma_w * (0.35 + 0.65 * sqrt(max(luma, 0.0)));
    rgb = max(rgb + n * amp, vec3<f32>(0.0));
  }

  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(rgb, a));
}
