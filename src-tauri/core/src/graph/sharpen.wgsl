// Sharpen (slot 8, AFTER tone shaping): unsharp mask on luma only —
// chroma untouched (no edge color fringing), detail param gates
// low-contrast response so flat areas stay clean.

struct SharpenUniforms {
  amount: f32,     // 0..3 effective gain
  sigma: f32,      // blur sigma in px (radius param)
  threshold: f32,  // |highpass| gate from sharpen_detail
  _pad: f32,
  width: u32,
  height: u32,
  _p0: u32,
  _p1: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> u: SharpenUniforms;

const LUMA_W = vec3<f32>(0.2627, 0.6780, 0.0593);

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let coord = vec2<i32>(gid.xy);
  let center = textureLoad(src, coord, 0);
  let c_luma = dot(max(center.rgb, vec3<f32>(0.0)), LUMA_W);

  var acc = 0.0;
  var wsum = 0.0;
  let inv2s = 1.0 / max(2.0 * u.sigma * u.sigma, 1e-6);
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
  let blur = acc / max(wsum, 1e-6);
  var high = c_luma - blur;
  // soft threshold: fade response below the gate (relative to local level)
  let gate = smoothstep(0.0, u.threshold * max(c_luma, 0.02), abs(high));
  high *= gate;
  let gain = 1.0 + u.amount * high / max(c_luma, 1e-4);
  let outc = max(center.rgb * gain, vec3<f32>(0.0));
  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(outc, center.a));
}
