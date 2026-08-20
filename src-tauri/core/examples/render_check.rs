//! Full-chain visual check: decode a real RAW, apply a multi-module edit
//! doc, render through the actual GPU graph, save PNG. The P3 skin/look
//! acceptance eyeball (spec 3.15 #6).
//! Usage: cargo run -p meratech-core --release --example render_check -- <raw> [out.png] [edited|neutral]

use meratech_core::doc::EditDoc;
use meratech_core::gpu::display::{upload_working_texture, ViewParams};
use meratech_core::gpu::GpuContext;
use meratech_core::graph::RenderGraph;
use meratech_core::message::DecodedPayload;
use meratech_core::ops::{apply_op, Op};
use meratech_core::raw::{Decoder, RawlerDecoder};
use serde_json::json;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().expect("usage: render_check <raw> [out] [mode]"));
    let out = args
        .next()
        .unwrap_or_else(|| "/tmp/render_check.png".into());
    let mode = args.next().unwrap_or_else(|| "edited".into());

    let dec = RawlerDecoder::default();
    let img = dec.decode(&path).expect("decode");
    let payload = DecodedPayload::from_decoded(img);

    let gpu = pollster_block(GpuContext::init()).expect("gpu");
    let tex = upload_working_texture(&gpu, &payload.rgba_f16, payload.width, payload.height);
    let tv = tex.create_view(&Default::default());
    let mut graph = RenderGraph::new(&gpu);
    // 4th arg: look (neutral|camera|agx)
    let look = std::env::args().nth(4).unwrap_or_default();
    graph.set_look(match look.as_str() {
        "camera" => 1,
        "agx" => 2,
        _ => 0,
    });

    let mut doc = EditDoc::new(&path.to_string_lossy());
    if mode == "edited" {
        // a tasteful standard edit exercising every P3 module
        for (p, v) in [
            ("exposure.stops", json!(0.25)),
            ("tone_curve.contrast", json!(30)),
            ("tone_curve.highlights", json!(-35)),
            ("tone_curve.shadows", json!(20)),
            ("hsl.orange.lum", json!(10)),
            ("hsl.orange.sat", json!(-8)),
            ("hsl.green.sat", json!(-25)),
            ("color_grade.highlights_hue", json!(60)),
            ("color_grade.highlights_sat", json!(14)),
            ("color_grade.shadows_hue", json!(250)),
            ("color_grade.shadows_sat", json!(10)),
            ("color_grade.perceptual_sat", json!(12)),
            ("calibration.green_hue", json!(-10)),
            ("detail.noise_luma", json!(15)),
            ("detail.sharpen_amount", json!(40)),
        ] {
            apply_op(
                &mut doc,
                &Op::SetParam {
                    path: p.into(),
                    value: v,
                },
            )
            .unwrap();
        }
    }

    // 6th arg: grade model (0=perceptual, 1=classic, 2=light). When present,
    // push a strong teal-shadow / warm-highlight split so the model's
    // character is obvious in the output.
    if let Some(gm) = std::env::args().nth(5) {
        for (p, v) in [
            ("color_grade.model", json!(gm.parse::<f32>().unwrap_or(0.0))),
            ("color_grade.shadows_hue", json!(215.0)),
            ("color_grade.shadows_sat", json!(55.0)),
            ("color_grade.highlights_hue", json!(45.0)),
            ("color_grade.highlights_sat", json!(55.0)),
            ("color_grade.perceptual_sat", json!(15.0)),
        ] {
            apply_op(
                &mut doc,
                &Op::SetParam {
                    path: p.into(),
                    value: v,
                },
            )
            .unwrap();
        }
        println!("grade model = {gm}");
    }

    // fit render at 1200px wide-ish
    let scale = 1200.0 / payload.width.max(payload.height) as f32;
    let view = ViewParams {
        out_w: (payload.width as f32 * scale) as u32,
        out_h: (payload.height as f32 * scale) as u32,
        scale: Some(scale),
        center_x: 0.5,
        center_y: 0.5,
        crop_preview: false,
    };
    let t0 = std::time::Instant::now();
    let frame = graph
        .render(
            &gpu,
            &tv,
            payload.width,
            payload.height,
            &view,
            &doc,
            payload.meta.estimated_cct.unwrap_or(5200.0),
            &Default::default(),
            None,
            None,
            None,
        )
        .expect("render");
    println!(
        "render {}x{} in {} ms, passes: {:?}",
        view.out_w,
        view.out_h,
        t0.elapsed().as_millis(),
        graph.last_passes_run
    );

    // RGBA → RGB
    let rgb: Vec<u8> = frame
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    image::save_buffer(&out, &rgb, view.out_w, view.out_h, image::ColorType::Rgb8).expect("png");
    println!("wrote {out}");
}

fn pollster_block<F: std::future::Future>(f: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(f)
}
