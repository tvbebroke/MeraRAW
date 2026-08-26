//! GPU render-graph tests (test strategy §6): identity-at-default,
//! known-input → known-output exposure math, and cache-the-chain
//! (editing a later module must NOT recompute earlier modules).
//! Skipped gracefully when no GPU adapter exists (CI).

use meratech_core::doc::EditDoc;
use meratech_core::gpu::display::{upload_working_texture, ViewParams};
use meratech_core::gpu::GpuContext;
use meratech_core::graph::RenderGraph;
use meratech_core::ops::{apply_op, Op};

const W: u32 = 8;
const H: u32 = 8;
const GRAY: f32 = 0.18;
const AS_SHOT: f32 = 5200.0;

fn f16_bytes(val: f32) -> Vec<u8> {
    let v = half::f16::from_f32(val).to_le_bytes();
    let one = half::f16::ONE.to_le_bytes();
    let mut out = Vec::with_capacity((W * H * 8) as usize);
    for _ in 0..W * H {
        out.extend_from_slice(&v);
        out.extend_from_slice(&v);
        out.extend_from_slice(&v);
        out.extend_from_slice(&one);
    }
    out
}

/// CPU mirror of present.wgsl `view_look` for a neutral gray, Neutral look
/// (gain 1.15, contrast 0.12, sat 1.0). sRGB OETF. Pinned with the shader.
fn expected_byte(linear: f32) -> u8 {
    let l = linear; // neutral: luma == channel value
    let (gain, contrast) = (1.15f32, 0.12f32);
    let lw = gain; // white point == gain (present.wgsl::view_look)
    let x = l * gain;
    let r = x * (1.0 + x / (lw * lw)) / (1.0 + x);
    let s = 0.5 - 0.5 * (r.clamp(0.0, 1.0) * std::f32::consts::PI).cos();
    let ld = (r + (s - r) * contrast).clamp(0.0, 1.0);
    let c = ld; // gray: outc == ld, sat 1.0 no-op
    let enc = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (enc * 255.0).round() as u8
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

fn center_pixel(frame: &[u8]) -> [u8; 3] {
    let i = (((H / 2) * W + W / 2) * 4) as usize;
    [frame[i], frame[i + 1], frame[i + 2]]
}

#[tokio::test]
async fn graph_identity_cache_and_exposure_math() {
    let Ok(gpu) = GpuContext::init().await else {
        eprintln!("no GPU; skipping");
        return;
    };
    let tex = upload_working_texture(&gpu, &f16_bytes(GRAY), W, H);
    let tex_view = tex.create_view(&Default::default());
    let mut graph = RenderGraph::new(&gpu);
    let mut doc = EditDoc::new("/synthetic.ARW");

    // 1) default doc → identity modules skipped entirely
    let frame = graph
        .render(
            &gpu,
            &tex_view,
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
    assert_eq!(
        graph.last_passes_run,
        vec!["extract", "present"],
        "default modules must be skipped (identity-at-default)"
    );
    let px = center_pixel(&frame);
    let want = expected_byte(GRAY);
    for c in px {
        assert!(
            (c as i32 - want as i32).abs() <= 3,
            "default render {px:?} != expected {want}"
        );
    }

    // 2) +1 stop → exposure pass runs, output doubles in linear
    apply_op(
        &mut doc,
        &Op::SetParam {
            path: "exposure.stops".into(),
            value: serde_json::json!(1.0),
        },
    )
    .unwrap();
    graph.invalidate_from_module("exposure");
    let frame = graph
        .render(
            &gpu,
            &tex_view,
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
    assert!(
        graph.last_passes_run.contains(&"exposure".to_string()),
        "exposure must run: {:?}",
        graph.last_passes_run
    );
    let px = center_pixel(&frame);
    let want = expected_byte(GRAY * 2.0);
    for c in px {
        assert!(
            (c as i32 - want as i32).abs() <= 3,
            "+1EV render {px:?} != expected {want}"
        );
    }

    // 3) cache-the-chain: edit the LATER module (white_balance) → exposure
    // must NOT re-run (its cache is reused), output keeps the +1EV effect.
    apply_op(
        &mut doc,
        &Op::SetParam {
            path: "white_balance.temp".into(),
            value: serde_json::json!(8000.0),
        },
    )
    .unwrap();
    graph.invalidate_from_module("white_balance");
    let frame = graph
        .render(
            &gpu,
            &tex_view,
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
    assert!(
        !graph.last_passes_run.contains(&"exposure".to_string()),
        "editing white_balance must reuse the exposure cache, ran: {:?}",
        graph.last_passes_run
    );
    assert!(
        graph.last_passes_run.contains(&"white_balance".to_string()),
        "white_balance must run"
    );
    let px = center_pixel(&frame);
    // warmer: R > B, and brightness still ~+1EV (cached exposure applied)
    assert!(px[0] > px[2], "warm WB should give R>B: {px:?}");
    let want = expected_byte(GRAY * 2.0);
    assert!(
        (px[1] as i32 - want as i32).abs() <= 12,
        "G should stay near +1EV level {want}: {px:?}"
    );

    // 4) cache must not change output vs full recompute (correctness)
    graph.invalidate_all();
    let full = graph
        .render(
            &gpu,
            &tex_view,
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
    assert_eq!(
        graph.last_passes_run,
        vec!["extract", "exposure", "white_balance", "present"]
    );
    assert_eq!(full, frame, "cached render must equal full recompute");
}
