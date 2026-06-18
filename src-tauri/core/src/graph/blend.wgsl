// Mask composite (spec 4.5 step 3): out = mix(base, local, mask).
// base = image before this mask's stack; local = after the mask's scoped
// module stack; mask already carries opacity/invert/feather.

struct BlendUniforms {
  width: u32,
  height: u32,
  _p0: u32,
  _p1: u32,
};

@group(0) @binding(0) var base: texture_2d<f32>;
@group(0) @binding(1) var local_t: texture_2d<f32>;
@group(0) @binding(2) var mask_t: texture_2d<f32>;
@group(0) @binding(3) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var<uniform> u: BlendUniforms;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let c = vec2<i32>(gid.xy);
  let b = textureLoad(base, c, 0);
  let l = textureLoad(local_t, c, 0);
  let m = textureLoad(mask_t, c, 0).r;
  textureStore(dst, c, vec4<f32>(mix(b.rgb, l.rgb, clamp(m, 0.0, 1.0)), b.a));
}
