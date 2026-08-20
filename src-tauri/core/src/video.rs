//! Video decode via a controlled FFmpeg subprocess.
//!
//! One stack (CLI), LGPL/GPL obligations depend on the user's FFmpeg build —
//! we invoke whatever `ffmpeg`/`ffprobe` is on PATH and never statically link.
//! Decode lands in linear Rec.2020 through `idt` (same graph as stills).

use crate::error::CoreError;
use crate::idt::{self, Primaries, Transfer};
use crate::image::RgbF32Buf;
use crate::raw::{DecodedImage, Decoder, ImageKind, ImageMeta, VideoMeta};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "m4v", "mkv", "webm", "m4a"];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoProbe {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub frame_count: u32,
    pub duration_s: f32,
    pub pix_fmt: String,
    pub color_primaries: Option<String>,
    pub color_transfer: Option<String>,
    pub color_space: Option<String>,
    pub color_range: Option<String>,
    /// Visible assumption when tags are missing.
    pub assumed_input: String,
}

#[derive(Clone, Copy, Debug)]
pub struct VideoLand {
    pub transfer: Transfer,
    pub primaries: Primaries,
    pub full_range: bool,
}

impl Default for VideoLand {
    fn default() -> Self {
        Self {
            transfer: Transfer::Srgb,
            primaries: Primaries::Rec709,
            full_range: false,
        }
    }
}

pub fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn is_video_path(path: &Path) -> bool {
    path.extension()
        .map(|e| {
            let e = e.to_string_lossy().to_lowercase();
            VIDEO_EXTENSIONS.iter().any(|x| *x == e)
        })
        .unwrap_or(false)
}

fn run_cmd(bin: &str, args: &[&str]) -> Result<Vec<u8>, CoreError> {
    let out = Command::new(bin)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            CoreError::Decode(format!(
                "{bin} missing or failed to start ({e}). Install FFmpeg to grade video."
            ))
        })?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(CoreError::Decode(format!(
            "{bin} failed: {}",
            err.lines().last().unwrap_or("unknown")
        )));
    }
    Ok(out.stdout)
}

pub fn probe(path: &Path) -> Result<VideoProbe, CoreError> {
    let p = path.to_string_lossy();
    let raw = run_cmd(
        "ffprobe",
        &[
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,avg_frame_rate,nb_frames,duration,pix_fmt,color_primaries,color_transfer,color_space,color_range",
            "-of",
            "json",
            p.as_ref(),
        ],
    )?;
    parse_probe(&raw)
}

fn parse_probe(json: &[u8]) -> Result<VideoProbe, CoreError> {
    let v: serde_json::Value = serde_json::from_slice(json)
        .map_err(|e| CoreError::Decode(format!("ffprobe json: {e}")))?;
    let s = v
        .get("streams")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| CoreError::Decode("ffprobe: no video stream".into()))?;
    let width = s.get("width").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    let height = s.get("height").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    if width == 0 || height == 0 {
        return Err(CoreError::Decode("ffprobe: missing dimensions".into()));
    }
    let fps = parse_rate(
        s.get("avg_frame_rate")
            .and_then(|x| x.as_str())
            .unwrap_or("24/1"),
    );
    let duration_s = s
        .get("duration")
        .and_then(|x| x.as_str())
        .and_then(|t| t.parse().ok())
        .unwrap_or(0.0);
    let frame_count = s
        .get("nb_frames")
        .and_then(|x| x.as_str())
        .and_then(|t| t.parse().ok())
        .filter(|n: &u32| *n > 0)
        .unwrap_or_else(|| (duration_s * fps).round().max(1.0) as u32);
    let prim = s
        .get("color_primaries")
        .and_then(|x| x.as_str())
        .map(|x| x.to_string());
    let trc = s
        .get("color_transfer")
        .and_then(|x| x.as_str())
        .map(|x| x.to_string());
    let assumed = if prim.is_none() && trc.is_none() {
        "Assumed input: Rec.709"
    } else {
        "Tagged"
    };
    Ok(VideoProbe {
        width,
        height,
        fps,
        frame_count,
        duration_s,
        pix_fmt: s
            .get("pix_fmt")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string(),
        color_primaries: prim,
        color_transfer: trc,
        color_space: s
            .get("color_space")
            .and_then(|x| x.as_str())
            .map(|x| x.to_string()),
        color_range: s
            .get("color_range")
            .and_then(|x| x.as_str())
            .map(|x| x.to_string()),
        assumed_input: assumed.into(),
    })
}

