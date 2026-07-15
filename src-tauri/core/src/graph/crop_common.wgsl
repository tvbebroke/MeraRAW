// Shared crop coordinate mapping — prepended (via Rust concat) to every
// shader that needs to walk from viewport pixels back to original-image uv:
// extract.wgsl, mask_geom.wgsl, mask_sample.wgsl. Keep them in lockstep or
// masks detach from image content under crop/rotation.
//
// Forward pipeline: original → rotate-90/flip (discrete) → straighten angle
// → crop rect. Shaders inverse-map per output pixel.
//
// View contract: `scale` = output px per content px; `center` = view center
// normalized in CONTENT space, where content is
//   mode 0 = full image (no crop),
//   mode 1 = the crop-rect region of the rotated image (committed crop),
//   mode 2 = the rotated full image (crop-tool editing preview: geometry
//            applies live, the rect is drawn by the UI overlay).
// The crop rect (l,t,r,b) is normalized in post-rotation space; rotate-90
// swaps the physical axes, and the straighten rotation happens in physical
// pixels (normalized-space rotation would shear non-square images).

struct CropSample {
  uv: vec2<f32>,   // original-image uv (valid when inside == 1u)
  inside: u32,
};

fn crop_inv_discrete(uv: vec2<f32>, rot90: u32, fh: u32, fv: u32) -> vec2<f32> {
  var p = uv;
  if (fv == 1u) { p.y = 1.0 - p.y; }
  if (fh == 1u) { p.x = 1.0 - p.x; }
  if (rot90 == 3u) { p = vec2(1.0 - p.y, p.x); }
  else if (rot90 == 2u) { p = vec2(1.0 - p.x, 1.0 - p.y); }
  else if (rot90 == 1u) { p = vec2(p.y, 1.0 - p.x); }
  return p;
}

fn crop_rotated_dims(img_w: f32, img_h: f32, rot90: u32) -> vec2<f32> {
  if (rot90 % 2u == 1u) {
    return vec2(img_h, img_w);
  }
  return vec2(img_w, img_h);
}

fn crop_content_dims(
  img_w: f32, img_h: f32,
  l: f32, t: f32, r: f32, b: f32,
  rot90: u32, mode: u32,
) -> vec2<f32> {
  let rot_dims = crop_rotated_dims(img_w, img_h, rot90);
  if (mode == 1u) {
    return vec2(max(r - l, 0.01) * rot_dims.x, max(b - t, 0.01) * rot_dims.y);
  }
  if (mode == 2u) {
    return rot_dims;
  }
  return vec2(img_w, img_h);
}

fn crop_map_out_px(
  out_px: vec2<f32>, out_half: vec2<f32>,
  img_w: f32, img_h: f32, scale: f32, center: vec2<f32>,
  l: f32, t: f32, r: f32, b: f32,
  angle: f32, rot90: u32, fh: u32, fv: u32, mode: u32,
) -> CropSample {
  var res: CropSample;
  res.uv = vec2(0.0, 0.0);
  res.inside = 0u;
  let content = crop_content_dims(img_w, img_h, l, t, r, b, rot90, mode);
  let content_px = center * content + (out_px - out_half) / scale;
  let cuv = content_px / content;
  if (cuv.x < 0.0 || cuv.x > 1.0 || cuv.y < 0.0 || cuv.y > 1.0) {
    return res;
  }
  if (mode == 0u) {
    res.uv = cuv;
    res.inside = 1u;
    return res;
  }
  var rot_norm = cuv;
  if (mode == 1u) {
    rot_norm = vec2(l, t) + cuv * vec2(max(r - l, 0.01), max(b - t, 0.01));
  }
  // straighten: true rotation in physical px about the rotated-image center
  let rot_dims = crop_rotated_dims(img_w, img_h, rot90);
  let p = (rot_norm - vec2(0.5, 0.5)) * rot_dims;
  let c = cos(-angle);
  let s = sin(-angle);
  let q = vec2(p.x * c - p.y * s, p.x * s + p.y * c);
  let norm2 = q / rot_dims + vec2(0.5, 0.5);
  let uv = crop_inv_discrete(norm2, rot90, fh, fv);
  if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
    return res;
  }
  res.uv = uv;
  res.inside = 1u;
  return res;
}
