// DCP look, GPU port (replaces the CPU readback pass). Mirrors
// profile/hue_sat_map.rs + curve.rs `DcpProfile::apply_look` exactly:
//   ProfileHueSatMap (dual-illuminant CCT blend) -> tone curve
//   -> baseline EV -> ProfileLookTable, all in linear ProPhoto.
// HSV delta tables for map1/map2/look are concatenated into `tables`;
// offsets + dims live in the uniform. The tone curve is a 1D LUT.

struct U {
  width: u32,
  height: u32,
  has_map1: u32,
  has_map2: u32,
  has_look: u32,
  tone_size: u32,
  cct_weight: f32,
  baseline_gain: f32,
  m1_off: u32, m1_hd: u32, m1_sd: u32, m1_vd: u32,
  m2_off: u32, m2_hd: u32, m2_sd: u32, m2_vd: u32,
  lk_off: u32, lk_hd: u32, lk_sd: u32, lk_vd: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> u: U;
@group(0) @binding(3) var<storage, read> tables: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read> tone: array<f32>;

// Row-major matrices from hue_sat_map.rs, stored column-major for `M * v`.
const REC2020_TO_PROPHOTO = mat3x3<f32>(
  vec3<f32>(0.8351703, 0.0540198, -0.0023388),
  vec3<f32>(0.0487892, 0.9289338, 0.0363283),
  vec3<f32>(0.1159976, 0.0170577, 0.9662183),
);
const PROPHOTO_TO_REC2020 = mat3x3<f32>(
  vec3<f32>(1.2006766, -0.0699240, 0.0055354),
  vec3<f32>(-0.0574642, 1.0805933, -0.0407677),
  vec3<f32>(-0.1431305, -0.0106824, 1.0350180),
);

// Rust f32::fract is truncate-based (sign-preserving); WGSL fract() is
// floor-based. Match the CPU.
fn rfract(x: f32) -> f32 {
  return x - trunc(x);
}

fn rgb_to_hsv(rgb: vec3<f32>) -> vec3<f32> {
  let r = max(rgb.x, 0.0);
  let g = max(rgb.y, 0.0);
  let b = max(rgb.z, 0.0);
  let mx = max(r, max(g, b));
  let mn = min(r, min(g, b));
  let v = mx;
  if (mx <= 1e-10) {
    return vec3<f32>(0.0, 0.0, 0.0);
  }
  let delta = mx - mn;
  let s = delta / mx;
  if (delta <= 1e-10) {
    return vec3<f32>(0.0, 0.0, v);
  }
  var h_deg: f32;
  if (mx == r) {
    h_deg = 60.0 * (((g - b) / delta) % 6.0);
  } else if (mx == g) {
    h_deg = 60.0 * ((b - r) / delta + 2.0);
  } else {
    h_deg = 60.0 * ((r - g) / delta + 4.0);
  }
  if (h_deg < 0.0) {
    h_deg += 360.0;
  }
  return vec3<f32>(h_deg / 360.0, s, v);
}

fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
  var h_deg = abs(rfract(hsv.x)) * 360.0;
  if (hsv.x < 0.0) {
    h_deg = 360.0 - h_deg;
  }
  let s = clamp(hsv.y, 0.0, 1.0);
  let v = max(hsv.z, 0.0);
  if (s <= 1e-10) {
    return vec3<f32>(v, v, v);
  }
  let c = v * s;
  let hp = h_deg / 60.0;
  let x = c * (1.0 - abs((hp % 2.0) - 1.0));
  let m = v - c;
  var rp = 0.0;
  var gp = 0.0;
  var bp = 0.0;
  if (hp < 1.0) {
    rp = c; gp = x; bp = 0.0;
  } else if (hp < 2.0) {
    rp = x; gp = c; bp = 0.0;
  } else if (hp < 3.0) {
    rp = 0.0; gp = c; bp = x;
  } else if (hp < 4.0) {
    rp = 0.0; gp = x; bp = c;
  } else if (hp < 5.0) {
    rp = x; gp = 0.0; bp = c;
  } else {
    rp = c; gp = 0.0; bp = x;
  }
  return vec3<f32>(rp + m, gp + m, bp + m);
}

