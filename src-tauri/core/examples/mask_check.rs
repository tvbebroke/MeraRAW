//! Visual subject-mask check: decode a RAW, run u2netp subject
//! segmentation, render with a strong scoped edit + overlay PNGs.
//! Usage: cargo run -p meratech-core --release --example mask_check -- <raw>

use meratech_core::doc::EditDoc;
use meratech_core::gpu::display::{upload_working_texture, ViewParams};
use meratech_core::gpu::GpuContext;
use meratech_core::graph::{upload_small_mask, RenderGraph};
use meratech_core::message::DecodedPayload;
use meratech_core::ops::{apply_op, Op};
use meratech_core::raw::{Decoder, RawlerDecoder};
use meratech_core::segment::{Segmenter, TractSegmenter};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: mask_check <raw>"),
    );
    let dec = RawlerDecoder::default();
    let img = dec.decode(&path).expect("decode");
    let payload = DecodedPayload::from_decoded(img);

    // subject segmentation on the small copy
    let t0 = std::time::Instant::now();
    let input = payload.small_cpu.downscale_to(768);
    let mask = TractSegmenter.subject(&input).expect("segment");
    println!("segmentation: {} ms ({}x{})", t0.elapsed().as_millis(), mask.width, mask.height);
    let coverage =
        mask.data.iter().filter(|v| **v > 0.5).count() as f32 / mask.data.len() as f32;
    println!("mask coverage >0.5: {:.1}%", coverage * 100.0);
    assert!(coverage > 0.02 && coverage < 0.9, "implausible subject mask");

    // mask PNG
    let mask_png: Vec<u8> = mask.data.iter().map(|v| (v * 255.0) as u8).collect();
    image::save_buffer(
        "/tmp/mask_subject.png",
        &mask_png,
        mask.width as u32,
        mask.height as u32,
        image::ColorType::L8,
    )
    .unwrap();

    // render: subject +1.2EV, background desaturated cool — obvious split
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let gpu = rt.block_on(GpuContext::init()).expect("gpu");
    let tex = upload_working_texture(&gpu, &payload.rgba_f16, payload.width, payload.height);
    let tv = tex.create_view(&Default::default());
    let mut graph = RenderGraph::new(&gpu);

    let mut doc = EditDoc::new(&path.to_string_lossy());
    let subj = apply_op(
        &mut doc,
        &Op::AddMask {
            kind: "subject".into(),
            source: json!({"type":"segmented","model":"subject_v1","hint":null}),
        },
    )
    .unwrap()
    .unwrap();
    let bg = apply_op(
        &mut doc,
        &Op::AddMask {
            kind: "background".into(),
            source: json!({"type":"segmented","model":"subject_v1","hint":null}),
        },
    )
    .unwrap()
    .unwrap();
    for (p, v) in [
        (format!("mask.{subj}.exposure.stops"), json!(0.8)),
        (format!("mask.{bg}.exposure.stops"), json!(-0.7)),
        (format!("mask.{bg}.color_grade.perceptual_sat"), json!(-60)),
    ] {
        apply_op(&mut doc, &Op::SetParam { path: p, value: v }).unwrap();
    }

    let small_tex = upload_small_mask(&gpu, &mask.data, mask.width as u32, mask.height as u32);
    let small_view = small_tex.create_view(&Default::default());
    let mut seg = HashMap::new();
    seg.insert(subj.clone(), small_view);
    seg.insert(
        bg.clone(),
        small_tex.create_view(&Default::default()),
    );

    let scale = 1100.0 / payload.width.max(payload.height) as f32;
    let view = ViewParams {
        out_w: (payload.width as f32 * scale) as u32,
        out_h: (payload.height as f32 * scale) as u32,
        scale: Some(scale),
        center_x: 0.5,
        center_y: 0.5,
    };
    let t1 = std::time::Instant::now();
    let frame = graph
        .render(
            &gpu,
            &tv,
            payload.width,
            payload.height,
            &view,
            &doc,
            payload.meta.estimated_cct.unwrap_or(5200.0),
            &seg,
            None,
        )
        .expect("render");
    println!(
        "masked render {} ms, passes: {:?}",
        t1.elapsed().as_millis(),
        graph.last_passes_run
    );
    let rgb: Vec<u8> = frame.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    image::save_buffer("/tmp/mask_render.png", &rgb, view.out_w, view.out_h, image::ColorType::Rgb8)
        .unwrap();
    println!("wrote /tmp/mask_subject.png + /tmp/mask_render.png");
}