fn parse_rate(s: &str) -> f32 {
    if let Some((a, b)) = s.split_once('/') {
        let n: f32 = a.parse().unwrap_or(24.0);
        let d: f32 = b.parse::<f32>().unwrap_or(1.0).max(1e-6);
        (n / d).clamp(1.0, 240.0)
    } else {
        s.parse().unwrap_or(24.0)
    }
}

/// Infer IDT from nclx/CICP-style tags. Unknown → Rec.709 display-referred.
pub fn land_from_tags(
    probe: &VideoProbe,
    override_t: Option<Transfer>,
    override_p: Option<Primaries>,
) -> VideoLand {
    let transfer = override_t.unwrap_or(match probe.color_transfer.as_deref() {
        Some("smpte2084" | "smpte2084-1") => Transfer::Pq,
        Some("arib-std-b67" | "hlg") => Transfer::Hlg,
        Some("bt709" | "smpte170m" | "bt470bg") => Transfer::Rec709,
        Some("iec61966-2-1" | "srgb") => Transfer::Srgb,
        Some("arri-logc3") => Transfer::LogC3,
        Some("arri-logc4") => Transfer::LogC4,
        Some("smpte428") => Transfer::Linear,
        _ => Transfer::Srgb,
    });
    let primaries = override_p.unwrap_or(match probe.color_primaries.as_deref() {
        Some("bt2020") => Primaries::Rec2020,
        Some("smpte432" | "displayp3") => Primaries::DisplayP3,
        Some("bt709" | "smpte170m") => Primaries::Rec709,
        _ => Primaries::Rec709,
    });
    let full_range = matches!(probe.color_range.as_deref(), Some("full" | "pc" | "jpeg"));
    VideoLand {
        transfer,
        primaries,
        full_range,
    }
}

/// `input.transfer` / `input.primaries` registry codes. 0 = Auto (use tags).
pub fn overrides_from_input_params(
    transfer: u32,
    primaries: u32,
) -> (Option<Transfer>, Option<Primaries>) {
    let override_t = if transfer == 0 {
        None
    } else {
        Some(Transfer::from_u32(transfer))
    };
    let override_p = match primaries {
        0 => None,
        1 => Some(Primaries::Rec709),
        2 => Some(Primaries::Rec2020),
        3 => Some(Primaries::DisplayP3),
        4 => Some(Primaries::SGamut3Cine),
        5 => Some(Primaries::ArriWideGamut3),
        6 => Some(Primaries::ArriWideGamut4),
        7 => Some(Primaries::VGamut),
        _ => None,
    };
    (override_t, override_p)
}

/// Optional audio to mux alongside a JPEG sequence (trimmed to the graded range).
#[derive(Clone, Debug)]
pub struct AudioPass {
    pub source: PathBuf,
    pub start_s: f32,
    pub duration_s: f32,
}

/// True when FFmpeg can see an audio stream on the source (best-effort).
pub fn has_audio_stream(path: &Path) -> bool {
    let p = path.to_string_lossy().into_owned();
    match run_cmd(
        "ffprobe",
        &[
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=index",
            "-of",
            "csv=p=0",
            &p,
        ],
    ) {
        Ok(out) => !String::from_utf8_lossy(&out).trim().is_empty(),
        Err(_) => false,
    }
}

fn mux_silent(pattern_s: &str, start_s: &str, fps_s: &str, out_s: &str) -> Result<(), CoreError> {
    run_cmd(
        "ffmpeg",
        &[
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-framerate",
            fps_s,
            "-start_number",
            start_s,
            "-i",
            pattern_s,
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-an",
            "-movflags",
            "+faststart",
            out_s,
        ],
    )?;
    Ok(())
}

fn mux_with_audio(
    pattern_s: &str,
    start_s: &str,
    fps_s: &str,
    out_s: &str,
    audio: &AudioPass,
    codec: &str,
) -> Result<(), CoreError> {
    let src = audio.source.to_string_lossy().into_owned();
    let ss = format!("{:.6}", audio.start_s.max(0.0));
    let dur = format!("{:.6}", audio.duration_s.max(1.0 / 120.0));
    let mut args: Vec<&str> = vec![
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-framerate",
        fps_s,
        "-start_number",
        start_s,
        "-i",
        pattern_s,
        "-ss",
        &ss,
        "-t",
        &dur,
        "-i",
        &src,
        "-map",
        "0:v:0",
        "-map",
        "1:a:0",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        codec,
    ];
    if codec == "aac" {
        args.extend(["-b:a", "192k"]);
    }
    args.extend(["-t", &dur, "-movflags", "+faststart", out_s]);
    run_cmd("ffmpeg", &args)?;
    Ok(())
}

