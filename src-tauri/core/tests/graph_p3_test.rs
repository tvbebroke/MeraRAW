//! Phase 3 module behavior on GPU (test strategy §6 + DoD 3.15):
//! constant-hue contrast, band-targeted HSL (orange = skin), zone-targeted
//! grade, flat-field invariance of noise/sharpen, calibration white hold.

use meratech_core::doc::EditDoc;
use meratech_core::gpu::display::{upload_working_texture, ViewParams};
use meratech_core::gpu::GpuContext;
use meratech_core::graph::RenderGraph;
use meratech_core::ops::{apply_op, Op};
use serde_json::json;

const W: u32 = 8;
const H: u32 = 8;
const AS_SHOT: f32 = 5200.0;

fn tex_of(gpu: &GpuContext, rgb: [f32; 3]) -> wgpu::Texture {
    let mut bytes = Vec::with_capacity((W * H * 8) as usize);
    let px: Vec<u8> = rgb
        .iter()
        .flat_map(|v| half::f16::from_f32(*v).to_le_bytes())
        .chain(half::f16::ONE.to_le_bytes())
        .collect();
    for _ in 0..W * H {
        bytes.extend_from_slice(&px);
    }
    upload_working_texture(gpu, &bytes, W, H)
}

fn view() -> ViewParams {
    ViewParams {
        out_w: W,
        out_h: H,
        scale: Some(1.0),
        center_x: 0.5,
        center_y: 0.5,
        crop_preview: false,
    }
}

fn center(frame: &[u8]) -> [f32; 3] {
    let i = (((H / 2) * W + W / 2) * 4) as usize;
    [frame[i] as f32, frame[i + 1] as f32, frame[i + 2] as f32]
}

/// classic HSV hue in degrees from display bytes
fn hue_deg(px: [f32; 3]) -> f32 {
    let (r, g, b) = (px[0], px[1], px[2]);
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let d = mx - mn;
    if d < 1.0 {
        return 0.0;
    }
    let h = if mx == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if mx == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    h.rem_euclid(360.0)
}

fn sat(px: [f32; 3]) -> f32 {
    let mx = px[0].max(px[1]).max(px[2]);
    let mn = px[0].min(px[1]).min(px[2]);
    if mx <= 0.0 {
        0.0
    } else {
        (mx - mn) / mx
    }
}

struct Rig {
    gpu: GpuContext,
    graph: RenderGraph,
}

impl Rig {
    fn render(&mut self, rgb: [f32; 3], edit: impl FnOnce(&mut EditDoc)) -> [f32; 3] {
        let tex = tex_of(&self.gpu, rgb);
        let tv = tex.create_view(&Default::default());
        let mut doc = EditDoc::new("/synthetic.ARW");
        edit(&mut doc);
        self.graph.invalidate_all();
        let frame = self
            .graph
            .render(
                &self.gpu,
                &tv,
                W,
                H,
                &view(),
                &doc,
                AS_SHOT,
                &Default::default(),
                None,
                None,
                None,
            )
            .unwrap();
        center(&frame)
    }
}

fn set(doc: &mut EditDoc, path: &str, v: serde_json::Value) {
    apply_op(
        doc,
        &Op::SetParam {
            path: path.into(),
            value: v,
        },
    )
    .unwrap();
}

const SKIN: [f32; 3] = [0.50, 0.35, 0.25];

