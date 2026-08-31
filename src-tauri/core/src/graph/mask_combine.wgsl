// Combine two R32Float mask layers (Lightroom-style add / subtract / intersect).

struct MaskCombineUniforms {
  width: u32,
  height: u32,
  // 0 = replace (first component), 1 = add, 2 = subtract, 3 = intersect
  op: u32,
  _pad: u32,
};

@group(0) @binding(0) var acc: texture_2d<f32>;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var dst: texture_storage_2d<r32float, write>;
@group(0) @binding(3) var<uniform> u: MaskCombineUniforms;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let p = vec2<i32>(i32(gid.x), i32(gid.y));
  let a = textureLoad(acc, p, 0).r;
  let s = textureLoad(src, p, 0).r;
  var o = a;
  if (u.op == 0u) {
    o = s;
  } else if (u.op == 1u) {
    o = max(a, s);
  } else if (u.op == 2u) {
    o = min(a, 1.0 - s);
  } else {
    o = min(a, s);
  }
  textureStore(dst, p, vec4<f32>(o, 0.0, 0.0, 0.0));
}