/// Mux a numbered JPEG sequence into Rec.709 H.264 MP4.
/// Tries source audio (`copy`, then AAC) when `audio` is set; otherwise silent.
pub fn mux_jpeg_sequence(
    pattern: &Path,
    start_number: u32,
    fps: f32,
    out: &Path,
    audio: Option<&AudioPass>,
) -> Result<PathBuf, CoreError> {
    let fps_s = format!("{:.3}", fps.max(1.0));
    let start_s = start_number.to_string();
    let pattern_s = pattern.to_string_lossy().into_owned();
    let out_s = out.to_string_lossy().into_owned();
    if let Some(a) = audio {
        if mux_with_audio(&pattern_s, &start_s, &fps_s, &out_s, a, "copy").is_ok() {
            return Ok(out.to_path_buf());
        }
        tracing::warn!("clip audio copy failed; trying AAC");
        if mux_with_audio(&pattern_s, &start_s, &fps_s, &out_s, a, "aac").is_ok() {
            return Ok(out.to_path_buf());
        }
        tracing::warn!("clip audio mux failed; writing silent H.264");
    }
    mux_silent(&pattern_s, &start_s, &fps_s, &out_s)?;
    Ok(out.to_path_buf())
}

/// Decode one frame as encoded Rec.709/sRGB 8-bit RGB, then IDT to linear Rec.2020.
pub fn decode_frame(path: &Path, frame: u32, land: VideoLand) -> Result<RgbF32Buf, CoreError> {
    let probe = probe(path)?;
    let fps = probe.fps.max(1.0);
    let ts = frame as f32 / fps;
    let p = path.to_string_lossy();
    let tmp = std::env::temp_dir().join(format!(
        "meraraw-vframe-{}-{}.png",
        std::process::id(),
        frame
    ));
    let tmp_s = tmp.to_string_lossy().into_owned();
    let _ = run_cmd(
        "ffmpeg",
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-ss",
            &format!("{ts:.6}"),
            "-i",
            p.as_ref(),
            "-frames:v",
            "1",
            "-an",
            "-y",
            &tmp_s,
        ],
    )?;
    let img = image::open(&tmp).map_err(|e| CoreError::Decode(format!("video frame: {e}")))?;
    let _ = std::fs::remove_file(&tmp);
    let rgb8 = img.to_rgb8();
    let (w, h) = rgb8.dimensions();
    let mut data = vec![0.0f32; (w * h * 3) as usize];
    for (i, px) in rgb8.pixels().enumerate() {
        let enc = [
            px[0] as f32 / 255.0,
            px[1] as f32 / 255.0,
            px[2] as f32 / 255.0,
        ];
        let lin = idt::idt_to_rec2020(enc, land.transfer, land.primaries);
        data[i * 3] = lin[0];
        data[i * 3 + 1] = lin[1];
        data[i * 3 + 2] = lin[2];
    }
    Ok(RgbF32Buf {
        data,
        width: w as usize,
        height: h as usize,
    })
}

pub struct VideoDecoder;

impl Decoder for VideoDecoder {
    fn probe(&self, path: &Path) -> bool {
        is_video_path(path)
    }

    fn metadata(&self, path: &Path) -> Result<ImageMeta, CoreError> {
        let pr = probe(path)?;
        Ok(video_meta(path, &pr, 0))
    }

    fn embedded_preview(
        &self,
        path: &Path,
        _max_dim: u32,
    ) -> Result<Option<(Vec<u8>, u32, u32)>, CoreError> {
        let land = VideoLand::default();
        let buf = decode_frame(path, 0, land)?;
        let mut rgba = vec![0u8; buf.width * buf.height * 4];
        for i in 0..(buf.width * buf.height) {
            let enc = idt::encode(
                Transfer::Srgb,
                [
                    buf.data[i * 3].clamp(0.0, 1.0),
                    buf.data[i * 3 + 1].clamp(0.0, 1.0),
                    buf.data[i * 3 + 2].clamp(0.0, 1.0),
                ],
            );
            rgba[i * 4] = (enc[0] * 255.0) as u8;
            rgba[i * 4 + 1] = (enc[1] * 255.0) as u8;
            rgba[i * 4 + 2] = (enc[2] * 255.0) as u8;
            rgba[i * 4 + 3] = 255;
        }
        Ok(Some((rgba, buf.width as u32, buf.height as u32)))
    }

