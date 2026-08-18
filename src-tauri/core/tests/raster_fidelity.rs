//! Zero-edit raster (JPEG/PNG) fidelity — the JPEG must not be re-rendered
//! as if it were sensor data.
//!
//! Contract: decode → linear Rec.2020 → `effective_display_look` (passthrough)
//! → sRGB OETF must stay close to the source, and must not invent clipped
//! highlights the source did not have.

use meratech_core::export::{output_transform_look, TargetSpace};
use meratech_core::pipeline::RgbStats;
use meratech_core::raw::{decoder_for, effective_display_look, ImageKind};

fn write_jpeg(path: &std::path::Path, img: &image::RgbImage) {
    img.save(path).unwrap_or_else(|e| panic!("save jpeg: {e}"));
}

fn present_decoded(path: &std::path::Path, user_look: u32) -> (Vec<u8>, u32, u32, ImageKind) {
    let dec = decoder_for(path);
    let out = dec
        .decode_with_profile(path, None)
        .unwrap_or_else(|e| panic!("decode: {e:?}"));
    let look = effective_display_look(out.meta.kind, user_look);
    let enc = output_transform_look(
        &out.working.data,
        out.working.width as u32,
        out.working.height as u32,
        TargetSpace::Srgb,
        false,
        look,
    );
    (enc.rgb8, enc.width, enc.height, out.meta.kind)
}

fn mae(a: &[u8], b: &[u8]) -> f32 {
    assert_eq!(a.len(), b.len());
    let s: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as f64 - *y as f64).abs())
        .sum();
    (s / a.len() as f64) as f32
}

#[test]
fn srgb_midgray_jpeg_zero_edit_stays_midgray() {
    let img = image::RgbImage::from_pixel(16, 12, image::Rgb([128, 128, 128]));
    let path = std::env::temp_dir().join("meraraw-fid-midgray.jpg");
    write_jpeg(&path, &img);
    let (rgb, w, h, kind) = present_decoded(&path, 1); // user look = Camera
    let _ = std::fs::remove_file(&path);
    assert_eq!(kind, ImageKind::Rendered);
    assert_eq!((w, h), (16, 12));
    // JPEG quant + our sRGB EOTF/OETF round-trip: stay near 128, not blown.
    let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
    for px in rgb.chunks_exact(3) {
        r += px[0] as u32;
        g += px[1] as u32;
        b += px[2] as u32;
    }
    let n = w * h;
    let (r, g, b) = (r / n, g / n, b / n);
    assert!(
        (r as i32 - 128).abs() < 18,
        "mid-gray JPEG presented as {r} (Camera look must not lift it)"
    );
    assert!((r as i32 - g as i32).abs() <= 2 && (g as i32 - b as i32).abs() <= 2);
}

#[test]
fn high_key_jpeg_does_not_invent_clips() {
    // Half the frame is 250 (bright, not clipped); half is 80.
    let img = image::RgbImage::from_fn(32, 24, |x, _| {
        if x < 16 {
            image::Rgb([250, 250, 250])
        } else {
            image::Rgb([80, 80, 80])
        }
    });
    let path = std::env::temp_dir().join("meraraw-fid-highkey.jpg");
    write_jpeg(&path, &img);
    let src = image::open(&path).unwrap().to_rgb8();
    let src_stats = RgbStats::from_rgb8(&src.into_raw());
    let (rgb, _, _, _) = present_decoded(&path, 1);
    let _ = std::fs::remove_file(&path);
    let out_stats = RgbStats::from_rgb8(&rgb);
    assert!(
        out_stats.clip_hi_pct <= src_stats.clip_hi_pct + 2.0,
        "zero-edit JPEG invented highlight clips: src={:.2}% out={:.2}%",
        src_stats.clip_hi_pct,
        out_stats.clip_hi_pct
    );
}

#[test]
fn dark_jpeg_is_not_lifted_by_camera_look() {
    let img = image::RgbImage::from_pixel(16, 12, image::Rgb([32, 32, 32]));
    let path = std::env::temp_dir().join("meraraw-fid-dark.jpg");
    write_jpeg(&path, &img);
    let (rgb, _, _, _) = present_decoded(&path, 1);
    let _ = std::fs::remove_file(&path);
    let mean = rgb.iter().map(|v| *v as u32).sum::<u32>() / rgb.len() as u32;
    assert!(
        mean < 55,
        "dark JPEG was lifted (Camera punchy leftover?): mean={mean}"
    );
}

