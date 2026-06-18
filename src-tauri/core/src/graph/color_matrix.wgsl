// Generic per-pixel color transform pass: out = M * rgb * gain.
// One shader, two P2 module instances:
//   exposure      — M = I, gain = 2^stops      (slot 1, linear multiply)
//   white_balance — M = Bradford-in-Rec2020    (slot 2, chromatic adaptation)
// Negative results clamped to 0 (out-of-gamut after adaptation).

struct PassUniforms {
  m0: vec4<f32>,   // matrix rows, .xyz used
  m1: vec4<f32>,
  m2: vec4<f32>,
  gain: f32,
  width: u32,
  height: u32,
  _pad: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> u: PassUniforms;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let p = textureLoad(src, vec2<i32>(gid.xy), 0);
  let rgb = p.rgb * u.gain;
  let out = vec3<f32>(
    dot(u.m0.xyz, rgb),
    dot(u.m1.xyz, rgb),
    dot(u.m2.xyz, rgb),
  );
  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(max(out, vec3<f32>(0.0)), p.a));
}