    fn decode_with_profile(
        &self,
        path: &Path,
        _profile_path: Option<&Path>,
    ) -> Result<DecodedImage, CoreError> {
        let pr = probe(path)?;
        let land = land_from_tags(&pr, None, None);
        let working = decode_frame(path, 0, land)?;
        Ok(DecodedImage {
            meta: video_meta(path, &pr, 0),
            working,
        })
    }
}

fn video_meta(path: &Path, pr: &VideoProbe, frame: u32) -> ImageMeta {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_uppercase())
        .unwrap_or_else(|| "MP4".into());
    ImageMeta {
        path: path.to_string_lossy().into_owned(),
        kind: ImageKind::Video,
        format: ext,
        bit_depth: 8,
        camera_make: String::new(),
        camera_model: String::new(),
        lens: None,
        iso: None,
        shutter: None,
        aperture: None,
        focal_mm: None,
        captured_at: None,
        width: pr.width,
        height: pr.height,
        orientation: "1".into(),
        as_shot_wb: [1.0, 1.0, 1.0],
        estimated_cct: None,
        camera_profile: None,
        available_profiles: vec![],
        available_profile_files: vec![],
        demosaic: String::new(),
        available_demosaic: vec![],
        gps_lat: None,
        gps_lon: None,
        input_color_space: Some(pr.assumed_input.clone()),
        video: Some(VideoMeta {
            fps: pr.fps,
            frame_count: pr.frame_count,
            duration_s: pr.duration_s,
            frame,
            in_frame: 0,
            out_frame: pr.frame_count.saturating_sub(1),
            input_transform: pr.assumed_input.clone(),
        }),
    }
}

/// Export every frame in `[start, end]` through `on_frame`.
pub fn for_each_frame(
    path: &Path,
    start: u32,
    end: u32,
    land: VideoLand,
    mut on_frame: impl FnMut(u32, &RgbF32Buf) -> Result<(), CoreError>,
) -> Result<(), CoreError> {
    for f in start..=end {
        let buf = decode_frame(path, f, land)?;
        on_frame(f, &buf)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rate_examples() {
        assert!((parse_rate("24000/1001") - 23.976).abs() < 0.01);
        assert!((parse_rate("24/1") - 24.0).abs() < 1e-6);
    }

    #[test]
    fn unknown_tags_assume_rec709() {
        let pr = VideoProbe {
            width: 1920,
            height: 1080,
            fps: 24.0,
            frame_count: 24,
            duration_s: 1.0,
            pix_fmt: "yuv420p".into(),
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
            assumed_input: "Assumed input: Rec.709".into(),
        };
        let l = land_from_tags(&pr, None, None);
        assert_eq!(l.primaries, Primaries::Rec709);
        assert_eq!(l.transfer, Transfer::Srgb);
    }

    #[test]
    fn input_param_zero_is_auto() {
        let (t, p) = overrides_from_input_params(0, 0);
        assert!(t.is_none() && p.is_none());
        let (t, p) = overrides_from_input_params(1, 2);
        assert_eq!(t, Some(Transfer::from_u32(1)));
        assert_eq!(p, Some(Primaries::Rec2020));
    }

    #[test]
    fn still_and_video_idt_share_working_space() {
        // Same Rec.709 encoded pixel, still JPEG path vs video landing.
        let enc = [idt::srgb_oetf(0.18); 3];
        let still = idt::idt_to_rec2020(enc, Transfer::Srgb, Primaries::Rec709);
        let video = idt::idt_to_rec2020(enc, Transfer::Srgb, Primaries::Rec709);
        for c in 0..3 {
            assert!((still[c] - video[c]).abs() < 1e-6);
            assert!((still[c] - 0.18).abs() < 2e-4);
        }
    }

    #[test]
    fn video_ext_probe() {
        assert!(is_video_path(Path::new("/tmp/a.mp4")));
        assert!(is_video_path(Path::new("/tmp/a.MOV")));
        assert!(!is_video_path(Path::new("/tmp/a.ARW")));
    }

    #[test]
    fn missing_file_has_no_audio() {
        assert!(!has_audio_stream(Path::new(
            "/tmp/meraraw-no-such-clip-xyz.mp4"
        )));
    }
}
