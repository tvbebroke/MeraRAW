// Tone curve + contrast (slot 7) — film-like constant-hue method
// (RawTherapee / Adobe DNG reference): curve the max and min channel,
// interpolate the middle by its original ratio. No yellow/blue shift
// under contrast. LUT built CPU-side over t = x/(1+x).

struct CurveUniforms {
  width: u32,
  height: u32,
  lut_size: u32,
  _pad: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> u: CurveUniforms;
@group(0) @binding(3) var<storage, read> lut: array<f32>;

fn lut_at(t: f32) -> f32 {
  let x = clamp(t, 0.0, 1.0) * f32(u.lut_size - 1u);
  let i = u32(floor(x));
  let j = min(i + 1u, u.lut_size - 1u);
  return mix(lut[i], lut[j], x - floor(x));
}

// apply LUT through the shutter compression: linear → t-domain → LUT → linear
fn tone(x: f32) -> f32 {
  let t = x / (1.0 + x);
  let y = lut_at(t);
  return y / max(1.0 - y, 5e-4);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let p = textureLoad(src, vec2<i32>(gid.xy), 0);
  let c = max(p.rgb, vec3<f32>(0.0));
  let mx = max(c.r, max(c.g, c.b));
  let mn = min(c.r, min(c.g, c.b));
  var outc: vec3<f32>;
  if (mx - mn < 1e-6) {
    // neutral: plain curve
    let v = tone(mx);
    outc = vec3<f32>(v);
  } else {
    let mx2 = tone(mx);
    let mn2 = tone(mn);
    let ratio = (c - vec3<f32>(mn)) / (mx - mn); // each channel 0..1
    outc = vec3<f32>(mn2) + ratio * (mx2 - mn2);
  }
  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(max(outc, vec3<f32>(0.0)), p.a));
}
