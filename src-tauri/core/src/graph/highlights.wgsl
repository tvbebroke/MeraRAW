// Highlight reconstruction: scale unclipped chromaticity up to the clip
// threshold and mix back. Slot sits after exposure in the fixed chain.

struct HighlightsU {
  amount: f32,
  clip: f32,
  width: u32,
  height: u32,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> u: HighlightsU;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  if (gid.x >= u.width || gid.y >= u.height) {
    return;
  }
  let p = textureLoad(src, vec2<i32>(gid.xy), 0);
  var rgb = p.rgb;
  let amount = clamp(u.amount, 0.0, 1.0);
  if (amount > 1e-6) {
    let clip = max(u.clip, 1e-4);
    let c = min(rgb, vec3<f32>(clip));
    let m = max(c.r, max(c.g, c.b));
    if (m > 1e-6) {
      let rec = c / m * clip;
      rgb = mix(rgb, rec, amount);
    }
  }
  textureStore(dst, vec2<i32>(gid.xy), vec4<f32>(rgb, p.a));
}