// MIRRORED in profile/hue_sat_map.rs — Fuji cyan blues → Affinity (desat + hue).
fn correct_blue_cyan(rec: vec3<f32>) -> vec3<f32> {
  let b = rec.z;
  if (b <= 1e-6 || b < rec.x || b < rec.y * 0.98) {
    return rec;
  }
  let hsv = rgb_to_hsv(rec);
  let h = hsv.x * 360.0;
  let s = hsv.y;
  let v = hsv.z;
  if (s < 0.20 || v < 0.02 || h < 175.0 || h > 225.0) {
    return rec;
  }
  let gb = rec.y / b;
  let rb = rec.x / b;
  let luma = 0.2126 * rec.x + 0.7152 * rec.y + 0.0722 * rec.z;

  if (gb > 0.70) {
    // Bluish-green / cyan — Affinity ~hue 192° at much lower sat with real red.
    let crush = clamp((0.35 - max(rb, 0.0)) / 0.35, 0.0, 1.0);
    let strength = crush * clamp((s - 0.25) / 0.75, 0.0, 1.0);
    if (strength < 0.05) { return rec; }
    let target_hue = 198.0;
    var nh = (h + (target_hue - h) * strength * 0.35) % 360.0;
    if (nh < 0.0) { nh += 360.0; }
    let ns = clamp(s * (1.0 - 0.70 * strength), 0.15, 1.0);
    var out = hsv_to_rgb(vec3<f32>(nh / 360.0, ns, v));
    let desat = 0.35 * strength;
    out = out * (1.0 - desat) + vec3<f32>(luma) * desat;
    out.x = max(out.x, luma * 0.55 * strength);
    out.z = max(out.z + 0.04 * strength * b, out.x);
    return max(out, vec3<f32>(0.0));
  }

  // Blue sky / primary blue — Affinity ~227°, R/B ≈ 0.22.
  // Mild: aggressive G crush washed Nikon/Sony skies.
  let hue_pull = clamp((225.0 - h) / 45.0, 0.0, 1.0);
  let red_pull = clamp((0.18 - max(rb, 0.0)) / 0.18, 0.0, 1.0);
  let strength = clamp(hue_pull * 0.25 + red_pull * 0.45, 0.0, 0.55);
  if (strength < 0.06) { return rec; }
  let target_hue = 227.0;
  var nh = (h + (target_hue - h) * strength * 0.65) % 360.0;
  if (nh < 0.0) { nh += 360.0; }
  let ns = clamp(s * (1.0 - 0.10 * strength), 0.0, 1.0);
  var out = hsv_to_rgb(vec3<f32>(nh / 360.0, ns, v));
  let want_r = b * 0.20;
  if (out.x < want_r) {
    out.x = out.x + (want_r - out.x) * strength * 0.7;
  }
  out.y = out.y * (1.0 - 0.08 * strength);
  return max(out, vec3<f32>(0.0));
}

// Trilinear HSV delta sample (matches HueSatMap::sample). off = first index in
// `tables`; hd/sd/vd = divisions. Returns (hue_shift_deg, sat_scale, val_scale).
fn sample_map(off: u32, hd: u32, sd: u32, vd: u32, h: f32, s: f32, v: f32) -> vec3<f32> {
  let hdf = f32(hd);
  let sdf = f32(sd);
  let vdf = f32(vd);

  var hf = (abs(rfract(h)) + select(0.0, 1.0, h < 0.0)) * hdf;
  if (hf >= hdf) {
    hf -= hdf;
  }
  let sf = clamp(s, 0.0, 1.0) * (sdf - 1.0);
  var vf = 0.0;
  if (vd > 1u) {
    vf = clamp(v, 0.0, 1.0) * (vdf - 1.0);
  }

  let h0 = u32(floor(hf)) % hd;
  let h1 = (h0 + 1u) % hd;
  let ht = hf - floor(hf);
  let s0 = u32(floor(sf));
  let s1 = min(s0 + 1u, sd - 1u);
  let st = sf - floor(sf);
  let v0 = u32(floor(vf));
  var v1 = 0u;
  if (vd > 1u) {
    v1 = min(v0 + 1u, vd - 1u);
  }
  let vt = vf - floor(vf);

  let p = hd * sd;
  let c000 = tables[off + v0 * p + h0 * sd + s0].xyz;
  let c100 = tables[off + v0 * p + h1 * sd + s0].xyz;
  let c010 = tables[off + v0 * p + h0 * sd + s1].xyz;
  let c110 = tables[off + v0 * p + h1 * sd + s1].xyz;
  let c001 = tables[off + v1 * p + h0 * sd + s0].xyz;
  let c101 = tables[off + v1 * p + h1 * sd + s0].xyz;
  let c011 = tables[off + v1 * p + h0 * sd + s1].xyz;
  let c111 = tables[off + v1 * p + h1 * sd + s1].xyz;

  let x00 = mix(c000, c100, ht);
  let x10 = mix(c010, c110, ht);
  let x01 = mix(c001, c101, ht);
  let x11 = mix(c011, c111, ht);
  let y0 = mix(x00, x10, st);
  let y1 = mix(x01, x11, st);
  return mix(y0, y1, vt);
}