#[test]
fn saturated_primaries_keep_hue() {
    let colors = [
        ([220, 20, 20], "red"),
        ([20, 200, 20], "green"),
        ([20, 20, 220], "blue"),
        ([220, 220, 20], "yellow"),
    ];
    for (rgb_in, name) in colors {
        let img = image::RgbImage::from_pixel(8, 8, image::Rgb(rgb_in));
        let path = std::env::temp_dir().join(format!("meraraw-fid-{name}.jpg"));
        write_jpeg(&path, &img);
        let (out, _, _, _) = present_decoded(&path, 0);
        let _ = std::fs::remove_file(&path);
        let (r, g, b) = (out[0], out[1], out[2]);
        match name {
            "red" => assert!(r > g && r > b, "{name} {r},{g},{b}"),
            "green" => assert!(g > r && g > b, "{name} {r},{g},{b}"),
            "blue" => assert!(b > r && b > g, "{name} {r},{g},{b}"),
            "yellow" => assert!(r > 150 && g > 150 && b < r && b < g, "{name} {r},{g},{b}"),
            _ => {}
        }
    }
}

#[test]
fn untagged_jpeg_uses_srgb_fallback() {
    let img = image::RgbImage::from_pixel(8, 8, image::Rgb([200, 40, 40]));
    let path = std::env::temp_dir().join("meraraw-fid-untagged.jpg");
    write_jpeg(&path, &img);
    let dec = decoder_for(&path);
    let meta = dec.metadata(&path).unwrap();
    let out = dec.decode_with_profile(&path, None).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(out.meta.kind, ImageKind::Rendered);
    assert_eq!(meta.input_color_space.as_deref(), Some("sRGB"));
    assert!(out.meta.camera_profile.is_none());
    assert!(out.meta.available_profiles.is_empty());
}

#[test]
fn dcp_curve_on_decoded_jpeg_is_the_burnout_passthrough_is_not() {
    let img = image::RgbImage::from_fn(24, 16, |x, y| {
        image::Rgb([
            (180 + x * 3).min(250) as u8,
            (160 + y * 4).min(250) as u8,
            140,
        ])
    });
    let path = std::env::temp_dir().join("meraraw-fid-dcp-burn.jpg");
    write_jpeg(&path, &img);
    let src = image::open(&path).unwrap().to_rgb8().into_raw();
    let dec = decoder_for(&path);
    let decoded = dec.decode_with_profile(&path, None).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(effective_display_look(decoded.meta.kind, 1), 3);
    let pass = output_transform_look(
        &decoded.working.data,
        decoded.working.width as u32,
        decoded.working.height as u32,
        TargetSpace::Srgb,
        false,
        3,
    );
    let curved: Vec<f32> = decoded
        .working
        .data
        .chunks_exact(3)
        .flat_map(|px| {
            meratech_core::curve::ProfileToneCurve::adobe_default()
                .apply_rgb([px[0], px[1], px[2]])
        })
        .collect();
    let burnt = output_transform_look(
        &curved,
        decoded.working.width as u32,
        decoded.working.height as u32,
        TargetSpace::Srgb,
        false,
        3,
    );
    let src_stats = RgbStats::from_rgb8(&src);
    let pass_stats = RgbStats::from_rgb8(&pass.rgb8);
    let burnt_stats = RgbStats::from_rgb8(&burnt.rgb8);
    assert!(
        pass_stats.clip_hi_pct <= src_stats.clip_hi_pct + 3.0,
        "passthrough invented clips: src={:.2} pass={:.2}",
        src_stats.clip_hi_pct,
        pass_stats.clip_hi_pct
    );
    assert!(
        burnt_stats.clip_hi_pct > pass_stats.clip_hi_pct + 5.0
            || mae(&burnt.rgb8, &pass.rgb8) > 20.0,
        "Adobe DCP curve on JPEG working buffer is the burnout; burnt_clip={:.1} pass_clip={:.1} mae={:.1}",
        burnt_stats.clip_hi_pct,
        pass_stats.clip_hi_pct,
        mae(&burnt.rgb8, &pass.rgb8)
    );
}
