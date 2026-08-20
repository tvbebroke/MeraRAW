//! Headless decode sanity check (color-harness seed, test strategy §2).
//! Usage: cargo run -p meratech-core --release --example decode_check -- <raw> [out.png]
//! Prints metadata + channel stats; writes a small display-transformed PNG
//! (same math as the GPU display shader, CPU-side) for eyeball verification.

use meratech_core::color::{mat_mul, mat_vec, REC2020_TO_XYZ, XYZ_TO_SRGB};
use meratech_core::raw::{Decoder, RawlerDecoder};
use std::path::PathBuf;

fn oetf_srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

fn view_transform(c: [f32; 3]) -> [f32; 3] {
    // pinned neutral view: Reinhard-extended, Lw=4 (display.wgsl mirror)
    let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    if l <= 1e-8 {
        return [0.0; 3];
    }
    let lw = 4.0f32;
    let ld = l * (1.0 + l / (lw * lw)) / (1.0 + l);
    let k = ld / l;
    [c[0] * k, c[1] * k, c[2] * k]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().expect("usage: decode_check <raw> [out.png]"));
    let out = args
        .next()
        .unwrap_or_else(|| "/tmp/decode_check.png".into());

    let dec = RawlerDecoder::default();
    assert!(dec.probe(&path), "probe rejected {}", path.display());

    let t0 = std::time::Instant::now();
    let img = dec.decode(&path).expect("decode");
    let decode_ms = t0.elapsed().as_millis();

    let m = &img.meta;
    println!("camera:   {} {}", m.camera_make, m.camera_model);
    println!("lens:     {:?}", m.lens);
    println!(
        "iso/shutter/f: {:?} {:?} {:?}",
        m.iso, m.shutter, m.aperture
    );
    println!(
        "dims:     {}x{} (orientation {})",
        m.width, m.height, m.orientation
    );
    println!(
        "as-shot wb: {:?}  est CCT: {:?}",
        m.as_shot_wb, m.estimated_cct
    );
    println!("decode:   {decode_ms} ms");

    let buf = &img.working;
    let n = (buf.width * buf.height) as f32;
    let mut mean = [0.0f64; 3];
    let mut maxv = [f32::MIN; 3];
    let mut minv = [f32::MAX; 3];
    let mut over1 = 0usize;
    for px in buf.data.chunks_exact(3) {
        for c in 0..3 {
            mean[c] += px[c] as f64;
            maxv[c] = maxv[c].max(px[c]);
            minv[c] = minv[c].min(px[c]);
        }
        if px.iter().any(|v| *v > 1.0) {
            over1 += 1;
        }
    }
    let mean = mean.map(|v| (v / n as f64) as f32);
    println!("mean RGB (linear Rec.2020): {mean:?}");
    println!("min: {minv:?}  max: {maxv:?}");
    println!("pixels >1.0 (headroom): {:.3}%", 100.0 * over1 as f32 / n);

    // sanity tripwires
    assert!(
        mean.iter().all(|v| *v > 0.0005 && *v < 4.0),
        "implausible mean {mean:?}"
    );
    assert!(minv.iter().all(|v| *v >= 0.0), "negatives leaked");
    let gray_ratio_rg = mean[0] / mean[1];
    let gray_ratio_bg = mean[2] / mean[1];
    println!("scene avg R/G={gray_ratio_rg:.3} B/G={gray_ratio_bg:.3} (gross cast check; scene-dependent)");

    // small display-transformed PNG (matches GPU shader math)
    let small = buf.downscale_to(1200);
    let rec2020_to_srgb = mat_mul(&XYZ_TO_SRGB, &REC2020_TO_XYZ);
    let mut png = vec![0u8; small.width * small.height * 3];
    for (i, px) in small.data.chunks_exact(3).enumerate() {
        let c = mat_vec(&rec2020_to_srgb, [px[0], px[1], px[2]]);
        let c = view_transform([c[0].max(0.0), c[1].max(0.0), c[2].max(0.0)]);
        for ch in 0..3 {
            png[i * 3 + ch] = (oetf_srgb(c[ch].clamp(0.0, 1.0)) * 255.0).round() as u8;
        }
    }
    image::save_buffer(
        &out,
        &png,
        small.width as u32,
        small.height as u32,
        image::ColorType::Rgb8,
    )
    .expect("write png");
    println!("wrote {out}");
}
