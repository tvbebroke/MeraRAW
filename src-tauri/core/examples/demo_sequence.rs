//! Demo renderer: animate the editor applying a tasteful golden-hour grade
//! + subject/background masking on a real RAW, frame by frame, so the steps
//! can be stitched into a short video. Pure render-side (no UI), exercising
//! the full P2–P4 pipeline.
//! Usage: cargo run -p meratech-core --release --example demo_sequence -- <raw> <out_dir>

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

fn smoothstep(a: f32, b: f32, t: f32) -> f32 {
    let x = ((t - a) / (b - a)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().expect("usage: demo_sequence <raw> <out_dir>"));
    let out_dir = args.next().unwrap_or_else(|| "/tmp/meratech-demo-frames".into());
    std::fs::create_dir_all(&out_dir).unwrap();

    let dec = RawlerDecoder::default();
    let img = dec.decode(&path).expect("decode");
    let payload = DecodedPayload::from_decoded(img);
    let cct = payload.meta.estimated_cct.unwrap_or(5200.0);

    // subject segmentation (once)
    let small = payload.small_cpu.downscale_to(768);
    let mask = TractSegmenter.subject(&small).expect("segment");
    eprintln!("subject mask {}x{}", mask.width, mask.height);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let gpu = rt.block_on(GpuContext::init()).expect("gpu");
    let tex = upload_working_texture(&gpu, &payload.rgba_f16, payload.width, payload.height);
    let tv = tex.create_view(&Default::default());
    let mut graph = RenderGraph::new(&gpu);
    let small_mask = upload_small_mask(&gpu, &mask.data, mask.width as u32, mask.height as u32);

    let (w, h) = (payload.width, payload.height);
    let scale = 1080.0 / w.max(h) as f32;
    let view = ViewParams {
        out_w: (w as f32 * scale) as u32,
        out_h: (h as f32 * scale) as u32,
        scale: Some(scale),
        center_x: 0.5,
        center_y: 0.5,
    };

    // global grade targets (tasteful golden hour, teal-orange)
    let global: &[(&str, f32)] = &[
        ("exposure.stops", 0.20),
        ("tone_curve.contrast", 28.0),
        ("tone_curve.highlights", -30.0),
        ("tone_curve.shadows", 22.0),
        ("color_grade.highlights_hue", 45.0),
        ("color_grade.highlights_sat", 16.0),
        ("color_grade.shadows_hue", 250.0),
        ("color_grade.shadows_sat", 10.0),
        ("color_grade.perceptual_sat", 14.0),
        ("hsl.orange.lum", 8.0),
        ("hsl.blue.sat", 22.0),
        ("hsl.purple.sat", 16.0),
    ];

    const N: usize = 60;
    for i in 0..N {
        let t = i as f32 / (N - 1) as f32;
        let g = smoothstep(0.04, 0.62, t); // global ramps first
        let m = smoothstep(0.50, 0.97, t); // masks ramp in second

        let mut doc = EditDoc::new(&path.to_string_lossy());
        for (p, v) in global {
            // hue stays fixed; only the strength-y params scale
            let scaled = if p.ends_with("_hue") { *v } else { *v * g };
            if scaled.abs() > 1e-3 {
                apply_op(
                    &mut doc,
                    &Op::SetParam {
                        path: (*p).into(),
                        value: json!(scaled),
                    },
                )
                .unwrap();
            }
        }
        // subject + background masks
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
            (format!("mask.{subj}.exposure.stops"), 0.40 * m),
            (format!("mask.{bg}.exposure.stops"), -0.28 * m),
            (format!("mask.{bg}.color_grade.perceptual_sat"), -18.0 * m),
        ] {
            if v.abs() > 1e-3 {
                apply_op(&mut doc, &Op::SetParam { path: p, value: json!(v) }).unwrap();
            }
        }

        let mut seg = HashMap::new();
        seg.insert(subj, small_mask.create_view(&Default::default()));
        seg.insert(bg, small_mask.create_view(&Default::default()));

        graph.invalidate_all();
        let frame = graph
            .render(&gpu, &tv, w, h, &view, &doc, cct, &seg, None, None)
            .expect("render");
        let rgb: Vec<u8> = frame.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        let p = format!("{out_dir}/frame_{i:03}.png");
        image::save_buffer(&p, &rgb, view.out_w, view.out_h, image::ColorType::Rgb8).unwrap();
    }
    println!("wrote {N} frames to {out_dir} ({}x{})", view.out_w, view.out_h);
}
