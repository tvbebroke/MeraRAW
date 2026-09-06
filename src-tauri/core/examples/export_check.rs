//! End-to-end export validation (spec 7.7): decode → edit doc → tiled
//! full-res linear render → output transform → JPEG + ICC. The graduation
//! ARW to a web JPEG is the acceptance bar.
//! Usage: cargo run -p meratech-core --release --example export_check -- <raw> [out_dir]

use meratech_core::doc::EditDoc;
use meratech_core::export::{
    encode_and_write, output_sharpen8, output_transform, resize_linear, ExportFormat,
    ExportSettings, TargetSpace,
};
use meratech_core::gpu::display::{upload_working_texture, ViewParams};
use meratech_core::gpu::GpuContext;
use meratech_core::graph::RenderGraph;
use meratech_core::message::DecodedPayload;
use meratech_core::ops::{apply_op, Op};
use meratech_core::raw::{Decoder, RawlerDecoder};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().expect("usage: export_check <raw> [out_dir]"));
    let out_dir = args
        .next()
        .unwrap_or_else(|| "/tmp/meratech-exports".into());

    let dec = RawlerDecoder::default();
    let img = dec.decode(&path).expect("decode");
    let payload = DecodedPayload::from_decoded(img);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let gpu = rt.block_on(GpuContext::init()).expect("gpu");
    let tex = upload_working_texture(&gpu, &payload.rgba_f16, payload.width, payload.height);
    let tv = tex.create_view(&Default::default());
    let mut graph = RenderGraph::new(&gpu);

    // the standard P3 edit (same recipe as render_check)
    let mut doc = EditDoc::new(&path.to_string_lossy());
    for (p, v) in [
        ("exposure.stops", json!(0.25)),
        ("tone_curve.contrast", json!(30)),
        ("tone_curve.highlights", json!(-35)),
        ("tone_curve.shadows", json!(20)),
        ("hsl.orange.lum", json!(10)),
        ("hsl.green.sat", json!(-25)),
        ("color_grade.highlights_hue", json!(60)),
        ("color_grade.highlights_sat", json!(14)),
        ("color_grade.perceptual_sat", json!(12)),
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

    // tiled full-res linear render
    const TILE: u32 = 1024;
    let (w, h) = (payload.width, payload.height);
    let cct = payload.meta.estimated_cct.unwrap_or(5200.0);
    let mut full = vec![0.0f32; (w as usize) * (h as usize) * 3];
    let t0 = std::time::Instant::now();
    for ty in (0..h).step_by(TILE as usize) {
        for tx in (0..w).step_by(TILE as usize) {
            let tw = TILE.min(w - tx);
            let th = TILE.min(h - ty);
            let view = ViewParams {
                out_w: tw,
                out_h: th,
                scale: Some(1.0),
                center_x: (tx as f32 + tw as f32 / 2.0) / w as f32,
                center_y: (ty as f32 + th as f32 / 2.0) / h as f32,
                crop_preview: false,
            };
            let tile = graph
                .render_linear_tile(
                    &gpu,
                    &tv,
                    w,
                    h,
                    &view,
                    &doc,
                    cct,
                    &HashMap::new(),
                    None,
                    None,
                )
                .expect("tile");
            for row in 0..th as usize {
                let src = row * tw as usize * 3;
                let dst = ((ty as usize + row) * w as usize + tx as usize) * 3;
                full[dst..dst + tw as usize * 3].copy_from_slice(&tile[src..src + tw as usize * 3]);
            }
        }
    }
    println!(
        "tiled linear render {}x{} in {} ms",
        w,
        h,
        t0.elapsed().as_millis()
    );

    for (target, name) in [
        (TargetSpace::Srgb, "srgb"),
        (TargetSpace::DisplayP3, "p3"),
        (TargetSpace::AdobeRgb, "adobe"),
    ] {
        let settings = ExportSettings {
            format: ExportFormat::Jpeg,
            target,
            quality: 90,
            max_dim: Some(2048),
            sharpen: 35.0,
            dest_dir: format!("{out_dir}/{name}"),
            metadata_policy: Default::default(),
            strip_metadata: false,
            copyright: None,
            video_clip: false,
            output_stem: None,
            video_in: None,
            video_out: None,
            video_audio: true,
        };
        let (lin, rw, rh) = resize_linear(full.clone(), w, h, 2048);
        let mut enc = output_transform(&lin, rw, rh, target, false, false);
        output_sharpen8(&mut enc.rgb8, rw, rh, settings.sharpen);
        let p = encode_and_write(&enc, &settings, &path.to_string_lossy(), &payload.meta)
            .expect("write");
        let bytes = std::fs::read(&p).unwrap();
        let has_icc = bytes.windows(12).any(|w| w == b"ICC_PROFILE\0");
        println!(
            "{name}: {} ({} KB, ICC={})",
            p.display(),
            bytes.len() / 1024,
            has_icc
        );
        assert!(has_icc, "{name} export missing ICC profile");
        let decoded = image::load_from_memory(&bytes).expect("exported jpeg decodes");
        assert_eq!(decoded.width().max(decoded.height()), 2048);
    }
    println!("export check OK");
}