#[tokio::test]
async fn p3_module_behavior() {
    let Ok(gpu) = GpuContext::init().await else {
        eprintln!("no GPU; skipping");
        return;
    };
    let graph = RenderGraph::new(&gpu);
    let mut rig = Rig { gpu, graph };

    let base = rig.render(SKIN, |_| {});

    // ---- tone_curve: contrast push must hold hue (film-like, DoD #2/#4)
    let contrasted = rig.render(SKIN, |d| set(d, "tone_curve.contrast", json!(70)));
    let dh = (hue_deg(contrasted) - hue_deg(base)).abs();
    assert!(
        dh.min(360.0 - dh) < 4.0,
        "contrast shifted hue: {} → {} ({}°)",
        hue_deg(base),
        hue_deg(contrasted),
        dh
    );
    // skin luma ~0.38 sits below pivot → darkens under +contrast
    assert!(
        contrasted[1] < base[1] - 2.0,
        "midtone-below-pivot should darken: {base:?} → {contrasted:?}"
    );

    // ---- hsl: orange band targets skin; blue band must NOT (DoD band isolation)
    let desat = rig.render(SKIN, |d| set(d, "hsl.orange.sat", json!(-85)));
    assert!(
        sat(desat) < sat(base) * 0.75,
        "orange desat failed: {} → {}",
        sat(base),
        sat(desat)
    );
    let blue_edit = rig.render(SKIN, |d| set(d, "hsl.blue.sat", json!(-85)));
    for c in 0..3 {
        assert!(
            (blue_edit[c] - base[c]).abs() <= 2.0,
            "blue band leaked onto skin: {base:?} → {blue_edit:?}"
        );
    }
    // orange lum lifts skin
    let lifted = rig.render(SKIN, |d| set(d, "hsl.orange.lum", json!(60)));
    assert!(lifted[1] > base[1] + 3.0, "orange lum: {base:?} → {lifted:?}");

    // ---- color_grade: shadows zone leaves highlights alone
    let bright: [f32; 3] = [1.2, 1.2, 1.2];
    let bright_base = rig.render(bright, |_| {});
    let bright_shadowed = rig.render(bright, |d| {
        set(d, "color_grade.shadows_hue", json!(220));
        set(d, "color_grade.shadows_sat", json!(80));
    });
    for c in 0..3 {
        assert!(
            (bright_shadowed[c] - bright_base[c]).abs() <= 3.0,
            "shadow wheel hit highlights: {bright_base:?} → {bright_shadowed:?}"
        );
    }
    let dark: [f32; 3] = [0.04, 0.04, 0.04];
    let dark_base = rig.render(dark, |_| {});
    let dark_graded = rig.render(dark, |d| {
        set(d, "color_grade.shadows_hue", json!(220));
        set(d, "color_grade.shadows_sat", json!(80));
    });
    assert!(
        dark_graded[2] > dark_base[2] + 2.0,
        "shadow wheel (blue) should tint shadows: {dark_base:?} → {dark_graded:?}"
    );

    // ---- grade saturation: constant-hue chroma move on skin
    let satboost = rig.render(SKIN, |d| set(d, "color_grade.perceptual_sat", json!(60)));
    let dh = (hue_deg(satboost) - hue_deg(base)).abs();
    assert!(
        dh.min(360.0 - dh) < 5.0,
        "sat boost shifted hue by {dh}°"
    );
    assert!(sat(satboost) > sat(base), "sat should rise");

    // ---- noise + sharpen: flat field is invariant (no fake detail)
    let denoised = rig.render(SKIN, |d| set(d, "detail.noise_luma", json!(70)));
    let sharpened = rig.render(SKIN, |d| {
        set(d, "detail.sharpen_amount", json!(100));
    });
    for c in 0..3 {
        assert!(
            (denoised[c] - base[c]).abs() <= 2.0,
            "NR changed a flat field: {base:?} → {denoised:?}"
        );
        assert!(
            (sharpened[c] - base[c]).abs() <= 2.0,
            "sharpen changed a flat field: {base:?} → {sharpened:?}"
        );
    }

    // ---- calibration: white must stay white; skin must move
    let gray: [f32; 3] = [0.4, 0.4, 0.4];
    let gray_base = rig.render(gray, |_| {});
    let gray_cal = rig.render(gray, |d| {
        set(d, "calibration.red_hue", json!(80));
        set(d, "calibration.blue_sat", json!(-50));
    });
    for c in 0..3 {
        assert!(
            (gray_cal[c] - gray_base[c]).abs() <= 2.0,
            "calibration moved neutral gray: {gray_base:?} → {gray_cal:?}"
        );
    }
    let skin_cal = rig.render(SKIN, |d| set(d, "calibration.red_hue", json!(80)));
    let moved: f32 = (0..3).map(|c| (skin_cal[c] - base[c]).abs()).sum();
    assert!(moved > 4.0, "calibration had no effect on color: {base:?} → {skin_cal:?}");

    // ---- shadow_tint pushes shadows toward magenta, spares highlights
    let dark_tinted = rig.render(dark, |d| set(d, "calibration.shadow_tint", json!(100)));
    assert!(
        dark_tinted[1] < dark_base[1] + 1.0
            && (dark_tinted[0] + dark_tinted[2]) > (dark_base[0] + dark_base[2]) + 2.0,
        "shadow tint magenta push failed: {dark_base:?} → {dark_tinted:?}"
    );
}
