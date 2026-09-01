//! Mask-stage GPU tests (P4 DoD): scoped stack runs the same passes,
//! compositing only affects masked regions, invert flips, segmented masks
//! sample + blend through the joint-bilateral path.

use meratech_core::doc::EditDoc;
use meratech_core::gpu::display::{upload_working_texture, ViewParams};
use meratech_core::gpu::GpuContext;
use meratech_core::graph::{upload_small_mask, RenderGraph};
use meratech_core::ops::{apply_op, Op};
use serde_json::json;
use std::collections::HashMap;

const W: u32 = 32;
const H: u32 = 32;
const GRAY: f32 = 0.18;
const AS_SHOT: f32 = 5200.0;

fn flat_tex(gpu: &GpuContext, val: f32) -> wgpu::Texture {
    let px: Vec<u8> = [val, val, val]
        .iter()
        .flat_map(|v| half::f16::from_f32(*v).to_le_bytes())
        .chain(half::f16::ONE.to_le_bytes())
        .collect();
    let mut bytes = Vec::with_capacity((W * H * 8) as usize);
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

fn px(frame: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * W + x) * 4) as usize;
    [frame[i], frame[i + 1], frame[i + 2]]
}

#[tokio::test]
async fn radial_mask_scopes_exposure_to_center() {
    let Ok(gpu) = GpuContext::init().await else {
        eprintln!("no GPU; skipping");
        return;
    };
    let tex = flat_tex(&gpu, GRAY);
    let tv = tex.create_view(&Default::default());
    let mut graph = RenderGraph::new(&gpu);
    let mut doc = EditDoc::new("/synthetic.ARW");

    let base = graph
        .render(
            &gpu,
            &tv,
            W,
            H,
            &view(),
            &doc,
            AS_SHOT,
            &HashMap::new(),
            None,
            None,
            None,
        )
        .unwrap();
    let base_center = px(&base, W / 2, H / 2);
    let base_corner = px(&base, 1, 1);

    // tight radial mask in the center, +2 stops inside
    let id = apply_op(
        &mut doc,
        &Op::AddMask {
            kind: "radial".into(),
            source: json!({"type":"radial","center":[0.5,0.5],"radii":[0.2,0.2],"rotation":0}),
        },
    )
    .unwrap()
    .unwrap();
    apply_op(
        &mut doc,
        &Op::SetParam {
            path: format!("mask.{id}.exposure.stops"),
            value: json!(2.0),
        },
    )
    .unwrap();

    graph.invalidate_from_module("masks");
    let masked = graph
        .render(
            &gpu,
            &tv,
            W,
            H,
            &view(),
            &doc,
            AS_SHOT,
            &HashMap::new(),
            None,
            None,
            None,
        )
        .unwrap();
    assert!(
        graph
            .last_passes_run
            .iter()
            .any(|p| p == "mask-local:exposure"),
        "scoped stack must run the SAME exposure pass: {:?}",
        graph.last_passes_run
    );
    let center = px(&masked, W / 2, H / 2);
    let corner = px(&masked, 1, 1);
    assert!(
        center[1] as i32 > base_center[1] as i32 + 15,
        "masked center must brighten: {base_center:?} → {center:?}"
    );
    for c in 0..3 {
        assert!(
            (corner[c] as i32 - base_corner[c] as i32).abs() <= 2,
            "outside the mask must not change: {base_corner:?} → {corner:?}"
        );
    }

    // invert flips the effect
    apply_op(
        &mut doc,
        &Op::RefineMask {
            id: id.clone(),
            opacity: None,
            feather: None,
            invert: Some(true),
            blend: None,
            enabled: None,
            name: None,
        },
    )
    .unwrap();
    graph.invalidate_from_module("masks");
    let inverted = graph
        .render(
            &gpu,
            &tv,
            W,
            H,
            &view(),
            &doc,
            AS_SHOT,
            &HashMap::new(),
            None,
            None,
            None,
        )
        .unwrap();
    let center_i = px(&inverted, W / 2, H / 2);
    let corner_i = px(&inverted, 1, 1);
    assert!(
        corner_i[1] as i32 > base_corner[1] as i32 + 15,
        "inverted: corner brightens: {corner_i:?}"
    );
    assert!(
        (center_i[1] as i32 - base_center[1] as i32).abs() <= 4,
        "inverted: center near base: {center_i:?} vs {base_center:?}"
    );

    // opacity 0 → no effect anywhere
    apply_op(
        &mut doc,
        &Op::RefineMask {
            id,
            opacity: Some(0.0),
            feather: None,
            invert: None,
            blend: None,
            enabled: None,
            name: None,
        },
    )
    .unwrap();
    graph.invalidate_from_module("masks");
    let zeroed = graph
        .render(
            &gpu,
            &tv,
            W,
            H,
            &view(),
            &doc,
            AS_SHOT,
            &HashMap::new(),
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(px(&zeroed, 1, 1), base_corner);
    assert_eq!(px(&zeroed, W / 2, H / 2), base_center);
}

#[tokio::test]
async fn segmented_mask_blends_via_small_texture() {
    let Ok(gpu) = GpuContext::init().await else {
        eprintln!("no GPU; skipping");
        return;
    };
    let tex = flat_tex(&gpu, GRAY);
    let tv = tex.create_view(&Default::default());
    let mut graph = RenderGraph::new(&gpu);
    let mut doc = EditDoc::new("/synthetic.ARW");

    let id = apply_op(
        &mut doc,
        &Op::AddMask {
            kind: "subject".into(),
            source: json!({"type":"segmented","model":"subject_v1","hint":null}),
        },
    )
    .unwrap()
    .unwrap();
    apply_op(
        &mut doc,
        &Op::SetParam {
            path: format!("mask.{id}.exposure.stops"),
            value: json!(2.0),
        },
    )
    .unwrap();

    // small "model output": left half = 1, right half = 0 (16×16)
    let mut small = vec![0.0f32; 16 * 16];
    for y in 0..16 {
        for x in 0..8 {
            small[y * 16 + x] = 1.0;
        }
    }
    let small_tex = upload_small_mask(&gpu, &small, 16, 16);
    let mut seg = HashMap::new();
    seg.insert(id.clone(), small_tex.create_view(&Default::default()));

    let frame = graph
        .render(
            &gpu,
            &tv,
            W,
            H,
            &view(),
            &doc,
            AS_SHOT,
            &seg,
            None,
            None,
            None,
        )
        .unwrap();
    let left = px(&frame, 4, H / 2);
    let right = px(&frame, W - 4, H / 2);
    assert!(
        left[1] as i32 > right[1] as i32 + 15,
        "left (masked) must be brighter: {left:?} vs {right:?}"
    );

    // without the segmentation texture (inference pending) the mask is inert
    graph.invalidate_all();
    let pending = graph
        .render(
            &gpu,
            &tv,
            W,
            H,
            &view(),
            &doc,
            AS_SHOT,
            &HashMap::new(),
            None,
            None,
            None,
        )
        .unwrap();
    let l2 = px(&pending, 4, H / 2);
    let r2 = px(&pending, W - 4, H / 2);
    assert_eq!(l2, r2, "pending segmentation must render as no-op");
}

fn luma_ramp_tex(gpu: &GpuContext) -> wgpu::Texture {
    let mut bytes = Vec::with_capacity((W * H * 8) as usize);
    for _y in 0..H {
        for x in 0..W {
            let v = x as f32 / (W - 1) as f32;
            let h = half::f16::from_f32(v);
            bytes.extend_from_slice(&h.to_le_bytes());
            bytes.extend_from_slice(&h.to_le_bytes());
            bytes.extend_from_slice(&h.to_le_bytes());
            bytes.extend_from_slice(&half::f16::ONE.to_le_bytes());
        }
    }
    upload_working_texture(gpu, &bytes, W, H)
}

#[tokio::test]
async fn parametric_luma_mask_scopes_grey_ramp() {
    let Ok(gpu) = GpuContext::init().await else {
        eprintln!("no GPU; skipping");
        return;
    };
    let tex = luma_ramp_tex(&gpu);
    let tv = tex.create_view(&Default::default());
    let mut graph = RenderGraph::new(&gpu);
    let mut doc = EditDoc::new("/synthetic.ARW");

    let base = graph
        .render(
            &gpu,
            &tv,
            W,
            H,
            &view(),
            &doc,
            AS_SHOT,
            &HashMap::new(),
            None,
            None,
            None,
        )
        .unwrap();
    let base_lo = px(&base, 1, H / 2);
    let base_mid = px(&base, W / 2, H / 2);
    let base_hi = px(&base, W - 2, H / 2);

    let id = apply_op(
        &mut doc,
        &Op::AddMask {
            kind: "parametric".into(),
            source: json!({
                "type": "parametric",
                "luma_lo": 0.4,
                "luma_hi": 0.6,
                "chroma_lo": 0.0,
                "chroma_hi": 1.0,
                "hue_lo": 0.0,
                "hue_hi": 0.0,
                "softness": 0.02
            }),
        },
    )
    .unwrap()
    .unwrap();
    apply_op(
        &mut doc,
        &Op::SetParam {
            path: format!("mask.{id}.exposure.stops"),
            value: json!(2.0),
        },
    )
    .unwrap();

    graph.invalidate_from_module("masks");
    let masked = graph
        .render(
            &gpu,
            &tv,
            W,
            H,
            &view(),
            &doc,
            AS_SHOT,
            &HashMap::new(),
            None,
            None,
            None,
        )
        .unwrap();
    let lo = px(&masked, 1, H / 2);
    let mid = px(&masked, W / 2, H / 2);
    let hi = px(&masked, W - 2, H / 2);
    assert!(
        mid[1] as i32 > base_mid[1] as i32 + 12,
        "mid-luma band must brighten: {base_mid:?} → {mid:?}"
    );
    for c in 0..3 {
        assert!(
            (lo[c] as i32 - base_lo[c] as i32).abs() <= 3,
            "low luma must not change: {base_lo:?} → {lo:?}"
        );
        assert!(
            (hi[c] as i32 - base_hi[c] as i32).abs() <= 3,
            "high luma must not change: {base_hi:?} → {hi:?}"
        );
    }
}
