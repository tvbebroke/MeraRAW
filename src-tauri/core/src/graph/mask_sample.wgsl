// Segmentation-mask sampling: small model mask (normalized image space) →
// viewport-res R32Float, joint-bilateral upsampled against the extracted
// image luma so the mask hugs real edges (spec 4.5 edge-refine).

struct MaskSampleUniforms {
  out_w: u32,
  out_h: u32,
  img_w: f32,
  img_h: f32,
  scale: f32,
  center_x: f32,
  center_y: f32,
  feather: f32,
  opacity: f32,
  invert: u32,
  _p0: u32,
  _p1: u32,
};

@group(0) @binding(0) var mask_src: texture_2d<f32>; // small model mask
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var guide: texture_2d<f32>;    // extract output (linear)
@group(0) @binding(3) var dst: texture_storage_2d<r32float, write>;
@group(0) @binding(4) var<uniform> u: MaskSampleUniforms;

const LUMA_W = vec3<f32>(0.2627, 0.6780, 0.0593);

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.out_w || gid.y >= u.out_h) {
    return;
  }
  let out_half = vec2<f32>(f32(u.out_w), f32(u.out_h)) * 0.5;
  let img_px = vec2<f32>(u.center_x * u.img_w, u.center_y * u.img_h)
    + (vec2<f32>(f32(gid.x) + 0.5, f32(gid.y) + 0.5) - out_half) / u.scale;
  let uv = img_px / vec2<f32>(u.img_w, u.img_h);

  var m = 0.0;
  if (uv.x >= 0.0 && uv.x <= 1.0 && uv.y >= 0.0 && uv.y <= 1.0) {
    // joint bilateral: sample the small mask around uv, weight by guide
    // luma similarity at viewport res
    let center = textureLoad(guide, vec2<i32>(gid.xy), 0).rgb;
    let c_luma = dot(max(center, vec3<f32>(0.0)), LUMA_W);
    var acc = 0.0;
    var wsum = 0.0;
    for (var dy = -2; dy <= 2; dy++) {
      for (var dx = -2; dx <= 2; dx++) {
        let o = vec2<i32>(dx, dy);
        let q = clamp(
          vec2<i32>(gid.xy) + o * 2,
          vec2<i32>(0, 0),
          vec2<i32>(i32(u.out_w) - 1, i32(u.out_h) - 1),
        );
        // guide similarity at the neighbor's viewport position
        let s_rgb = textureLoad(guide, q, 0).rgb;
        let s_luma = dot(max(s_rgb, vec3<f32>(0.0)), LUMA_W);
        let rel = (s_luma - c_luma) / max(c_luma + 0.02, 0.04);
        let w_r = exp(-rel * rel * 6.0);
        let w_s = exp(-f32(dx * dx + dy * dy) * 0.15);
        // neighbor offset: out-px → image-px (÷scale) → uv (÷img dims)
        let n_uv = uv
          + vec2<f32>(f32(dx), f32(dy)) * 2.0 / u.scale
            / vec2<f32>(u.img_w, u.img_h);
        let mv = textureSampleLevel(
          mask_src,
          samp,
          clamp(n_uv, vec2<f32>(0.0), vec2<f32>(1.0)),
          0.0,
        ).r;
        acc += mv * w_r * w_s;
        wsum += w_r * w_s;
      }
    }
    m = acc / max(wsum, 1e-6);
    // feather: soften the transition band
    let f = clamp(u.feather, 0.0, 0.9);
    if (f > 0.01) {
      m = smoothstep(0.5 * f, 1.0 - 0.5 * f, m);
    }
  }
  if (u.invert == 1u) {
    m = 1.0 - m;
  }
  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(m * u.opacity, 0.0, 0.0, 0.0));
}
