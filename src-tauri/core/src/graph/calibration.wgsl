// Calibration (slot 3): secondary per-primary hue/sat matrix on top of the
// P1 base matrix (LR Calibration-panel equivalent), plus shadow_tint as a
// low-luma chroma offset. Matrix built CPU-side from the params.

struct CalibUniforms {
  m0: vec4<f32>,
  m1: vec4<f32>,
  m2: vec4<f32>,
  // xyz = tint offset (linear rgb), w = unused
  shadow_tint: vec4<f32>,
  width: u32,
  height: u32,
  _p0: u32,
  _p1: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> u: CalibUniforms;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let p = textureLoad(src, vec2<i32>(gid.xy), 0);
  var rgb = vec3<f32>(
    dot(u.m0.xyz, p.rgb),
    dot(u.m1.xyz, p.rgb),
    dot(u.m2.xyz, p.rgb),
  );
  // shadow tint: weighted toward the darkest stops, fades by mid-gray
  let luma = dot(max(rgb, vec3<f32>(0.0)), vec3<f32>(0.2627, 0.6780, 0.0593));
  let w = 1.0 - smoothstep(0.0, 0.25, luma);
  rgb += u.shadow_tint.xyz * w * max(luma, 0.004) * 8.0;
  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(max(rgb, vec3<f32>(0.0)), p.a));
}
