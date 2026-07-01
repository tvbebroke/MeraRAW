//! Decode any supported file through the real dispatch (decoder_for) and
//! print metadata + working-buffer sanity. Verifies the rendered-image path
//! (JPEG/PNG/TIFF/WebP/HEIC/JXL/PSD) end-to-end, off the RAW path.
//! Usage: cargo run -p meratech-core --example decode_any -- <file>

use meratech_core::raw::decoder_for;
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from(std::env::args().nth(1).expect("usage: decode_any <file>"));
    let dec = decoder_for(&path);
    let img = dec.decode_with_profile(&path, None).expect("decode");
    let m = &img.meta;
    let n = img.working.data.len();
    let mut mean = [0.0f64; 3];
    let (mut mn, mut mx) = (f32::INFINITY, f32::NEG_INFINITY);
    for px in img.working.data.chunks_exact(3) {
        for c in 0..3 {
            mean[c] += px[c] as f64;
            mn = mn.min(px[c]);
            mx = mx.max(px[c]);
        }
    }
    let cnt = (n / 3) as f64;
    let mean = mean.map(|v| v / cnt);
    println!(
        "OK  {}  kind={:?} format={} depth={}bit  {}x{}  buf={} floats  mean(lin Rec2020)=[{:.4},{:.4},{:.4}]  range=[{:.4},{:.4}]",
        path.file_name().unwrap().to_string_lossy(),
        m.kind,
        m.format,
        m.bit_depth,
        img.working.width,
        img.working.height,
        n,
        mean[0], mean[1], mean[2],
        mn, mx,
    );
    assert!(mn >= 0.0, "negative values leaked into working buffer");
    assert!(n == img.working.width * img.working.height * 3, "buffer size mismatch");
}
