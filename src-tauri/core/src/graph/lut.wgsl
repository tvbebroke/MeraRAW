// 3D LUT look (slot 8, after tone_curve, before sharpen).
//
// Working chain: scene-linear Rec.2020. CPU oracle: CubeLut::apply_working.
// Uniforms (matrices, shaper, interp) are uploaded from color.rs / idt.rs —
// do not hardcode Rec.709 matrices here.
//
// Storage layout (`lut`): interleaved r,g,b per entry, R varying fastest —
//   index = ((b*size + g)*size + r) * 3 + channel.

struct LutU {
  size: u32,
  width: u32,
  height: u32,
  shaper: u32, // Transfer id (0 linear … 10 HLG)
  dmin: vec4<f32>,
  dmax: vec4<f32>,
  opacity: f32,
  interp: u32, // 0 tetrahedral, 1 trilinear
  kind: u32,
  _p0: u32,
  m_in0: vec4<f32>,
  m_in1: vec4<f32>,
  m_in2: vec4<f32>,
  m_out0: vec4<f32>,
  m_out1: vec4<f32>,
  m_out2: vec4<f32>,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> u: LutU;
@group(0) @binding(3) var<storage, read> lut: array<f32>;

fn lut_at(r: u32, g: u32, b: u32) -> vec3<f32> {
  let idx = ((b * u.size + g) * u.size + r) * 3u;
  return vec3<f32>(lut[idx], lut[idx + 1u], lut[idx + 2u]);
}

fn mul3(r0: vec4<f32>, r1: vec4<f32>, r2: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
  return vec3<f32>(dot(r0.xyz, v), dot(r1.xyz, v), dot(r2.xyz, v));
}

fn srgb_oetf(c: f32) -> f32 {
  if (c <= 0.0) { return 0.0; }
  if (c <= 0.0031308) { return 12.92 * c; }
  if (c <= 1.0) { return 1.055 * pow(c, 1.0 / 2.4) - 0.055; }
  let d = 1.055 / 2.4;
  return 1.0 + d * (c - 1.0);
}
fn srgb_eotf(c: f32) -> f32 {
  if (c <= 0.0) { return 0.0; }
  if (c <= 0.04045) { return c / 12.92; }
  if (c <= 1.0) { return pow((c + 0.055) / 1.055, 2.4); }
  let d = 1.055 / 2.4;
  return 1.0 + (c - 1.0) / max(d, 1e-6);
}

fn rec709_oetf(c: f32) -> f32 {
  if (c < 0.0) { return 0.0; }
  if (c < 0.018) { return 4.5 * c; }
  return 1.099 * pow(c, 0.45) - 0.099;
}
fn rec709_eotf(c: f32) -> f32 {
  if (c < 0.0) { return 0.0; }
  if (c < 0.081) { return c / 4.5; }
  return pow((c + 0.099) / 1.099, 1.0 / 0.45);
}

fn cineon_enc(lin: f32) -> f32 {
  let y = max(lin, 0.0) * (1.0 - 0.0108) + 0.0108;
  return (300.0 * log(y) / log(10.0) + 685.0) / 1023.0;
}
fn cineon_dec(x: f32) -> f32 {
  return (pow(10.0, (1023.0 * x - 685.0) / 300.0) - 0.0108) / (1.0 - 0.0108);
}

fn acescct_enc(lin: f32) -> f32 {
  let x = max(lin, 0.0);
  if (x <= 0.0078125) { return 10.5402377416545 * x + 0.0729055341958355; }
  return (log2(x) + 9.72) / 17.52;
}
fn acescct_dec(x: f32) -> f32 {
  if (x <= 0.155251141552511) { return (x - 0.0729055341958355) / 10.5402377416545; }
  return exp2(x * 17.52 - 9.72);
}

fn logc3_enc(lin: f32) -> f32 {
  if (lin > 0.010591) { return 0.247190 * log(5.555556 * lin + 0.052272) / log(10.0) + 0.385537; }
  return 5.367655 * lin + 0.092809;
}
fn logc3_dec(x: f32) -> f32 {
  let cut = logc3_enc(0.010591);
  if (x > cut) { return (pow(10.0, (x - 0.385537) / 0.247190) - 0.052272) / 5.555556; }
  return (x - 0.092809) / 5.367655;
}

fn slog3_enc(lin: f32) -> f32 {
  if (lin >= 0.01125) { return (420.0 + log((lin + 0.01) / 0.19) / log(10.0) * 261.5) / 1023.0; }
  return (lin * (171.2102946929 - 95.0) / 0.01125 + 95.0) / 1023.0;
}
fn slog3_dec(x: f32) -> f32 {
  if (x >= 171.2102946929 / 1023.0) { return 0.19 * pow(10.0, (x * 1023.0 - 420.0) / 261.5) - 0.01; }
  return (x * 1023.0 - 95.0) * 0.01125 / (171.2102946929 - 95.0);
}

fn vlog_enc(lin: f32) -> f32 {
  if (lin < 0.01) { return 5.6 * lin + 0.125; }
  return 0.241514 * log(lin + 0.00873) / log(10.0) + 0.598206;
}
fn vlog_dec(x: f32) -> f32 {
  if (x < vlog_enc(0.01)) { return (x - 0.125) / 5.6; }
  return pow(10.0, (x - 0.598206) / 0.241514) - 0.00873;
}

fn encode_ch(sh: u32, c: f32) -> f32 {
  switch sh {
    case 1u: { return srgb_oetf(c); }
    case 2u: { return rec709_oetf(c); }
    case 3u: { return cineon_enc(c); }
    case 4u: { return acescct_enc(c); }
    case 5u: { return logc3_enc(c); }
    case 7u: { return slog3_enc(c); }
    case 8u: { return vlog_enc(c); }
    default: { return c; }
  }
}
fn decode_ch(sh: u32, c: f32) -> f32 {
  switch sh {
    case 1u: { return srgb_eotf(c); }
    case 2u: { return rec709_eotf(c); }
    case 3u: { return cineon_dec(c); }
    case 4u: { return acescct_dec(c); }
    case 5u: { return logc3_dec(c); }
    case 7u: { return slog3_dec(c); }
    case 8u: { return vlog_dec(c); }
    default: { return c; }
  }
}

fn encode3(sh: u32, rgb: vec3<f32>) -> vec3<f32> {
  return vec3<f32>(encode_ch(sh, rgb.x), encode_ch(sh, rgb.y), encode_ch(sh, rgb.z));
}
fn decode3(sh: u32, rgb: vec3<f32>) -> vec3<f32> {
  return vec3<f32>(decode_ch(sh, rgb.x), decode_ch(sh, rgb.y), decode_ch(sh, rgb.z));
}

fn domain_t(rgb: vec3<f32>) -> vec3<f32> {
  let span = max(u.dmax.xyz - u.dmin.xyz, vec3<f32>(1e-6));
  return clamp((rgb - u.dmin.xyz) / span, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn sample_trilinear(rgb: vec3<f32>) -> vec3<f32> {
  let n = f32(u.size - 1u);
  let pos = domain_t(rgb) * n;
  let i0 = vec3<u32>(floor(pos));
  let i1 = min(i0 + vec3<u32>(1u), vec3<u32>(u.size - 1u));
  let f = pos - floor(pos);
  let c000 = lut_at(i0.x, i0.y, i0.z);
  let c100 = lut_at(i1.x, i0.y, i0.z);
  let c010 = lut_at(i0.x, i1.y, i0.z);
  let c001 = lut_at(i0.x, i0.y, i1.z);
  let c110 = lut_at(i1.x, i1.y, i0.z);
  let c101 = lut_at(i1.x, i0.y, i1.z);
  let c011 = lut_at(i0.x, i1.y, i1.z);
  let c111 = lut_at(i1.x, i1.y, i1.z);
  let c00 = mix(c000, c100, f.x);
  let c10 = mix(c010, c110, f.x);
  let c01 = mix(c001, c101, f.x);
  let c11 = mix(c011, c111, f.x);
  let c0 = mix(c00, c10, f.y);
  let c1 = mix(c01, c11, f.y);
  return mix(c0, c1, f.z);
}

fn sample_tetrahedral(rgb: vec3<f32>) -> vec3<f32> {
  let n = f32(u.size - 1u);
  let pos = domain_t(rgb) * n;
  let i0 = vec3<u32>(floor(pos));
  let i1 = min(i0 + vec3<u32>(1u), vec3<u32>(u.size - 1u));
  let f = pos - floor(pos);
  let fx = f.x; let fy = f.y; let fz = f.z;
  let c000 = lut_at(i0.x, i0.y, i0.z);
  let c100 = lut_at(i1.x, i0.y, i0.z);
  let c010 = lut_at(i0.x, i1.y, i0.z);
  let c001 = lut_at(i0.x, i0.y, i1.z);
  let c110 = lut_at(i1.x, i1.y, i0.z);
  let c101 = lut_at(i1.x, i0.y, i1.z);
  let c011 = lut_at(i0.x, i1.y, i1.z);
  let c111 = lut_at(i1.x, i1.y, i1.z);
  if (fx >= fy) {
    if (fy >= fz) {
      return (1.0 - fx) * c000 + (fx - fy) * c100 + (fy - fz) * c110 + fz * c111;
    } else if (fx >= fz) {
      return (1.0 - fx) * c000 + (fx - fz) * c100 + (fz - fy) * c101 + fy * c111;
    } else {
      return (1.0 - fz) * c000 + (fz - fx) * c001 + (fx - fy) * c101 + fy * c111;
    }
  } else if (fz >= fy) {
    return (1.0 - fz) * c000 + (fz - fy) * c001 + (fy - fx) * c011 + fx * c111;
  } else if (fz >= fx) {
    return (1.0 - fy) * c000 + (fy - fz) * c010 + (fz - fx) * c011 + fx * c111;
  }
  return (1.0 - fy) * c000 + (fy - fx) * c010 + (fx - fz) * c110 + fz * c111;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) { return; }
  let coord = vec2<i32>(gid.xy);
  let p = textureLoad(src, coord, 0);
  let rgb0 = p.rgb;
  let amount = clamp(u.opacity, 0.0, 1.0);
  if (amount <= 0.0) {
    textureStore(dst, coord, p);
    return;
  }

  let lin_in = mul3(u.m_in0, u.m_in1, u.m_in2, rgb0);
  var encoded: vec3<f32>;
  var residual = vec3<f32>(0.0);
  let display = (u.shaper == 0u) || (u.shaper == 1u) || (u.shaper == 2u);
  if (display) {
    let sdr = clamp(lin_in, vec3<f32>(0.0), vec3<f32>(1.0));
    residual = lin_in - sdr;
    encoded = encode3(u.shaper, sdr);
  } else {
    encoded = encode3(u.shaper, max(lin_in, vec3<f32>(0.0)));
  }

  var looked: vec3<f32>;
  if (u.interp == 1u) {
    looked = sample_trilinear(encoded);
  } else {
    looked = sample_tetrahedral(encoded);
  }
  let decoded = decode3(u.shaper, looked) + residual;
  let lin_work = mul3(u.m_out0, u.m_out1, u.m_out2, decoded);
  let outc = mix(rgb0, lin_work, amount);
  textureStore(dst, coord, vec4<f32>(outc, p.a));
}
