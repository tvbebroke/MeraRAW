// View-region extraction: working master → viewport tile (zoom/pan + crop).
// Coordinate mapping lives in crop_common.wgsl (prepended at pipeline build).
struct ExtractUniforms {
  out_w: u32,
  out_h: u32,
  img_w: f32,
  img_h: f32,
  scale: f32,
  center_x: f32,
  center_y: f32,
  crop_left: f32,
  crop_top: f32,
  crop_right: f32,
  crop_bottom: f32,
  crop_angle: f32,
  crop_rotate_90: u32,
  crop_flip_h: u32,
  crop_flip_v: u32,
  crop_mode: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> u: ExtractUniforms;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.out_w || gid.y >= u.out_h) {
    return;
  }
  let out_half = vec2<f32>(f32(u.out_w), f32(u.out_h)) * 0.5;
  let out_px = vec2<f32>(f32(gid.x) + 0.5, f32(gid.y) + 0.5);
  let m = crop_map_out_px(
    out_px, out_half,
    u.img_w, u.img_h, u.scale, vec2(u.center_x, u.center_y),
    u.crop_left, u.crop_top, u.crop_right, u.crop_bottom,
    u.crop_angle, u.crop_rotate_90, u.crop_flip_h, u.crop_flip_v, u.crop_mode,
  );
  var out = vec4<f32>(0.0, 0.0, 0.0, 0.0);
  if (m.inside == 1u) {
    out = vec4<f32>(max(textureSampleLevel(src, samp, m.uv, 0.0).rgb, vec3<f32>(0.0)), 1.0);
  }
  textureStore(dst, vec2<i32>(gid.xy), out);
}
