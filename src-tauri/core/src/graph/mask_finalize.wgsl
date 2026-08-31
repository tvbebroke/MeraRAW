// Apply mask-level invert + opacity after composite combine.

struct MaskFinalizeUniforms {
  width: u32,
  height: u32,
  opacity: f32,
  invert: u32,
  _pad: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var<uniform> u: MaskFinalizeUniforms;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let p = vec2<i32>(i32(gid.x), i32(gid.y));
  var m = textureLoad(src, p, 0).r;
  if (u.invert == 1u) {
    m = 1.0 - m;
  }
  m = m * u.opacity;
  textureStore(dst, p, vec4<f32>(m, 0.0, 0.0, 0.0));
}
