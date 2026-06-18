// Terminal display-transform node (contract F2 screen side), 1:1 over the
// chain output: gamut Rec2020→sRGB → pinned neutral view transform
// (Reinhard-extended Lw=4) → clamp → sRGB OETF. alpha 0 → letterbox bg.
// Constants mirror gpu/display.wgsl; golden tests pin them.

struct PresentUniforms {
  width: u32,
  height: u32,
  overlay: f32, // 0 = off; else mask-overlay tint strength
  look: u32,    // 0 = Neutral, 1 = Camera (punchy)
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> u: PresentUniforms;
@group(0) @binding(3) var overlay_mask: texture_2d<f32>;

const REC2020_TO_SRGB = mat3x3<f32>(
  vec3<f32>( 1.6605, -0.1246, -0.0182),
  vec3<f32>(-0.5876,  1.1329, -0.1006),
  vec3<f32>(-0.0728, -0.0083,  1.1187),
);

const VIEW_LW: f32 = 4.0;
const LUMA = vec3<f32>(0.2126, 0.7152, 0.0722);

// shared look operator — MIRRORED in export.rs::view_look (WYSIWYG).
// Reinhard-extended shoulder (headroom-safe) + a gentle S-curve, all on
// luminance so hue is preserved; then a saturation scale.
fn view_look(c: vec3<f32>, look: u32) -> vec3<f32> {
  var gain = 1.15;     // Neutral: a touch of lift so the base isn't dark
  var contrast = 0.12;
  var sat = 1.0;
  if (look == 1u) {    // Camera: punchy, JPEG-like
    gain = 1.6;
    contrast = 0.34;
    sat = 1.22;
  }
  let l = dot(max(c, vec3<f32>(0.0)), LUMA);
  if (l <= 1e-8) {
    return vec3<f32>(0.0);
  }
  let x = l * gain;
  let r = x * (1.0 + x / (VIEW_LW * VIEW_LW)) / (1.0 + x);
  let s = 0.5 - 0.5 * cos(clamp(r, 0.0, 1.0) * 3.14159265);
  let ld = clamp(mix(r, s, contrast), 0.0, 1.0);
  var outc = c * (ld / l);
  let l2 = dot(max(outc, vec3<f32>(0.0)), LUMA);
  outc = mix(vec3<f32>(l2), outc, sat);
  return max(outc, vec3<f32>(0.0));
}

fn oetf_srgb(c: vec3<f32>) -> vec3<f32> {
  let lo = c * 12.92;
  let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
  return select(hi, lo, c <= vec3<f32>(0.0031308));
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let p = textureLoad(src, vec2<i32>(gid.xy), 0);
  var encoded = vec3<f32>(0.0863, 0.0863, 0.0941); // app bg
  if (p.a > 0.0) {
    var c = REC2020_TO_SRGB * max(p.rgb, vec3<f32>(0.0));
    c = view_look(max(c, vec3<f32>(0.0)), u.look);
    encoded = oetf_srgb(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)));
    if (u.overlay > 0.0) {
      let m = textureLoad(overlay_mask, vec2<i32>(gid.xy), 0).r;
      encoded = mix(encoded, vec3<f32>(1.0, 0.15, 0.15), clamp(m, 0.0, 1.0) * u.overlay);
    }
  }
  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(encoded, 1.0));
}
