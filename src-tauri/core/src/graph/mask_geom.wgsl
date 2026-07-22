// Geometric mask rasterization (radial / linear / brush) → R32Float,
// viewport-res, view-aware (same coordinate math as extract.wgsl).
// Geometry lives in normalized image coords (resolution-independent).

struct MaskGeomUniforms {
  // view (extract math)
  out_w: u32,
  out_h: u32,
  img_w: f32,
  img_h: f32,
  scale: f32,
  center_x: f32,
  center_y: f32,
  // 0 = radial, 1 = linear, 2 = brush
  kind: u32,
  // radial: a=(cx,cy), b=(rx,ry), c.x=rotation
  // linear: a=start, b=end
  pa: vec2<f32>,
  pb: vec2<f32>,
  rotation: f32,
  feather: f32,   // 0..1
  opacity: f32,   // 0..1 (pre-applied here)
  invert: u32,
  stroke_count: u32,
  // crop mapping (same contract as extract.wgsl / crop_common.wgsl)
  crop_left: f32,
  crop_top: f32,
  crop_right: f32,
  crop_bottom: f32,
  crop_angle: f32,
  crop_rotate_90: u32,
  crop_flip_h: u32,
  crop_flip_v: u32,
  crop_mode: u32,
  crop_persp_v: f32,
  crop_persp_h: f32,
  _pad_crop: u32,
  _p0: u32,
  _p1: u32,
  _p2: u32,
  _p3: u32,
};

// brush strokes: xy = point (normalized), z = radius (normalized to img w),
// w = hardness in [0,1], sign(w+0.5)…  add/sub encoded by radius sign.
@group(0) @binding(0) var dst: texture_storage_2d<r32float, write>;
@group(0) @binding(1) var<uniform> u: MaskGeomUniforms;
@group(0) @binding(2) var<storage, read> strokes: array<vec4<f32>>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.out_w || gid.y >= u.out_h) {
    return;
  }
  let out_half = vec2<f32>(f32(u.out_w), f32(u.out_h)) * 0.5;
  let out_px = vec2<f32>(f32(gid.x) + 0.5, f32(gid.y) + 0.5);
  let cm = crop_map_out_px(
    out_px, out_half,
    u.img_w, u.img_h, u.scale, vec2(u.center_x, u.center_y),
    u.crop_left, u.crop_top, u.crop_right, u.crop_bottom,
    u.crop_angle, u.crop_rotate_90, u.crop_flip_h, u.crop_flip_v, u.crop_mode,
    u.crop_persp_v, u.crop_persp_h,
  );
  if (cm.inside == 0u) {
    textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(0.0, 0.0, 0.0, 0.0));
    return;
  }
  let p = cm.uv; // normalized ORIGINAL image coords (mask geometry space)
  let aspect = u.img_w / u.img_h;

  var m = 0.0;
  if (u.kind == 0u) {
    // radial: elliptical falloff, feather widens the soft band
    let cs = cos(-u.rotation);
    let sn = sin(-u.rotation);
    var d = p - u.pa;
    d.x *= aspect; // circular in pixel space
    let r = vec2<f32>(d.x * cs - d.y * sn, d.x * sn + d.y * cs);
    let rr = vec2<f32>(max(u.pb.x * aspect, 1e-4), max(u.pb.y, 1e-4));
    let q = length(r / rr);
    let soft = max(u.feather, 0.02);
    m = 1.0 - smoothstep(1.0 - soft, 1.0 + soft, q);
  } else if (u.kind == 1u) {
    // linear gradient: 1 before start, 0 after end
    let dir = u.pb - u.pa;
    let len2 = max(dot(dir, dir), 1e-8);
    let t = dot(p - u.pa, dir) / len2;
    let soft = u.feather * 0.5;
    m = 1.0 - smoothstep(0.0 - soft, 1.0 + soft, t);
  } else {
    // brush: accumulate add/sub strokes
    for (var i = 0u; i < u.stroke_count; i++) {
      let s = strokes[i];
      var d = p - s.xy;
      d.x *= aspect;
      let radius = abs(s.z);
      let hardness = clamp(s.w, 0.0, 0.98);
      let cov = 1.0 - smoothstep(radius * hardness, radius * (1.0 + u.feather), length(d));
      if (s.z >= 0.0) {
        m = max(m, cov);
      } else {
        m = min(m, 1.0 - cov);
      }
    }
  }

  if (u.invert == 1u) {
    m = 1.0 - m;
  }
  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(m * u.opacity, 0.0, 0.0, 0.0));
}
