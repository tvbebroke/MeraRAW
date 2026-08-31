//! Decode + Camera-look render with autoloaded DCP; print stats vs optional refs.
//! Usage:
//!   cargo run -p meratech-core --release --example color_compare -- <raw> [out.png]

use meratech_core::doc::EditDoc;
use meratech_core::gpu::display::{upload_working_texture, ViewParams};
use meratech_core::gpu::GpuContext;
use meratech_core::graph::RenderGraph;
use meratech_core::message::DecodedPayload;
use meratech_core::profile::{choose_profile, load_dcp_profile, resolve_profile_file, ProfileIndex};
use meratech_core::raw::{Decoder, Demosaic, RawlerDecoder};
use meratech_core::sidecar;
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let use_sidecar = args.iter().any(|a| a == "--app");
    let preview_only = args.iter().any(|a| a == "--preview-only");
    args.retain(|a| a != "--app" && a != "--preview-only");
    let path = PathBuf::from(args.first().expect("usage: color_compare [--app] [--preview-only] <raw> [out.png]"));
    let out = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| format!("/tmp/mera_{}.png", path.file_stem().unwrap().to_string_lossy()));

    let index = ProfileIndex::embedded();
    let dec = RawlerDecoder::default();
    if preview_only {
        let prev = dec.embedded_preview(&path, 1200).expect("preview");
        let Some((rgba, w, h)) = prev else {
            eprintln!("no embedded preview");
            return;
        };
        let rgb: Vec<u8> = rgba
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        let (mean8, sat, _) = rgb8_stats(&rgb, w, h);
        println!(
            "embedded JPEG RGB8 mean={:?} sat={:.3}",
            mean8, sat
        );
        image::save_buffer(&out, &rgb, w, h, image::ColorType::Rgb8).expect("png");
        println!("wrote {out}");
        return;
    }
    let meta = dec.metadata(&path).expect("metadata");
    let chosen = choose_profile(&meta, &index, None);
    let dcp_path = chosen
        .as_ref()
        .and_then(|p| resolve_profile_file(&p.file));
    println!("camera: {} {}", meta.camera_make, meta.camera_model);
    println!("mode: {}", if use_sidecar { "app (sidecar)" } else { "stock" });
    println!(
        "profile: {:?}",
        chosen.as_ref().map(|p| p.file.as_str())
    );
    println!("dcp path: {:?}", dcp_path.as_ref().map(|p| p.display().to_string()));

    let doc = if use_sidecar {
        sidecar::load_edits(&path)
            .ok()
            .flatten()
            .unwrap_or_else(|| EditDoc::new(&path.to_string_lossy()))
    } else {
        EditDoc::new(&path.to_string_lossy())
    };
    let demosaic = Demosaic::parse_or_default(doc.meta.demosaic.as_deref());
    println!("demosaic: {}", demosaic.name());
    let img = dec
        .decode_with_options(&path, dcp_path.as_deref(), demosaic)
        .expect("decode");
    let dcp = load_dcp_profile(&img.meta, chosen.as_ref())
        .or_else(|| {
            Some(meratech_core::profile::DcpProfile::builtin_standard(
                &img.meta.camera_make,
                &img.meta.camera_model,
            ))
        })
        .map(Arc::new);
    if let Some(d) = dcp.as_ref() {
        let look = d.look_data();
        println!(
            "dcp look: maps={} look_tbl={} tone_embedded={} baseline_gain={:.3}",
            look.map1.is_some() || look.map2.is_some(),
            look.look.is_some(),
            d.tone_curve_embedded(),
            look.baseline_gain
        );
    } else {
        println!("dcp look: NONE");
    }

    let payload = DecodedPayload::from_decoded(img);
    let mean = {
        let n = (payload.width * payload.height) as f64;
        let mut s = [0.0f64; 3];
        let pix: &[half::f16] = bytemuck::cast_slice(&payload.rgba_f16);
        for px in pix.chunks_exact(4) {
            s[0] += px[0].to_f32() as f64;
            s[1] += px[1].to_f32() as f64;
            s[2] += px[2].to_f32() as f64;
        }
        [s[0] / n, s[1] / n, s[2] / n]
    };
    println!(
        "linear Rec.2020 mean: [{:.4}, {:.4}, {:.4}]",
        mean[0], mean[1], mean[2]
    );

    let gpu = pollster_block(GpuContext::init()).expect("gpu");
    let tex = upload_working_texture(&gpu, &payload.rgba_f16, payload.width, payload.height);
    let tv = tex.create_view(&Default::default());
    let mut graph = RenderGraph::new(&gpu);
    graph.set_look(1); // Camera
    let scale = 1200.0 / payload.width.max(payload.height) as f32;
    let view = ViewParams {
        out_w: (payload.width as f32 * scale) as u32,
        out_h: (payload.height as f32 * scale) as u32,
        scale: Some(scale),
        center_x: 0.5,
        center_y: 0.5,
        crop_preview: false,
    };
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
            dcp.as_deref(),
            None,
        )
        .expect("render");

    let rgb: Vec<u8> = frame
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let (mean8, sat, p95) = rgb8_stats(&rgb, view.out_w, view.out_h);
    println!(
        "Camera look RGB8 mean={:?} p95={:?} sat={:.3}",
        mean8, p95, sat
    );
    image::save_buffer(&out, &rgb, view.out_w, view.out_h, image::ColorType::Rgb8).expect("png");
    println!("wrote {out}");
}

fn rgb8_stats(rgb: &[u8], _w: u32, _h: u32) -> ([f32; 3], f32, [f32; 3]) {
    let n = rgb.len() / 3;
    let step = (n / 50_000).max(1);
    let mut sum = [0.0f64; 3];
    let mut sat_sum = 0.0f64;
    let mut vals: [Vec<f32>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut count = 0usize;
    for i in (0..n).step_by(step) {
        let r = rgb[i * 3] as f32;
        let g = rgb[i * 3 + 1] as f32;
        let b = rgb[i * 3 + 2] as f32;
        sum[0] += r as f64;
        sum[1] += g as f64;
        sum[2] += b as f64;
        let mx = r.max(g).max(b);
        let mn = r.min(g).min(b);
        if mx > 1.0 {
            sat_sum += ((mx - mn) / mx) as f64;
        }
        vals[0].push(r);
        vals[1].push(g);
        vals[2].push(b);
        count += 1;
    }
    let mean = [
        (sum[0] / count as f64) as f32,
        (sum[1] / count as f64) as f32,
        (sum[2] / count as f64) as f32,
    ];
    let sat = (sat_sum / count as f64) as f32;
    let pct = |v: &mut Vec<f32>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[((v.len() as f32) * 0.95) as usize]
    };
    let p95 = [pct(&mut vals[0]), pct(&mut vals[1]), pct(&mut vals[2])];
    (mean, sat, p95)
}

fn pollster_block<F: std::future::Future>(f: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(f)
}
