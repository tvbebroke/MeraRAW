// View-region extraction: working master (full-res linear Rec.2020) →
// viewport-resolution linear tile (zoom/pan applied, bilinear).
// First node of the interactive chain; module passes then run at viewport
// res (proxy discipline, gpu-memory spec §2.1). Out-of-image → alpha 0
// (display pass letterboxes).

struct ExtractUniforms {
  out_w: u32,
  out_h: u32,
  img_w: f32,
  img_h: f32,
  scale: f32,
  center_x: f32,
  center_y: f32,
  _pad: f32,
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
  let img_px = vec2<f32>(u.center_x * u.img_w, u.center_y * u.img_h)
    + (vec2<f32>(f32(gid.x) + 0.5, f32(gid.y) + 0.5) - out_half) / u.scale;
  let uv = img_px / vec2<f32>(u.img_w, u.img_h);
  var out = vec4<f32>(0.0, 0.0, 0.0, 0.0);
  if (uv.x >= 0.0 && uv.x <= 1.0 && uv.y >= 0.0 && uv.y <= 1.0) {
    out = vec4<f32>(max(textureSampleLevel(src, samp, uv, 0.0).rgb, vec3<f32>(0.0)), 1.0);
  }
  textureStore(dst, vec2<i32>(gid.xy), out);
}
