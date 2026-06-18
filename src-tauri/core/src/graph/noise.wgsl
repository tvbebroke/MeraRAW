// Detail / noise reduction (slot 4, BEFORE sharpen — never sharpen noise).
// Luma: 5×5 bilateral (range sigma from strength, gated by detail_preserve).
// Chroma: 5×5 gaussian smoothing of the chroma offsets.
// Identity when both strengths are 0 (pass skipped by the graph).

struct NoiseUniforms {
  luma_sigma: f32,    // bilateral range sigma (0 = off)
  luma_amt: f32,      // blend 0..1
  chroma_amt: f32,    // blend 0..1
  _pad: f32,
  width: u32,
  height: u32,
  _p0: u32,
  _p1: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> u: NoiseUniforms;

const LUMA_W = vec3<f32>(0.2627, 0.6780, 0.0593);

fn luma_of(c: vec3<f32>) -> f32 {
  return dot(max(c, vec3<f32>(0.0)), LUMA_W);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let coord = vec2<i32>(gid.xy);
  let center = textureLoad(src, coord, 0);
  let c_luma = luma_of(center.rgb);
  let c_chroma = center.rgb - vec3<f32>(c_luma);

  var luma_acc = 0.0;
  var luma_wsum = 0.0;
  var chroma_acc = vec3<f32>(0.0);
  var chroma_wsum = 0.0;

  let inv2sr = 1.0 / max(2.0 * u.luma_sigma * u.luma_sigma, 1e-6);
  for (var dy = -2; dy <= 2; dy++) {
    for (var dx = -2; dx <= 2; dx++) {
      let q = clamp(
        coord + vec2<i32>(dx, dy),
        vec2<i32>(0, 0),
        vec2<i32>(i32(u.width) - 1, i32(u.height) - 1),
      );
      let s = textureLoad(src, q, 0).rgb;
      let s_luma = luma_of(s);
      let dist2 = f32(dx * dx + dy * dy);
      let w_sp = exp(-dist2 * 0.18); // spatial gaussian, sigma≈1.7px
      // bilateral on luma (relative difference → exposure-invariant)
      let rel = (s_luma - c_luma) / max(c_luma + 0.01, 0.02);
      let w_range = exp(-rel * rel * inv2sr);
      luma_acc += s_luma * w_sp * w_range;
      luma_wsum += w_sp * w_range;
      // chroma: plain gaussian
      chroma_acc += (s - vec3<f32>(s_luma)) * w_sp;
      chroma_wsum += w_sp;
    }
  }
  let luma_f = mix(c_luma, luma_acc / max(luma_wsum, 1e-6), u.luma_amt);
  let chroma_f = mix(c_chroma, chroma_acc / max(chroma_wsum, 1e-6), u.chroma_amt);
  let outc = max(vec3<f32>(luma_f) + chroma_f, vec3<f32>(0.0));
  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(outc, center.a));
}
