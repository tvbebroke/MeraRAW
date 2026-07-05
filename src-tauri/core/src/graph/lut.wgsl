// 3D LUT look (slot 8, after tone_curve, before sharpen).
//
// The working chain is scene-linear Rec.2020. Creative `.cube` LUTs are almost
// always authored against display-encoded values, so we sRGB-encode before the
// lookup and decode after — an identity LUT is therefore identity here. Known
// limitation: the LUT is sampled in Rec.2020 primaries (no 709→2020 conversion)
// and highlights above 1.0 clamp to the LUT's top corner.
//
// Storage layout (`lut`): interleaved r,g,b per entry, R varying fastest —
//   index = ((b*size + g)*size + r) * 3 + channel.

struct LutU {
  size: u32,
  width: u32,
  height: u32,
  _p0: u32,
  dmin: vec4<f32>,
  dmax: vec4<f32>,
  opacity: f32, // 0..1
  _p1: f32,
  _p2: f32,
  _p3: f32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> u: LutU;
@group(0) @binding(3) var<storage, read> lut: array<f32>;

fn lut_at(r: u32, g: u32, b: u32, c: u32) -> f32 {
  let idx = ((b * u.size + g) * u.size + r) * 3u + c;
  return lut[idx];
}

fn srgb_enc(c: f32) -> f32 {
  let x = clamp(c, 0.0, 1.0);
  if (x <= 0.0031308) { return 12.92 * x; }
  return 1.055 * pow(x, 1.0 / 2.4) - 0.055;
}

fn srgb_dec(c: f32) -> f32 {
  let x = clamp(c, 0.0, 1.0);
  if (x <= 0.04045) { return x / 12.92; }
  return pow((x + 0.055) / 1.055, 2.4);
}

fn sample3(rgb: vec3<f32>) -> vec3<f32> {
  let n = f32(u.size - 1u);
  let span = max(u.dmax.xyz - u.dmin.xyz, vec3<f32>(1e-6));
  let t = clamp((rgb - u.dmin.xyz) / span, vec3<f32>(0.0), vec3<f32>(1.0));
  let pos = t * n;
  let i0 = vec3<u32>(floor(pos));
  let i1 = min(i0 + vec3<u32>(1u), vec3<u32>(u.size - 1u));
  let f = pos - floor(pos);
  var acc: array<f32, 3>;
  for (var c = 0u; c < 3u; c = c + 1u) {
    let c00 = mix(lut_at(i0.x, i0.y, i0.z, c), lut_at(i1.x, i0.y, i0.z, c), f.x);
    let c10 = mix(lut_at(i0.x, i1.y, i0.z, c), lut_at(i1.x, i1.y, i0.z, c), f.x);
    let c01 = mix(lut_at(i0.x, i0.y, i1.z, c), lut_at(i1.x, i0.y, i1.z, c), f.x);
    let c11 = mix(lut_at(i0.x, i1.y, i1.z, c), lut_at(i1.x, i1.y, i1.z, c), f.x);
    let c0 = mix(c00, c10, f.y);
    let c1 = mix(c01, c11, f.y);
    acc[c] = mix(c0, c1, f.z);
  }
  return vec3<f32>(acc[0], acc[1], acc[2]);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let coord = vec2<i32>(gid.xy);
  let p = textureLoad(src, coord, 0);
  let lin = max(p.rgb, vec3<f32>(0.0));

  let enc = vec3<f32>(srgb_enc(lin.r), srgb_enc(lin.g), srgb_enc(lin.b));
  let looked = sample3(enc);
  let dec = vec3<f32>(srgb_dec(looked.r), srgb_dec(looked.g), srgb_dec(looked.b));

  let outc = mix(lin, dec, clamp(u.opacity, 0.0, 1.0));
  textureStore(dst, coord, vec4<f32>(outc, p.a));
}