// Apply an HSV delta to a ProPhoto RGB (matches HueSatMap::apply_prophoto body).
// Do NOT clamp V to 1 — scene-referred headroom must survive into the tone curve
// or speculars collapse into magenta/green after the 1D profile curve.
fn apply_delta(pro: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
  let hsv = rgb_to_hsv(pro);
  var h = hsv.x + d.x / 360.0;
  h -= floor(h);
  let s = clamp(hsv.y * d.y, 0.0, 1.0);
  let vv = max(hsv.z * d.z, 0.0);
  return hsv_to_rgb(vec3<f32>(h, s, vv));
}

// Monotone-cubic tone curve via the baked LUT (matches ProfileToneCurve::eval).
fn tone_eval(x: f32) -> f32 {
  if (x <= 0.0) {
    return 0.0;
  }
  let n = u.tone_size;
  if (x >= 1.0) {
    return tone[n - 1u] + (x - 1.0);
  }
  let fpos = x * f32(n - 1u);
  let i0 = u32(floor(fpos));
  let i1 = min(i0 + 1u, n - 1u);
  let t = fpos - floor(fpos);
  return max(mix(tone[i0], tone[i1], t), 0.0);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let p = textureLoad(src, vec2<i32>(gid.xy), 0);
  var rec = p.rgb;

  // 1. ProfileHueSatMap (dual-illuminant CCT blend) in ProPhoto.
  if (u.has_map1 != 0u) {
    let pro = REC2020_TO_PROPHOTO * rec;
    let hsv = rgb_to_hsv(pro);
    var d = sample_map(u.m1_off, u.m1_hd, u.m1_sd, u.m1_vd, hsv.x, hsv.y, hsv.z);
    if (u.has_map2 != 0u) {
      let d2 = sample_map(u.m2_off, u.m2_hd, u.m2_sd, u.m2_vd, hsv.x, hsv.y, hsv.z);
      let w = u.cct_weight;
      d = vec3<f32>(
        w * d.x + (1.0 - w) * d2.x,
        w * d.y + (1.0 - w) * d2.y,
        w * d.z + (1.0 - w) * d2.z,
      );
    }
    let pro2 = apply_delta(pro, d);
    rec = PROPHOTO_TO_REC2020 * pro2;
  }

  // 2. Tone curve — Adobe midtone per-channel (saturation) blended toward
  // luminance-preserving near clip (avoids Fuji magenta/green speculars).
  // Mirrors ProfileToneCurve::apply_rgb in curve.rs.
  let r0 = max(rec.x, 0.0);
  let g0 = max(rec.y, 0.0);
  let b0 = max(rec.z, 0.0);
  let per = vec3<f32>(tone_eval(r0), tone_eval(g0), tone_eval(b0));
  let y = 0.2627 * r0 + 0.6780 * g0 + 0.0593 * b0;
  var luma: vec3<f32>;
  if (y > 1e-8) {
    luma = vec3<f32>(r0, g0, b0) * (tone_eval(y) / y);
  } else {
    let v = tone_eval(y);
    luma = vec3<f32>(v, v, v);
  }
  let peak = max(r0, max(g0, b0));
  let chroma = (peak - min(r0, min(g0, b0))) / max(peak, 1e-6);
  // Near-neutral: full luma curve (RW2 coot/water magenta whites).
  if (chroma < 0.20) {
    rec = luma;
  } else {
    let warmth = clamp((r0 - b0) / max(peak, 1e-6), 0.0, 1.0);
    let sat_keep = warmth * clamp((chroma - 0.20) / 0.25, 0.0, 1.0);
    let blend_start = 0.72 + 0.12 * sat_keep;
    let t = clamp((peak - blend_start) / max(1.05 - blend_start, 0.05), 0.0, 1.0);
    var w = t * t * (3.0 - 2.0 * t);
    w = w * (1.0 - sat_keep * 0.45);
    rec = mix(per, luma, w);
  }

  // 3. Baseline exposure.
  rec = rec * u.baseline_gain;

  // 4. ProfileLookTable in ProPhoto — then soft-mix chroma (Affinity: Mamiya ZD).
  if (u.has_look != 0u) {
    let pre = rec;
    let pro = REC2020_TO_PROPHOTO * rec;
    let hsv = rgb_to_hsv(pro);
    let d = sample_map(u.lk_off, u.lk_hd, u.lk_sd, u.lk_vd, hsv.x, hsv.y, hsv.z);
    let pro2 = apply_delta(pro, d);
    let after = PROPHOTO_TO_REC2020 * pro2;
    let l0 = dot(max(pre, vec3<f32>(0.0)), vec3<f32>(0.2126, 0.7152, 0.0722));
    let l1 = dot(max(after, vec3<f32>(0.0)), vec3<f32>(0.2126, 0.7152, 0.0722));
    let t = 0.06;
    rec = vec3<f32>(l1) + (pre - vec3<f32>(l0)) * (1.0 - t) + (after - vec3<f32>(l1)) * t;
    rec = max(rec, vec3<f32>(0.0));
  }

  rec = correct_blue_cyan(rec);

  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(rec, p.a));
}
