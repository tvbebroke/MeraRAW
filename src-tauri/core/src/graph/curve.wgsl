// Tone curve (slot 7): RGB luma (constant-hue) + optional per-channel R/G/B LUTs.
// LUT buffer layout: [luma | red | green | blue] × lut_size entries.

struct CurveUniforms {
  width: u32,
  height: u32,
  lut_size: u32,
  flags: u32, // bit0=luma, bit1=R, bit2=G, bit3=B
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> u: CurveUniforms;
@group(0) @binding(3) var<storage, read> lut: array<f32>;

fn lut_at(slot: u32, t: f32) -> f32 {
  let base = slot * u.lut_size;
  let x = clamp(t, 0.0, 1.0) * f32(u.lut_size - 1u);
  let i = u32(floor(x));
  let j = min(i + 1u, u.lut_size - 1u);
  return mix(lut[base + i], lut[base + j], x - floor(x));
}

fn tone_slot(slot: u32, x: f32) -> f32 {
  let t = x / (1.0 + x);
  let y = lut_at(slot, t);
  return y / max(1.0 - y, 5e-4);
}

fn apply_luma(c: vec3<f32>) -> vec3<f32> {
  let mx = max(c.r, max(c.g, c.b));
  let mn = min(c.r, min(c.g, c.b));
  if (mx - mn < 1e-6) {
    let v = tone_slot(0u, mx);
    return vec3<f32>(v);
  }
  let mx2 = tone_slot(0u, mx);
  let mn2 = tone_slot(0u, mn);
  let ratio = (c - vec3<f32>(mn)) / (mx - mn);
  return vec3<f32>(mn2) + ratio * (mx2 - mn2);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let p = textureLoad(src, vec2<i32>(gid.xy), 0);
  var c = max(p.rgb, vec3<f32>(0.0));
  if ((u.flags & 1u) != 0u) {
    c = apply_luma(c);
  }
  if ((u.flags & 2u) != 0u) {
    c.r = tone_slot(1u, c.r);
  }
  if ((u.flags & 4u) != 0u) {
    c.g = tone_slot(2u, c.g);
  }
  if ((u.flags & 8u) != 0u) {
    c.b = tone_slot(3u, c.b);
  }
  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(max(c, vec3<f32>(0.0)), p.a));
}
