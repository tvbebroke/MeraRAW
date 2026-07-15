// Classical denoise (slot 4) — impulse → VST → Y0U0V0 → à-trous Wiener
// (or NLM luma) → detail recovery → inverse VST. Identity when all strengths
// are 0 (pass skipped by the graph). Mirrors core::denoise::cpu stage-for-stage
// with a parallel (non-cascaded) multi-scale approximation for the wavelet
// path so it fits a single dispatch; CPU export/bench uses true cascade.

struct NoiseUniforms {
  profile_a: f32,
  profile_b: f32,
  luma_s0: f32,
  luma_s1: f32,
  luma_s2: f32,
  luma_s3: f32,
  luma_s4: f32,
  chroma_s0: f32,
  chroma_s1: f32,
  chroma_s2: f32,
  chroma_s3: f32,
  chroma_s4: f32,
  chroma_s5: f32,
  detail: f32,
  impulse_k: f32,
  texture_k: f32,
  sigma_scale: f32,
  use_nlm: f32,
  nlm_patch: f32,
  nlm_search: f32,
  nlm_h: f32,
  nlm_center: f32,
  width: u32,
  height: u32,
  _p0: u32,
  _p1: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> u: NoiseUniforms;

const SIGMA_Y0: f32 = 0.5773503;
const SIGMA_U0: f32 = 0.7071068;
const SIGMA_V0: f32 = 0.6123724;
const ATROUS_SIGMA = array<f32, 6>(0.8907, 0.2007, 0.0855, 0.0531, 0.0265, 0.0133);
const K5 = array<f32, 5>(1.0 / 16.0, 4.0 / 16.0, 6.0 / 16.0, 4.0 / 16.0, 1.0 / 16.0);

fn load_rgb(c: vec2<i32>) -> vec3<f32> {
  let p = clamp(c, vec2<i32>(0), vec2<i32>(i32(u.width) - 1, i32(u.height) - 1));
  return textureLoad(src, p, 0).rgb;
}

fn vst_forward(z: f32) -> f32 {
  if (u.profile_a < 1e-12) {
    return z / sqrt(max(u.profile_b, 1e-12));
  }
  let t = z / u.profile_a + 0.375 + u.profile_b / (u.profile_a * u.profile_a);
  return 2.0 * sqrt(max(t, 0.0));
}

fn vst_inverse(d_in: f32) -> f32 {
  if (u.profile_a < 1e-12) {
    return d_in * sqrt(max(u.profile_b, 1e-12));
  }
  let sigma2 = u.profile_b / (u.profile_a * u.profile_a);
  let d = max(d_in, 0.5);
  let sq32 = 1.2247449;
  let inv = d * d * 0.25 + 0.25 * sq32 / d - 1.375 / (d * d)
    + 0.625 * sq32 / (d * d * d) - 0.125 - sigma2;
  return max(u.profile_a * inv, 0.0);
}

fn rgb_to_y0u0v0(rgb: vec3<f32>) -> vec3<f32> {
  return vec3<f32>(
    (rgb.r + rgb.g + rgb.b) / 3.0,
    (rgb.r - rgb.b) * 0.5,
    (rgb.r - 2.0 * rgb.g + rgb.b) * 0.25
  );
}

fn y0u0v0_to_rgb(yuv: vec3<f32>) -> vec3<f32> {
  let g = yuv.x - (4.0 / 3.0) * yuv.z;
  let r = yuv.x + (2.0 / 3.0) * yuv.z + yuv.y;
  let b = yuv.x + (2.0 / 3.0) * yuv.z - yuv.y;
  return vec3<f32>(r, g, b);
}

fn sigma_at(x: f32) -> f32 {
  return sqrt(max(u.profile_a * max(x, 0.0) + u.profile_b, 0.0));
}

fn median9(v: ptr<function, array<f32, 9>>) -> f32 {
  // Partial sort network — enough to extract the median (index 4).
  for (var i = 0; i < 8; i++) {
    for (var j = i + 1; j < 9; j++) {
      if ((*v)[j] < (*v)[i]) {
        let t = (*v)[i];
        (*v)[i] = (*v)[j];
        (*v)[j] = t;
      }
    }
  }
  return (*v)[4];
}

fn impulse_rgb(coord: vec2<i32>) -> vec3<f32> {
  var outc = load_rgb(coord);
  if (u.impulse_k <= 0.0) {
    return outc;
  }
  for (var c = 0; c < 3; c++) {
    var v: array<f32, 9>;
    var i = 0;
    for (var dy = -1; dy <= 1; dy++) {
      for (var dx = -1; dx <= 1; dx++) {
        let s = load_rgb(coord + vec2<i32>(dx, dy));
        v[i] = s[c];
        i += 1;
      }
    }
    let med = median9(&v);
    let center = outc[c];
    if (abs(center - med) > u.impulse_k * max(sigma_at(med), 1e-6)) {
      outc[c] = med;
    }
  }
  return outc;
}

fn load_yuv(coord: vec2<i32>) -> vec3<f32> {
  let rgb = impulse_rgb(coord);
  let r = vst_forward(rgb.r);
  let g = vst_forward(rgb.g);
  let b = vst_forward(rgb.b);
  return rgb_to_y0u0v0(vec3<f32>(r, g, b));
}

/// Separable B3 smooth of YUV at dilated step, sampled from impulse+VST source.
fn atrous_smooth_yuv(coord: vec2<i32>, level: i32) -> vec3<f32> {
  let step = i32(1u << u32(level));
  // horizontal into registers via nested vertical (full 5×5 separable)
  var tmp: array<vec3<f32>, 5>;
  for (var i = 0; i < 5; i++) {
    var acc = vec3<f32>(0.0);
    for (var j = 0; j < 5; j++) {
      let q = coord + vec2<i32>((j - 2) * step, (i - 2) * step);
      acc += K5[j] * load_yuv(q);
    }
    tmp[i] = acc;
  }
  var outv = vec3<f32>(0.0);
  for (var i = 0; i < 5; i++) {
    outv += K5[i] * tmp[i];
  }
  return outv;
}

fn wiener(d: f32, t: f32) -> f32 {
  if (t <= 0.0) { return 1.0; }
  let d2 = d * d;
  return max(d2 - t * t, 0.0) / (d2 + 1e-9);
}

fn luma_s(j: i32) -> f32 {
  switch j {
    case 0: { return u.luma_s0; }
    case 1: { return u.luma_s1; }
    case 2: { return u.luma_s2; }
    case 3: { return u.luma_s3; }
    default: { return u.luma_s4; }
  }
}

fn chroma_s(j: i32) -> f32 {
  switch j {
    case 0: { return u.chroma_s0; }
    case 1: { return u.chroma_s1; }
    case 2: { return u.chroma_s2; }
    case 3: { return u.chroma_s3; }
    case 4: { return u.chroma_s4; }
    default: { return u.chroma_s5; }
  }
}

/// Multi-scale starlet-style shrink. Each level's smooth is a dilated B3 of
/// the *source* (parallel), then we difference successive smooths — a common
/// single-pass approximation of the cascaded à-trous used on CPU.
fn wavelet_denoise(coord: vec2<i32>, yuv: vec3<f32>) -> vec3<f32> {
  var prev = yuv;
  var accum = vec3<f32>(0.0);
  var deep = yuv;
  for (var j = 0; j < 6; j++) {
    let next = atrous_smooth_yuv(coord, j);
    let d = prev - next;
    if (j < 5) {
      let sy = luma_s(j) * SIGMA_Y0 * ATROUS_SIGMA[j] * u.sigma_scale;
      accum.x += d.x * wiener(d.x, sy);
    }
    let su = chroma_s(j) * SIGMA_U0 * ATROUS_SIGMA[j] * u.sigma_scale;
    let sv = chroma_s(j) * SIGMA_V0 * ATROUS_SIGMA[j] * u.sigma_scale;
    accum.y += d.y * wiener(d.y, su);
    accum.z += d.z * wiener(d.z, sv);
    prev = next;
    deep = next;
  }
  return deep + accum;
}

fn nlm_luma(coord: vec2<i32>, yuv: vec3<f32>) -> f32 {
  let sigma = SIGMA_Y0 * u.sigma_scale;
  let pr = i32(u.nlm_patch);
  let sr = i32(u.nlm_search);
  let patch_n = f32((2 * pr + 1) * (2 * pr + 1));
  let h2 = (u.nlm_h * sigma) * (u.nlm_h * sigma) * patch_n;
  var acc = 0.0;
  var wsum = 0.0;
  for (var qy = -sr; qy <= sr; qy++) {
    for (var qx = -sr; qx <= sr; qx++) {
      if (qx == 0 && qy == 0) { continue; }
      let far = max(abs(qx), abs(qy)) > 3;
      if (far && ((qx & 1) != 0 || (qy & 1) != 0)) { continue; }
      var dist = 0.0;
      for (var py = -pr; py <= pr; py++) {
        for (var px = -pr; px <= pr; px++) {
          let a = load_yuv(coord + vec2<i32>(px, py)).x;
          let b = load_yuv(coord + vec2<i32>(qx + px, qy + py)).x;
          dist += (a - b) * (a - b);
        }
      }
      let d = max(dist - 2.0 * sigma * sigma * patch_n, 0.0);
      let wgt = exp(-d / h2) * select(1.0, 4.0, far);
      acc += wgt * load_yuv(coord + vec2<i32>(qx, qy)).x;
      wsum += wgt;
    }
  }
  let wc = u.nlm_center * max(wsum, 1e-6);
  return (acc + wc * yuv.x) / max(wsum + wc, 1e-6);
}

fn recover(coord: vec2<i32>, orig: vec3<f32>, den: vec3<f32>) -> vec3<f32> {
  var outv = den;
  let sigma_y = SIGMA_Y0 * u.sigma_scale;
  if (u.detail > 0.0) {
    let r = orig.x - den.x;
    let t = clamp((abs(r) / max(sigma_y, 1e-6) - 1.5) / 2.5, 0.0, 1.0);
    let gate = t * t * (3.0 - 2.0 * t);
    outv.x = den.x + u.detail * r * gate;
  }
  var s = 0.0;
  var s2 = 0.0;
  for (var dy = -2; dy <= 2; dy++) {
    for (var dx = -2; dx <= 2; dx++) {
      let v = load_yuv(coord + vec2<i32>(dx, dy)).x;
      s += v;
      s2 += v * v;
    }
  }
  let m = s / 25.0;
  let var_ = max(s2 / 25.0 - m * m, 0.0);
  let noise_var = sigma_y * sigma_y;
  let t = clamp((var_ - noise_var) / max(u.texture_k * noise_var, 1e-9), 0.0, 1.0);
  outv = outv + t * 0.5 * (orig - outv);
  return outv;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let coord = vec2<i32>(gid.xy);
  let a_in = textureLoad(src, coord, 0).a;
  let yuv = load_yuv(coord);
  var den = wavelet_denoise(coord, yuv);
  if (u.use_nlm > 0.5) {
    den.x = nlm_luma(coord, yuv);
  }
  let fin = recover(coord, yuv, den);
  let rgb_v = y0u0v0_to_rgb(fin);
  let outc = vec3<f32>(
    vst_inverse(rgb_v.r),
    vst_inverse(rgb_v.g),
    vst_inverse(rgb_v.b)
  );
  textureStore(dst, coord, vec4<f32>(max(outc, vec3<f32>(0.0)), a_in));
}
