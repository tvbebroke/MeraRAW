// View-region extraction: working master → viewport tile (zoom/pan + optional crop).
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
  crop_enabled: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> u: ExtractUniforms;

fn inv_discrete(uv: vec2<f32>, rot90: u32, fh: u32, fv: u32) -> vec2<f32> {
  var p = uv;
  if (fv == 1u) { p.y = 1.0 - p.y; }
  if (fh == 1u) { p.x = 1.0 - p.x; }
  if (rot90 == 3u) { p = vec2(1.0 - p.y, p.x); }
  else if (rot90 == 2u) { p = vec2(1.0 - p.x, 1.0 - p.y); }
  else if (rot90 == 1u) { p = vec2(p.y, 1.0 - p.x); }
  return p;
}

fn rotate2d(p: vec2<f32>, center: vec2<f32>, angle_rad: f32) -> vec2<f32> {
  let v = p - center;
  let c = cos(angle_rad);
  let s = sin(angle_rad);
  return center + vec2(v.x * c - v.y * s, v.x * s + v.y * c);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.out_w || gid.y >= u.out_h) {
    return;
  }
  let out_half = vec2<f32>(f32(u.out_w), f32(u.out_h)) * 0.5;
  var out = vec4<f32>(0.0, 0.0, 0.0, 0.0);

  if (u.crop_enabled == 0u) {
    let img_px = vec2<f32>(u.center_x * u.img_w, u.center_y * u.img_h)
      + (vec2<f32>(f32(gid.x) + 0.5, f32(gid.y) + 0.5) - out_half) / u.scale;
    let uv = img_px / vec2<f32>(u.img_w, u.img_h);
    if (uv.x >= 0.0 && uv.x <= 1.0 && uv.y >= 0.0 && uv.y <= 1.0) {
      out = vec4<f32>(max(textureSampleLevel(src, samp, uv, 0.0).rgb, vec3<f32>(0.0)), 1.0);
    }
  } else {
    let crop_w = max(u.crop_right - u.crop_left, 0.01);
    let crop_h = max(u.crop_bottom - u.crop_top, 0.01);
    let eff_w = crop_w * u.img_w;
    let eff_h = crop_h * u.img_h;
    let out_px = vec2<f32>(f32(gid.x) + 0.5, f32(gid.y) + 0.5);
    let crop_center = vec2<f32>(
      (u.crop_left + u.crop_right) * 0.5 * u.img_w,
      (u.crop_top + u.crop_bottom) * 0.5 * u.img_h,
    );
    let crop_px = crop_center
      + (out_px - out_half) / u.scale;
    let uv_crop = vec2<f32>(
      (crop_px.x / u.img_w - u.crop_left) / crop_w,
      (crop_px.y / u.img_h - u.crop_top) / crop_h,
    );
    if (uv_crop.x >= 0.0 && uv_crop.x <= 1.0 && uv_crop.y >= 0.0 && uv_crop.y <= 1.0) {
      var src_uv = vec2<f32>(
        u.crop_left + uv_crop.x * crop_w,
        u.crop_top + uv_crop.y * crop_h,
      );
      src_uv = rotate2d(src_uv, vec2(0.5, 0.5), -u.crop_angle);
      src_uv = inv_discrete(src_uv, u.crop_rotate_90, u.crop_flip_h, u.crop_flip_v);
      if (src_uv.x >= 0.0 && src_uv.x <= 1.0 && src_uv.y >= 0.0 && src_uv.y <= 1.0) {
        out = vec4<f32>(max(textureSampleLevel(src, samp, src_uv, 0.0).rgb, vec3<f32>(0.0)), 1.0);
      }
    }
  }
  textureStore(dst, vec2<i32>(gid.xy), out);
}
