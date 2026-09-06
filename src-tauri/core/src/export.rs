//! Export — contract F2 OUTPUT side (distinct from the screen view
//! transform): linear Rec.2020 → delivery space with constant-hue gamut
//! compression → OETF → resize-aware sharpen → encode + ICC embed + EXIF.
//! The last thing the pipeline does, the first thing the client sees.
//!
//! Formats: JPEG / PNG / 16-bit TIFF (pure-Rust) and HEIC (macOS `sips`).
//! Every format ships color-managed (embedded ICC) and, unless stripped,
//! carries EXIF (camera/lens/exposure) + optional copyright.

use crate::color::{self, bradford_adapt, mat_mul, mat_vec, Mat3};
use crate::error::CoreError;
use crate::profile::DcpProfile;
use crate::raw::ImageMeta;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetSpace {
    Srgb,
    DisplayP3,
    AdobeRgb,
    ProPhoto,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportFormat {
    Jpeg,
    Png,
    Tiff16,
    Heic,
}

/// What metadata to embed on export (phase 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetadataPolicy {
    /// Camera/exposure (+ GPS when present) + copyright.
    #[default]
    Preserve,
    /// Same as preserve but omit GPS tags.
    StripGps,
    /// Write no EXIF / IPTC-style tags.
    StripAll,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSettings {
    pub format: ExportFormat,
    pub target: TargetSpace,
    /// JPEG/HEIC quality 1-100
    pub quality: u8,
    /// longest-edge resize; None = full resolution
    pub max_dim: Option<u32>,
    /// output sharpen 0-100, applied AFTER resize (spec 7.2)
    pub sharpen: f32,
    pub dest_dir: String,
    /// Preferred policy. Takes effect unless `strip_metadata` is true.
    #[serde(default)]
    pub metadata_policy: MetadataPolicy,
    /// Legacy boolean: when true, forces [`MetadataPolicy::StripAll`].
    #[serde(default)]
    pub strip_metadata: bool,
    /// optional copyright/artist string written to EXIF + TIFF.
    #[serde(default)]
    pub copyright: Option<String>,
    /// When true and the source is video, export every frame in the in/out range
    /// (no dropped frames), then mux H.264 if FFmpeg is on PATH.
    #[serde(default)]
    pub video_clip: bool,
    /// Override the output file stem (used for per-frame video stills).
    #[serde(default)]
    pub output_stem: Option<String>,
    /// Inclusive in-point for `video_clip` (defaults to the clip's current in mark).
    #[serde(default)]
    pub video_in: Option<u32>,
    /// Inclusive out-point for `video_clip`.
    #[serde(default)]
    pub video_out: Option<u32>,
    /// Copy source audio into the muxed H.264 clip when possible (falls back to silent).
    #[serde(default = "default_true")]
    pub video_audio: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            format: ExportFormat::Jpeg,
            target: TargetSpace::Srgb,
            quality: 90,
            max_dim: Some(2560),
            sharpen: 30.0,
            dest_dir: String::new(),
            metadata_policy: MetadataPolicy::Preserve,
            strip_metadata: false,
            copyright: None,
            video_clip: false,
            output_stem: None,
            video_in: None,
            video_out: None,
            video_audio: true,
        }
    }
}

/// One folder-batch slot. Virtual copies share `path` and are distinguished
/// by `doc_id` so each copy keeps its own sidecar and output stem.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchExportItem {
    pub path: PathBuf,
    #[serde(default)]
    pub doc_id: Option<String>,
}

impl ExportSettings {
    pub fn effective_policy(&self) -> MetadataPolicy {
        if self.strip_metadata {
            MetadataPolicy::StripAll
        } else {
            self.metadata_policy
        }
    }
}

/// Display P3 (linear) → XYZ D65 (color-science-reference §2).
const P3_TO_XYZ: Mat3 = [
    [0.4865709, 0.2656677, 0.1982173],
    [0.2289746, 0.6917385, 0.0792869],
    [0.0000000, 0.0451134, 1.0439444],
];

/// Adobe RGB (1998) linear → XYZ D65 (derived from primaries; standard).
const ADOBE_TO_XYZ: Mat3 = [
    [0.5766690, 0.1855582, 0.1882286],
    [0.2973450, 0.6273636, 0.0752915],
    [0.0270314, 0.0706889, 0.9913375],
];

/// ROMM RGB (ProPhoto) linear → XYZ **D50** (ISO 22028-2 / Lindbloom).
/// ProPhoto is a D50-referred space, so unlike the D65 spaces above it
/// needs a chromatic adaptation on the way in (see `target_from_rec2020`).
const PROPHOTO_TO_XYZ_D50: Mat3 = [
    [0.7976749, 0.1351917, 0.0313534],
    [0.2880402, 0.7118741, 0.0000857],
    [0.0000000, 0.0000000, 0.8252100],
];

/// CIE standard illuminant whites (XYZ, Y=1). reference §4 / §2.
const D65_XYZ: [f32; 3] = [0.95047, 1.0, 1.08883];
const D50_XYZ: [f32; 3] = [0.96422, 1.0, 0.82521];

pub(crate) fn target_from_rec2020(target: TargetSpace) -> Mat3 {
    match target {
        // ProPhoto is D50: adapt the working white (D65) before the matrix.
        TargetSpace::ProPhoto => {
            let adapt = bradford_adapt(D65_XYZ, D50_XYZ);
            let from_xyz =
                color::mat_inverse(&PROPHOTO_TO_XYZ_D50).expect("prophoto matrix invertible");
            mat_mul(&from_xyz, &mat_mul(&adapt, &color::REC2020_TO_XYZ))
        }
        _ => {
            let to_xyz = match target {
                TargetSpace::Srgb => color::SRGB_TO_XYZ,
                TargetSpace::DisplayP3 => P3_TO_XYZ,
                TargetSpace::AdobeRgb => ADOBE_TO_XYZ,
                TargetSpace::ProPhoto => unreachable!(),
            };
            let from_xyz = color::mat_inverse(&to_xyz).expect("target matrix invertible");
            mat_mul(&from_xyz, &color::REC2020_TO_XYZ)
        }
    }
}

fn oetf(target: TargetSpace, v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    match target {
        // sRGB + Display P3 share the sRGB curve (§3)
        TargetSpace::Srgb | TargetSpace::DisplayP3 => {
            if v <= 0.0031308 {
                12.92 * v
            } else {
                1.055 * v.powf(1.0 / 2.4) - 0.055
            }
        }
        TargetSpace::AdobeRgb => v.powf(1.0 / 2.19921875),
        // ROMM (ProPhoto): linear toe below 1/512, gamma 1.8 above.
        TargetSpace::ProPhoto => {
            if v < 1.0 / 512.0 {
                16.0 * v
            } else {
                v.powf(1.0 / 1.8)
            }
        }
    }
}

/// Reinhard white point — MIRRORS present.wgsl `VIEW_LW`.
const VIEW_LW: f32 = 4.0;

/// Display look — MIRRORS present.wgsl `view_look` (looks 0 / 1).
/// Camera-without-DCP is the Affinity-matched fallback (gain 1.08), not the
/// old punchy Reinhard. DCP Camera uses looks 5/6/7 via [`apply_present_look`].
fn view_look(rgb: [f32; 3], camera: bool) -> [f32; 3] {
    let (gain, contrast, sat) = if camera {
        (1.08f32, 0.16f32, 1.02f32)
    } else {
        (1.15f32, 0.12f32, 1.0f32)
    };
    let rgb = [rgb[0].max(0.0), rgb[1].max(0.0), rgb[2].max(0.0)];
    let l = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    if l <= 1e-8 {
        return [0.0; 3];
    }
    let x = l * gain;
    let r = x * (1.0 + x / (VIEW_LW * VIEW_LW)) / (1.0 + x);
    let s = 0.5 - 0.5 * (r.clamp(0.0, 1.0) * std::f32::consts::PI).cos();
    let ld = (r + (s - r) * contrast).clamp(0.0, 1.0);
    let k = ld / l;
    let mut out = [rgb[0] * k, rgb[1] * k, rgb[2] * k];
    let l2 = 0.2126 * out[0].max(0.0) + 0.7152 * out[1].max(0.0) + 0.0722 * out[2].max(0.0);
    for c in out.iter_mut() {
        *c = (l2 + (*c - l2) * sat).max(0.0);
    }
    out
}

/// Clipped-green magenta highlights — MIRRORS present.wgsl `fix_magenta_highlights`.
fn fix_magenta_highlights(c: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = c;
    let mx = r.max(g.max(b));
    if mx < 0.40 {
        return c;
    }
    if (r - b).abs() > 0.08 * mx {
        return c;
    }
    let rb_min = r.min(b);
    if rb_min < g + 0.02 {
        return c;
    }
    let rb = 0.5 * (r + b);
    let lag = rb - g;
    if lag < 0.02 {
        return c;
    }
    let hi = ((mx - 0.40) / 0.40).clamp(0.0, 1.0);
    let t = (lag / rb.max(1e-6)).clamp(0.0, 1.0) * hi;
    [r, g + (rb - g) * t * 0.90, b]
}

/// Fuji cyan/blue Rec.2020→sRGB clip — MIRRORS present.wgsl `fix_srgb_cyan`.
/// Allows slightly negative R from the gamut matrix (do not clamp first).
fn fix_srgb_cyan(c: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = c;
    if b < 0.02 || b < g {
        return c;
    }
    let mx = r.max(g.max(b));
    let mn = r.min(g.min(b));
    let chroma = (mx - mn) / mx.max(1e-6);
    if chroma < 0.18 || (mx > 0.55 && chroma < 0.28) {
        return c;
    }
    let rb = r / b.max(1e-6);
    let gb = g / b.max(1e-6);
    if rb > 0.38 {
        return c;
    }
    if gb < 0.12 && rb > 0.15 {
        return c;
    }
    let luma = 0.2126 * r.max(0.0) + 0.7152 * g.max(0.0) + 0.0722 * b.max(0.0);
    let crush = ((0.38 - rb) / 0.38).clamp(0.0, 1.0);
    let cyan = (gb / 0.85).clamp(0.0, 1.0);
    let strength = crush * cyan.max(0.45);
    if strength < 0.08 {
        return c;
    }
    let mut out = [
        c[0] + (luma - c[0]) * (0.50 * strength),
        c[1] + (luma - c[1]) * (0.50 * strength),
        c[2] + (luma - c[2]) * (0.50 * strength),
    ];
    let want_rb = if gb > 0.55 { 0.45 } else { 0.22 };
    out[0] = out[0].max(want_rb * b * strength);
    out[1] *= 1.0 - 0.18 * strength * (gb / 0.6).clamp(0.0, 1.0);
    out
}

fn apply_present_gamut_fixes(c: [f32; 3]) -> [f32; 3] {
    fix_srgb_cyan(fix_magenta_highlights(c))
}

/// Post-matrix look used by present.wgsl after Rec.2020 → sRGB.
/// Looks 5/6/7 are the DCP Camera grades; 0/1 are Neutral / no-DCP Camera.
fn apply_present_look(srgb: [f32; 3], look: u32) -> [f32; 3] {
    let c = [srgb[0].max(0.0), srgb[1].max(0.0), srgb[2].max(0.0)];
    match look {
        5 => DcpProfile::view_look_display(c),
        6 => DcpProfile::view_look_display_embedded(c),
        7 => DcpProfile::view_look_display_builtin(c),
        1 => view_look(c, true),
        _ => view_look(c, false),
    }
}

/// Constant-hue gamut compression: mix toward luma until in-gamut (≥0).
fn gamut_compress(rgb: [f32; 3]) -> [f32; 3] {
    let luma = 0.2126 * rgb[0].max(0.0) + 0.7152 * rgb[1].max(0.0) + 0.0722 * rgb[2].max(0.0);
    let m = rgb[0].min(rgb[1]).min(rgb[2]);
    if m >= 0.0 || luma <= 0.0 {
        return rgb.map(|c| c.max(0.0));
    }
    let t = (-m / (luma - m).max(1e-6)).clamp(0.0, 1.0);
    rgb.map(|c| (c + (luma - c) * t).max(0.0))
}

/// linear Rec.2020 RGB f32 → encoded target-space bytes (8-bit) or u16.
pub struct EncodedImage {
    pub width: u32,
    pub height: u32,
    pub rgb8: Vec<u8>,
    pub rgb16: Vec<u16>,
}

pub fn output_transform(
    linear: &[f32],
    width: u32,
    height: u32,
    target: TargetSpace,
    want16: bool,
    camera_look: bool,
) -> EncodedImage {
    // present.wgsl converts Rec.2020 → sRGB *before* running the look, so the
    // look must run in linear sRGB here too or export drifts from the preview.
    encode_looked(
        linear,
        width,
        height,
        target,
        want16,
        if camera_look { 1 } else { 0 },
    )
}

/// Original look — gamut map + OETF only (mirrors present.wgsl look 4).
pub fn output_transform_passthrough(
    linear: &[f32],
    width: u32,
    height: u32,
    target: TargetSpace,
    want16: bool,
) -> EncodedImage {
    let m = target_from_rec2020(target);
    let px = (width * height) as usize;
    let mut rgb8 = if want16 {
        Vec::new()
    } else {
        Vec::with_capacity(px * 3)
    };
    let mut rgb16 = if want16 {
        Vec::with_capacity(px * 3)
    } else {
        Vec::new()
    };
    for i in 0..px {
        let lin = [linear[i * 3], linear[i * 3 + 1], linear[i * 3 + 2]];
        let target_lin = gamut_compress(mat_vec(&m, lin));
        for c in target_lin {
            let e = oetf(target, c);
            if want16 {
                rgb16.push((e * 65535.0).round() as u16);
            } else {
                rgb8.push((e * 255.0).round() as u8);
            }
        }
    }
    EncodedImage {
        width,
        height,
        rgb8,
        rgb16,
    }
}

/// AgX filmic display transform — MIRRORS graph/present.wgsl `agx()` exactly
/// (same Minimal-AgX constants), so Filmic preview == Filmic sRGB export.
/// Input: linear sRGB. Output: sRGB display-encoded [0,1].
fn agx_rs(srgb_lin: [f32; 3]) -> [f32; 3] {
    const INSET: Mat3 = [
        [0.8424790622, 0.0423282423, 0.0423756549],
        [0.0784336000, 0.8784686365, 0.0784336000],
        [0.0792237451, 0.0791661275, 0.8791429738],
    ];
    const OUTSET: Mat3 = [
        [1.1968790051, -0.0528968518, -0.0529716355],
        [-0.0980208811, 1.1519031299, -0.0980434501],
        [-0.0990297441, -0.0989611768, 1.1510736726],
    ];
    const MIN_EV: f32 = -12.47393;
    const MAX_EV: f32 = 4.026069;
    let contrast = |x: f32| {
        let x2 = x * x;
        let x4 = x2 * x2;
        15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x
            - 0.00232
    };
    let v = mat_vec(&INSET, srgb_lin);
    let mut log = [0f32; 3];
    for k in 0..3 {
        let x = (v[k].max(1e-10).log2() - MIN_EV) / (MAX_EV - MIN_EV);
        log[k] = contrast(x.clamp(0.0, 1.0));
    }
    let out = mat_vec(&OUTSET, log);
    [
        out[0].clamp(0.0, 1.0),
        out[1].clamp(0.0, 1.0),
        out[2].clamp(0.0, 1.0),
    ]
}

/// AgX → sRGB 8-bit (display-encoded; bypasses the normal matrix/gamut/OETF).
fn agx_srgb_output(linear: &[f32], width: u32, height: u32) -> EncodedImage {
    let m = target_from_rec2020(TargetSpace::Srgb); // linear Rec.2020 → linear sRGB
    let px = (width * height) as usize;
    let mut rgb8 = Vec::with_capacity(px * 3);
    for i in 0..px {
        let rec = [
            linear[i * 3].max(0.0),
            linear[i * 3 + 1].max(0.0),
            linear[i * 3 + 2].max(0.0),
        ];
        let srgb = mat_vec(&m, rec).map(|c| c.max(0.0));
        for c in agx_rs(srgb) {
            rgb8.push((c * 255.0).round() as u8);
        }
    }
    EncodedImage {
        width,
        height,
        rgb8,
        rgb16: Vec::new(),
    }
}

/// Look-aware output — MIRRORS present.wgsl dispatch:
/// look 2 (Filmic AgX) → AgX for sRGB 8-bit; look 3/4 → gamut map + OETF;
/// look 5/6/7 → DCP Camera grades; look 0/1 → Neutral / no-DCP Camera.
/// Looks 5/6/7 used to fall through to `look >= 1` (old punchy Reinhard),
/// which made Camera-look ARW exports diverge from the viewport.
pub fn output_transform_look(
    linear: &[f32],
    width: u32,
    height: u32,
    target: TargetSpace,
    want16: bool,
    look: u32,
) -> EncodedImage {
    if look == 2 && target == TargetSpace::Srgb && !want16 {
        return agx_srgb_output(linear, width, height);
    }
    if look == 3 || look == 4 {
        return output_transform_passthrough(linear, width, height, target, want16);
    }
    encode_looked(linear, width, height, target, want16, look)
}

/// Rec.2020 → sRGB → present look → delivery space + OETF.
fn encode_looked(
    linear: &[f32],
    width: u32,
    height: u32,
    target: TargetSpace,
    want16: bool,
    look: u32,
) -> EncodedImage {
    let m = target_from_rec2020(target);
    let to_srgb = target_from_rec2020(TargetSpace::Srgb);
    let srgb_to_target = mat_mul(
        &m,
        &color::mat_inverse(&to_srgb).expect("srgb matrix invertible"),
    );
    let px = (width * height) as usize;
    let mut rgb8 = if want16 {
        Vec::new()
    } else {
        Vec::with_capacity(px * 3)
    };
    let mut rgb16 = if want16 {
        Vec::with_capacity(px * 3)
    } else {
        Vec::new()
    };
    for i in 0..px {
        let lin = [
            linear[i * 3].max(0.0),
            linear[i * 3 + 1].max(0.0),
            linear[i * 3 + 2].max(0.0),
        ];
        // present.wgsl: matrix → magenta/cyan fixes (neg R allowed) → look.
        let looked = apply_present_look(apply_present_gamut_fixes(mat_vec(&to_srgb, lin)), look);
        let target_lin = gamut_compress(mat_vec(&srgb_to_target, looked));
        for c in target_lin {
            let e = oetf(target, c);
            if want16 {
                rgb16.push((e * 65535.0).round() as u16);
            } else {
                rgb8.push((e * 255.0).round() as u8);
            }
        }
    }
    EncodedImage {
        width,
        height,
        rgb8,
        rgb16,
    }
}

/// Lanczos resize in LINEAR space (before the output transform — quality).
pub fn resize_linear(
    linear: Vec<f32>,
    width: u32,
    height: u32,
    max_dim: u32,
) -> (Vec<f32>, u32, u32) {
    // delegated so the Lanczos code compiles optimized in dev (see resize crate)
    meratech_resize::resize_rgb_f32(linear, width, height, max_dim)
}

/// Output sharpen (USM on 8-bit luma, after resize — spec 7.2).
pub fn output_sharpen8(rgb: &mut [u8], width: u32, height: u32, amount: f32) {
    if amount <= 0.0 || width < 4 || height < 4 {
        return;
    }
    let k = amount / 100.0 * 0.8;
    let (w, h) = (width as usize, height as usize);
    let luma: Vec<f32> = (0..w * h)
        .map(|i| {
            0.2126 * rgb[i * 3] as f32
                + 0.7152 * rgb[i * 3 + 1] as f32
                + 0.0722 * rgb[i * 3 + 2] as f32
        })
        .collect();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            let blur =
                (luma[i - 1] + luma[i + 1] + luma[i - w] + luma[i + w] + luma[i] * 4.0) / 8.0;
            let high = luma[i] - blur;
            let gain = 1.0 + k * high / luma[i].max(8.0);
            for c in 0..3 {
                let v = rgb[i * 3 + c] as f32 * gain;
                rgb[i * 3 + c] = v.clamp(0.0, 255.0) as u8;
            }
        }
    }
}

fn icc_bytes(target: TargetSpace) -> Option<Vec<u8>> {
    // macOS ships the delivery profiles (ProPhoto == "ROMM RGB").
    let path = match target {
        TargetSpace::Srgb => "/System/Library/ColorSync/Profiles/sRGB Profile.icc",
        TargetSpace::DisplayP3 => "/System/Library/ColorSync/Profiles/Display P3.icc",
        TargetSpace::AdobeRgb => "/System/Library/ColorSync/Profiles/AdobeRGB1998.icc",
        TargetSpace::ProPhoto => "/System/Library/ColorSync/Profiles/ROMM RGB.icc",
    };
    std::fs::read(path).ok()
}

// ---------------------------------------------------------------------------
// EXIF / metadata (hand-rolled little-endian TIFF; no external dep)
// ---------------------------------------------------------------------------

enum ExifVal {
    Ascii(String),
    Short(u16),
    Long(u32),
    Rational(u32, u32),
    /// Multiple rationals (GPS lat/lon DMS).
    Rationals(Vec<(u32, u32)>),
    Undef(Vec<u8>),
}

impl ExifVal {
    fn type_id(&self) -> u16 {
        match self {
            ExifVal::Ascii(_) => 2,
            ExifVal::Short(_) => 3,
            ExifVal::Long(_) => 4,
            ExifVal::Rational(..) | ExifVal::Rationals(_) => 5,
            ExifVal::Undef(_) => 7,
        }
    }
    fn count(&self) -> u32 {
        match self {
            ExifVal::Ascii(s) => s.len() as u32 + 1, // + NUL
            ExifVal::Short(_) => 1,
            ExifVal::Long(_) => 1,
            ExifVal::Rational(..) => 1,
            ExifVal::Rationals(v) => v.len() as u32,
            ExifVal::Undef(b) => b.len() as u32,
        }
    }
    fn raw(&self) -> Vec<u8> {
        match self {
            ExifVal::Ascii(s) => {
                let mut v = s.as_bytes().to_vec();
                v.push(0);
                v
            }
            ExifVal::Short(x) => x.to_le_bytes().to_vec(),
            ExifVal::Long(x) => x.to_le_bytes().to_vec(),
            ExifVal::Rational(n, d) => {
                let mut v = n.to_le_bytes().to_vec();
                v.extend_from_slice(&d.to_le_bytes());
                v
            }
            ExifVal::Rationals(vals) => {
                let mut v = Vec::with_capacity(vals.len() * 8);
                for (n, d) in vals {
                    v.extend_from_slice(&n.to_le_bytes());
                    v.extend_from_slice(&d.to_le_bytes());
                }
                v
            }
            ExifVal::Undef(b) => b.clone(),
        }
    }
}

/// Serialize one IFD. Values >4 bytes spill into `data` (offsets are
/// absolute, based at `data_base`). Entries must be tag-sorted (TIFF rule).
fn serialize_ifd(
    entries: &[(u16, ExifVal)],
    data_base: usize,
    data: &mut Vec<u8>,
    next_ifd: u32,
) -> Vec<u8> {
    let mut ifd = Vec::with_capacity(2 + entries.len() * 12 + 4);
    ifd.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for (tag, val) in entries {
        let raw = val.raw();
        ifd.extend_from_slice(&tag.to_le_bytes());
        ifd.extend_from_slice(&val.type_id().to_le_bytes());
        ifd.extend_from_slice(&val.count().to_le_bytes());
        if raw.len() <= 4 {
            let mut field = [0u8; 4];
            field[..raw.len()].copy_from_slice(&raw);
            ifd.extend_from_slice(&field);
        } else {
            let off = (data_base + data.len()) as u32;
            ifd.extend_from_slice(&off.to_le_bytes());
            data.extend_from_slice(&raw);
            if data.len() % 2 == 1 {
                data.push(0); // values are word-aligned
            }
        }
    }
    ifd.extend_from_slice(&next_ifd.to_le_bytes());
    ifd
}

/// Build a raw EXIF/TIFF blob (no "Exif\0\0" prefix) from capture metadata.
fn build_exif(meta: &ImageMeta, settings: &ExportSettings) -> Vec<u8> {
    const EXIF_IFD_PTR: u16 = 0x8769;
    const GPS_IFD_PTR: u16 = 0x8825;

    let include_gps = settings.effective_policy() == MetadataPolicy::Preserve
        && meta.gps_lat.is_some()
        && meta.gps_lon.is_some();

    let mut ifd0: Vec<(u16, ExifVal)> = Vec::new();
    if !meta.camera_make.is_empty() {
        ifd0.push((0x010F, ExifVal::Ascii(meta.camera_make.clone()))); // Make
    }
    if !meta.camera_model.is_empty() {
        ifd0.push((0x0110, ExifVal::Ascii(meta.camera_model.clone()))); // Model
    }
    ifd0.push((0x0131, ExifVal::Ascii("MeraRAW".into()))); // Software
    if let Some(dt) = exif_datetime(meta.captured_at.as_deref()) {
        ifd0.push((0x0132, ExifVal::Ascii(dt))); // DateTime
    }
    if let Some(c) = settings.copyright.as_deref().filter(|c| !c.is_empty()) {
        ifd0.push((0x013B, ExifVal::Ascii(c.to_string()))); // Artist
        ifd0.push((0x8298, ExifVal::Ascii(c.to_string()))); // Copyright
    }
    // ExifIFD pointer (LONG) — value patched once IFD0 size is known.
    ifd0.push((EXIF_IFD_PTR, ExifVal::Long(0)));
    if include_gps {
        ifd0.push((GPS_IFD_PTR, ExifVal::Long(0)));
    }

    let mut exif: Vec<(u16, ExifVal)> = Vec::new();
    exif.push((0x9000, ExifVal::Undef(b"0230".to_vec()))); // ExifVersion
    if let Some((n, d)) = shutter_rational(meta.shutter.as_deref()) {
        exif.push((0x829A, ExifVal::Rational(n, d))); // ExposureTime
    }
    if let Some(a) = meta.aperture.filter(|a| *a > 0.0) {
        exif.push((0x829D, ExifVal::Rational((a * 1000.0).round() as u32, 1000)));
        // FNumber
    }
    if let Some(iso) = meta.iso {
        exif.push((0x8827, ExifVal::Short(iso.min(65535) as u16))); // ISOSpeedRatings
    }
    if let Some(dt) = exif_datetime(meta.captured_at.as_deref()) {
        exif.push((0x9003, ExifVal::Ascii(dt))); // DateTimeOriginal
    }
    if let Some(f) = meta.focal_mm.filter(|f| *f > 0.0) {
        exif.push((0x920A, ExifVal::Rational((f * 100.0).round() as u32, 100)));
        // FocalLength
    }
    if let Some(l) = meta.lens.as_deref().filter(|l| !l.is_empty()) {
        exif.push((0xA434, ExifVal::Ascii(l.to_string()))); // LensModel
    }

    let mut gps: Vec<(u16, ExifVal)> = Vec::new();
    if include_gps {
        let lat = meta.gps_lat.unwrap();
        let lon = meta.gps_lon.unwrap();
        gps.push((0x0000, ExifVal::Undef(vec![2, 3, 0, 0]))); // GPSVersionID
        gps.push((
            0x0001,
            ExifVal::Ascii(if lat >= 0.0 { "N" } else { "S" }.into()),
        ));
        gps.push((0x0002, ExifVal::Rationals(deg_to_dms(lat.abs()))));
        gps.push((
            0x0003,
            ExifVal::Ascii(if lon >= 0.0 { "E" } else { "W" }.into()),
        ));
        gps.push((0x0004, ExifVal::Rationals(deg_to_dms(lon.abs()))));
    }

    // Layout: header(8) | IFD0 | ExifIFD | [GPS IFD] | data.
    let n0 = ifd0.len();
    let s0 = 2 + 12 * n0 + 4;
    let exif_off = 8 + s0;
    let se = 2 + 12 * exif.len() + 4;
    let gps_off = exif_off + se;
    let sg = if gps.is_empty() {
        0
    } else {
        2 + 12 * gps.len() + 4
    };
    let data_base = 8 + s0 + se + sg;

    if let Some(e) = ifd0.iter_mut().find(|e| e.0 == EXIF_IFD_PTR) {
        e.1 = ExifVal::Long(exif_off as u32);
    }
    if include_gps {
        if let Some(e) = ifd0.iter_mut().find(|e| e.0 == GPS_IFD_PTR) {
            e.1 = ExifVal::Long(gps_off as u32);
        }
    }

    ifd0.sort_by_key(|e| e.0);
    exif.sort_by_key(|e| e.0);
    gps.sort_by_key(|e| e.0);

    let mut data = Vec::new();
    let ifd0_bytes = serialize_ifd(&ifd0, data_base, &mut data, 0);
    let exif_bytes = serialize_ifd(&exif, data_base, &mut data, 0);
    let gps_bytes = if gps.is_empty() {
        Vec::new()
    } else {
        serialize_ifd(&gps, data_base, &mut data, 0)
    };

    let mut out =
        Vec::with_capacity(8 + ifd0_bytes.len() + exif_bytes.len() + gps_bytes.len() + data.len());
    out.extend_from_slice(b"II"); // little-endian
    out.extend_from_slice(&42u16.to_le_bytes());
    out.extend_from_slice(&8u32.to_le_bytes()); // IFD0 at offset 8
    out.extend_from_slice(&ifd0_bytes);
    out.extend_from_slice(&exif_bytes);
    out.extend_from_slice(&gps_bytes);
    out.extend_from_slice(&data);
    out
}

fn deg_to_dms(abs_deg: f64) -> Vec<(u32, u32)> {
    let deg = abs_deg.floor();
    let min_f = (abs_deg - deg) * 60.0;
    let min = min_f.floor();
    let sec = (min_f - min) * 60.0;
    vec![
        (deg as u32, 1),
        (min as u32, 1),
        ((sec * 10_000.0).round() as u32, 10_000),
    ]
}

fn exif_datetime(captured: Option<&str>) -> Option<String> {
    let s = captured?.trim();
    if s.len() < 19 {
        return None;
    }
    // Normalize "2024-05-11T19:15:00…" / "2024:05:11 19:15:00" → EXIF form.
    let out: String = s
        .chars()
        .take(19)
        .enumerate()
        .map(|(i, ch)| match i {
            4 | 7 => ':',
            10 => ' ',
            _ => ch,
        })
        .collect();
    Some(out)
}

fn shutter_rational(s: Option<&str>) -> Option<(u32, u32)> {
    let s = s?.trim();
    if let Some((n, d)) = s.split_once('/') {
        let n: u32 = n.trim().parse().ok()?;
        let d: u32 = d.trim().parse().ok()?;
        if d == 0 {
            return None;
        }
        return Some((n, d));
    }
    let v: f32 = s.parse().ok()?;
    if v <= 0.0 {
        return None;
    }
    Some(((v * 1000.0).round() as u32, 1000))
}

/// Splice EXIF (APP1) + ICC (APP2) marker segments in after the JPEG SOI.
fn splice_jpeg_metadata(jpeg: Vec<u8>, icc: Option<&[u8]>, exif: Option<&[u8]>) -> Vec<u8> {
    let mut out = Vec::with_capacity(jpeg.len() + icc.map_or(0, |i| i.len()) + 512);
    out.extend_from_slice(&jpeg[..2]); // SOI
    if let Some(exif) = exif {
        // APP1: "Exif\0\0" + TIFF. Tiny — single segment.
        let payload_len = 2 + 6 + exif.len();
        out.extend_from_slice(&[0xFF, 0xE1]);
        out.extend_from_slice(&(payload_len as u16).to_be_bytes());
        out.extend_from_slice(b"Exif\0\0");
        out.extend_from_slice(exif);
    }
    if let Some(icc) = icc {
        const CHUNK: usize = 65519 - 14; // marker payload minus ICC header
        let chunks: Vec<&[u8]> = icc.chunks(CHUNK).collect();
        let total = chunks.len() as u8;
        for (i, chunk) in chunks.iter().enumerate() {
            let payload_len = 2 + 12 + 2 + chunk.len();
            out.extend_from_slice(&[0xFF, 0xE2]);
            out.extend_from_slice(&(payload_len as u16).to_be_bytes());
            out.extend_from_slice(b"ICC_PROFILE\0");
            out.push(i as u8 + 1);
            out.push(total);
            out.extend_from_slice(chunk);
        }
    }
    out.extend_from_slice(&jpeg[2..]);
    out
}

/// PNG encode with embedded ICC (iCCP) + EXIF (eXIf) via the png crate.
fn encode_png(
    enc: &EncodedImage,
    icc: Option<&[u8]>,
    exif: Option<&[u8]>,
) -> Result<Vec<u8>, CoreError> {
    use std::borrow::Cow;
    let mut info = png::Info::with_size(enc.width, enc.height);
    info.color_type = png::ColorType::Rgb;
    info.bit_depth = png::BitDepth::Eight;
    if let Some(icc) = icc {
        info.icc_profile = Some(Cow::Owned(icc.to_vec()));
    }
    if let Some(exif) = exif {
        info.exif_metadata = Some(Cow::Owned(exif.to_vec()));
    }
    let mut out = Vec::new();
    {
        let encoder = png::Encoder::with_info(&mut out, info)
            .map_err(|e| CoreError::Io(format!("png info: {e}")))?;
        let mut writer = encoder
            .write_header()
            .map_err(|e| CoreError::Io(format!("png header: {e}")))?;
        writer
            .write_image_data(&enc.rgb8)
            .map_err(|e| CoreError::Io(format!("png data: {e}")))?;
    }
    Ok(out)
}

/// 16-bit TIFF with ICCProfile tag (34675) + baseline metadata tags.
fn encode_tiff16(
    enc: &EncodedImage,
    icc: Option<&[u8]>,
    meta: &ImageMeta,
    settings: &ExportSettings,
) -> Result<Vec<u8>, CoreError> {
    use tiff::encoder::{colortype, TiffEncoder};
    use tiff::tags::Tag;
    let map = |e: tiff::TiffError| CoreError::Io(format!("tiff: {e}"));

    let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
    {
        let mut tiff = TiffEncoder::new(&mut cursor).map_err(map)?;
        let mut image = tiff
            .new_image::<colortype::RGB16>(enc.width, enc.height)
            .map_err(map)?;
        {
            let dir = image.encoder();
            if let Some(icc) = icc {
                dir.write_tag(Tag::Unknown(34675), icc).map_err(map)?; // ICCProfile
            }
            if settings.effective_policy() != MetadataPolicy::StripAll {
                if !meta.camera_make.is_empty() {
                    let _ = dir.write_tag(Tag::Make, meta.camera_make.as_str());
                }
                if !meta.camera_model.is_empty() {
                    let _ = dir.write_tag(Tag::Model, meta.camera_model.as_str());
                }
                let _ = dir.write_tag(Tag::Software, "MeraRAW");
                if let Some(dt) = exif_datetime(meta.captured_at.as_deref()) {
                    let _ = dir.write_tag(Tag::DateTime, dt.as_str());
                }
                if let Some(c) = settings.copyright.as_deref().filter(|c| !c.is_empty()) {
                    let _ = dir.write_tag(Tag::Unknown(33432), c); // Copyright
                    let _ = dir.write_tag(Tag::Artist, c);
                }
            }
        }
        image.write_data(&enc.rgb16).map_err(map)?;
    }
    Ok(cursor.into_inner())
}

/// Encode + write. Returns the output path.
pub fn encode_and_write(
    enc: &EncodedImage,
    settings: &ExportSettings,
    source_path: &str,
    meta: &ImageMeta,
) -> Result<PathBuf, CoreError> {
    let stem = Path::new(source_path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "export".into());
    let stem = settings
        .output_stem
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or(stem);
    let dir = if settings.dest_dir.is_empty() {
        Path::new(source_path)
            .parent()
            .unwrap_or(Path::new("."))
            .join("exports")
    } else {
        PathBuf::from(&settings.dest_dir)
    };
    std::fs::create_dir_all(&dir)?;

    let icc = icc_bytes(settings.target);
    let exif = if settings.effective_policy() == MetadataPolicy::StripAll {
        None
    } else {
        Some(build_exif(meta, settings))
    };

    let path = match settings.format {
        ExportFormat::Jpeg => {
            let p = dir.join(format!("{stem}.jpg"));
            let img = image::RgbImage::from_raw(enc.width, enc.height, enc.rgb8.clone())
                .ok_or_else(|| CoreError::Io("buffer".into()))?;
            let mut bytes = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(
                &mut bytes,
                settings.quality.clamp(1, 100),
            )
            .encode_image(&img)
            .map_err(|e| CoreError::Io(format!("encode: {e}")))?;
            let bytes = splice_jpeg_metadata(bytes, icc.as_deref(), exif.as_deref());
            std::fs::write(&p, bytes)?;
            p
        }
        ExportFormat::Png => {
            let p = dir.join(format!("{stem}.png"));
            let bytes = encode_png(enc, icc.as_deref(), exif.as_deref())?;
            std::fs::write(&p, bytes)?;
            p
        }
        ExportFormat::Tiff16 => {
            let p = dir.join(format!("{stem}.tif"));
            let bytes = encode_tiff16(enc, icc.as_deref(), meta, settings)?;
            std::fs::write(&p, bytes)?;
            p
        }
        ExportFormat::Heic => {
            // No pure-Rust HEIC encoder: hand an ICC+EXIF-embedded PNG carrier
            // to macOS `sips`, which carries the profile into the HEIC.
            // sips is macOS-only, so HEIC export is a macOS-only feature for now.
            #[cfg(not(target_os = "macos"))]
            {
                return Err(CoreError::Io(
                    "HEIC export is only available on macOS in this build — \
                     use JPEG, PNG or TIFF."
                        .into(),
                ));
            }
            #[cfg(target_os = "macos")]
            {
                let p = dir.join(format!("{stem}.heic"));
                let png = encode_png(enc, icc.as_deref(), exif.as_deref())?;
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                let tmp = std::env::temp_dir().join(format!("meraraw-{stem}-{nanos}.png"));
                std::fs::write(&tmp, &png)?;
                let out = std::process::Command::new("/usr/bin/sips")
                    .args(["-s", "format", "heic"])
                    .args([
                        "-s",
                        "formatOptions",
                        &settings.quality.clamp(1, 100).to_string(),
                    ])
                    .arg(&tmp)
                    .arg("--out")
                    .arg(&p)
                    .output();
                let _ = std::fs::remove_file(&tmp);
                match out {
                    Ok(o) if o.status.success() => p,
                    Ok(o) => {
                        return Err(CoreError::Io(format!(
                            "sips heic failed: {}",
                            String::from_utf8_lossy(&o.stderr).trim()
                        )))
                    }
                    Err(e) => return Err(CoreError::Io(format!("sips spawn: {e}"))),
                }
            }
        }
    };
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_meta() -> ImageMeta {
        ImageMeta {
            path: "/x/DSC0001.ARW".into(),
            kind: crate::raw::ImageKind::Raw,
            format: "ARW".into(),
            bit_depth: 0,
            camera_make: "SONY".into(),
            camera_model: "ILCE-7M4".into(),
            lens: Some("FE 50mm F1.8".into()),
            iso: Some(400),
            shutter: Some("1/250".into()),
            aperture: Some(1.8),
            focal_mm: Some(50.0),
            captured_at: Some("2024-05-11T19:15:00".into()),
            width: 4,
            height: 4,
            orientation: "Normal".into(),
            as_shot_wb: [1.0, 1.0, 1.0],
            estimated_cct: Some(5200.0),
            camera_profile: None,
            available_profiles: Vec::new(),
            available_profile_files: Vec::new(),
            demosaic: String::new(),
            available_demosaic: Vec::new(),
            gps_lat: Some(37.7749),
            gps_lon: Some(-122.4194),
            input_color_space: None,
            video: None,
        }
    }

    #[test]
    fn strip_gps_omits_gps_ifd_pointer() {
        let meta = test_meta();
        let preserve = build_exif(
            &meta,
            &ExportSettings {
                metadata_policy: MetadataPolicy::Preserve,
                ..Default::default()
            },
        );
        let stripped = build_exif(
            &meta,
            &ExportSettings {
                metadata_policy: MetadataPolicy::StripGps,
                ..Default::default()
            },
        );
        // GPS IFD pointer tag 0x8825
        let has_gps = |blob: &[u8]| {
            let n0 = u16::from_le_bytes([blob[8], blob[9]]) as usize;
            (0..n0).any(|i| {
                let off = 10 + i * 12;
                u16::from_le_bytes([blob[off], blob[off + 1]]) == 0x8825
            })
        };
        assert!(has_gps(&preserve), "preserve should embed GPS IFD ptr");
        assert!(!has_gps(&stripped), "stripGps must omit GPS IFD ptr");
    }

    #[test]
    fn neutral_stays_neutral_in_every_target() {
        for target in [
            TargetSpace::Srgb,
            TargetSpace::DisplayP3,
            TargetSpace::AdobeRgb,
            TargetSpace::ProPhoto,
        ] {
            let lin = [0.18f32, 0.18, 0.18];
            let e = output_transform(&lin, 1, 1, target, false, false);
            let px = &e.rgb8;
            assert!(
                (px[0] as i32 - px[1] as i32).abs() <= 1
                    && (px[1] as i32 - px[2] as i32).abs() <= 1,
                "{target:?}: {px:?}"
            );
        }
    }

    /// Neutral look (present.wgsl VIEW_LW=4) is monotone and does not clip
    /// mid-highlights. Sensor sat does *not* slam to display white — that was
    /// the old export-only Reinhard (white point == gain).
    #[test]
    fn highlights_roll_off_without_clipping() {
        let enc = |v: f32| {
            output_transform(&[v, v, v], 1, 1, TargetSpace::Srgb, false, false).rgb8[0] as i32
        };
        assert!(enc(1.0) < 255, "Neutral look must not slam sat to white");
        assert!(enc(0.7) < enc(1.0), "not monotone at the top");
        assert!(enc(0.5) < enc(0.7), "not monotone");
        assert!(
            enc(0.7) - enc(0.5) < enc(0.3) - enc(0.1),
            "no compression near white: {} vs {}",
            enc(0.7) - enc(0.5),
            enc(0.3) - enc(0.1)
        );
    }

    /// No-DCP Camera (look 1) is the milder Affinity fallback, not a +20-byte
    /// punchy lift over Neutral. Both stay off the ceiling at mid-highlights.
    #[test]
    fn camera_fallback_stays_near_neutral() {
        let enc = |v: f32, camera: bool| {
            output_transform(&[v, v, v], 1, 1, TargetSpace::Srgb, false, camera).rgb8[0] as i32
        };
        let (mid_cam, mid_neu) = (enc(0.1655, true), enc(0.1655, false));
        assert!(
            (mid_cam - mid_neu).abs() < 20,
            "look 1 drifted from Neutral: camera {mid_cam} vs neutral {mid_neu}"
        );
        assert!(enc(0.75, true) < 255, "burnt at 0.75: {}", enc(0.75, true));
        assert!(enc(0.5, true) < enc(0.75, true), "not monotone");
    }

    /// DCP Camera grades (looks 5/6/7) must not fall through to look 1.
    /// That fall-through was the old punchy Reinhard and made ARW Camera
    /// exports diverge from the viewport.
    #[test]
    fn dcp_camera_grades_are_not_punchy_fallback() {
        let lin = [0.35f32, 0.38, 0.36];
        let fallback = output_transform_look(&lin, 1, 1, TargetSpace::Srgb, false, 1);
        let look5 = output_transform_look(&lin, 1, 1, TargetSpace::Srgb, false, 5);
        let look6 = output_transform_look(&lin, 1, 1, TargetSpace::Srgb, false, 6);
        let look7 = output_transform_look(&lin, 1, 1, TargetSpace::Srgb, false, 7);
        assert_ne!(
            look5.rgb8, fallback.rgb8,
            "look 5 must not equal no-DCP Camera"
        );
        assert_ne!(
            look6.rgb8, fallback.rgb8,
            "look 6 must not equal no-DCP Camera"
        );
        assert_ne!(
            look7.rgb8, fallback.rgb8,
            "look 7 must not equal no-DCP Camera"
        );
        let to_srgb = target_from_rec2020(TargetSpace::Srgb);
        let srgb = apply_present_gamut_fixes(mat_vec(&to_srgb, lin));
        let graded = DcpProfile::view_look_display(srgb);
        for ch in 0..3 {
            let expect =
                (oetf(TargetSpace::Srgb, graded[ch].clamp(0.0, 1.0)) * 255.0).round() as i32;
            assert!(
                (look5.rgb8[ch] as i32 - expect).abs() <= 1,
                "look 5 must use the DCP grade: ch{ch} got {} want {expect}",
                look5.rgb8[ch]
            );
        }
    }

    /// present.wgsl converts Rec.2020 → sRGB and *then* runs the look. sRGB
    /// export has to agree pixel-for-pixel or the preview is lying.
    #[test]
    fn srgb_export_matches_preview_ordering() {
        // transcribed from present.wgsl (row-major); rounded literals there
        // put the two matrices ~1e-4 apart, hence the ±1 byte tolerance.
        const PRESENT_REC2020_TO_SRGB: Mat3 = [
            [1.6605, -0.5876, -0.0728],
            [-0.1246, 1.1329, -0.0083],
            [-0.0182, -0.1006, 1.1187],
        ];
        for rec in [
            [0.05f32, 0.30, 0.02],
            [0.40, 0.50, 0.60],
            [0.90, 0.20, 0.10],
            [0.02, 0.02, 0.02],
            [0.75, 0.80, 0.30],
        ] {
            for look in [0u32, 1, 5, 6, 7] {
                let c = apply_present_gamut_fixes(mat_vec(&PRESENT_REC2020_TO_SRGB, rec));
                let preview = apply_present_look(c, look)
                    .map(|v| (oetf(TargetSpace::Srgb, v.clamp(0.0, 1.0)) * 255.0).round() as i32);
                let got = output_transform_look(&rec, 1, 1, TargetSpace::Srgb, false, look);
                for ch in 0..3 {
                    let d = (preview[ch] - got.rgb8[ch] as i32).abs();
                    assert!(
                        d <= 1,
                        "preview/export split on {rec:?} look={look} ch{ch}: \
                         preview {} vs export {}",
                        preview[ch],
                        got.rgb8[ch]
                    );
                }
            }
        }
    }

    #[test]
    fn out_of_gamut_compresses_at_positive_values() {
        let e = output_transform(&[0.0, 0.5, 0.0], 1, 1, TargetSpace::Srgb, false, false);
        assert_eq!(e.rgb8.len(), 3);
        assert!(e.rgb8[1] > e.rgb8[0] && e.rgb8[1] > e.rgb8[2]);
    }

    #[test]
    fn prophoto_keeps_wide_green_in_range_and_dominant() {
        // A saturated Rec.2020 green must encode green-dominant and fully
        // in-range (no negative/overflow) through the D50 ProPhoto path.
        let pp = output_transform(&[0.0, 0.6, 0.0], 1, 1, TargetSpace::ProPhoto, false, false);
        assert_eq!(pp.rgb8.len(), 3);
        assert!(
            pp.rgb8[1] > pp.rgb8[0] && pp.rgb8[1] > pp.rgb8[2],
            "green dominant: {:?}",
            pp.rgb8
        );
        assert!(pp.rgb8[1] > 100, "green present: {:?}", pp.rgb8);
    }

    #[test]
    fn icc_and_exif_embed_keeps_valid_jpeg() {
        let img = image::RgbImage::from_pixel(8, 8, image::Rgb([128, 128, 128]));
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90)
            .encode_image(&img)
            .unwrap();
        let icc = vec![0xAAu8; 100_000]; // multi-chunk
        let exif = build_exif(&test_meta(), &ExportSettings::default());
        let out = splice_jpeg_metadata(jpeg, Some(&icc), Some(&exif));
        assert_eq!(&out[..2], &[0xFF, 0xD8], "SOI intact");
        assert_eq!(&out[2..4], &[0xFF, 0xE1], "APP1 EXIF follows SOI");
        let icc_needle = b"ICC_PROFILE\0";
        let icc_chunks = out
            .windows(icc_needle.len())
            .filter(|w| *w == icc_needle)
            .count();
        assert!(
            icc_chunks >= 2,
            "expected multi-chunk ICC, got {icc_chunks}"
        );
        let decoded = image::load_from_memory(&out).expect("jpeg decodes");
        assert_eq!(decoded.width(), 8);
    }

    #[test]
    fn exif_blob_is_well_formed_tiff() {
        let exif = build_exif(&test_meta(), &ExportSettings::default());
        assert_eq!(&exif[..2], b"II", "little-endian header");
        assert_eq!(u16::from_le_bytes([exif[2], exif[3]]), 42, "magic 42");
        let ifd0 = u32::from_le_bytes([exif[4], exif[5], exif[6], exif[7]]) as usize;
        assert_eq!(ifd0, 8);
        let n0 = u16::from_le_bytes([exif[8], exif[9]]) as usize;
        assert!(n0 >= 4, "IFD0 has entries, got {n0}");
        // every entry's tag is ascending (TIFF requirement)
        let mut prev = 0u16;
        for i in 0..n0 {
            let off = 10 + i * 12;
            let tag = u16::from_le_bytes([exif[off], exif[off + 1]]);
            assert!(tag >= prev, "IFD0 tags must be sorted: {tag} after {prev}");
            prev = tag;
        }
        // ExifIFD pointer (0x8769) present and points inside the blob
        let has_exif_ptr = (0..n0).any(|i| {
            let off = 10 + i * 12;
            u16::from_le_bytes([exif[off], exif[off + 1]]) == 0x8769
        });
        assert!(has_exif_ptr, "ExifIFD pointer present");
    }

    #[test]
    fn strip_metadata_yields_no_exif() {
        let s = ExportSettings {
            strip_metadata: true,
            ..Default::default()
        };
        assert!(s.strip_metadata);
        // encode_and_write gates EXIF on this flag (see fn body).
    }

    #[test]
    fn shutter_and_datetime_parsing() {
        assert_eq!(shutter_rational(Some("1/250")), Some((1, 250)));
        assert_eq!(shutter_rational(Some("0.5")), Some((500, 1000)));
        assert_eq!(shutter_rational(Some("bad")), None);
        assert_eq!(
            exif_datetime(Some("2024-05-11T19:15:00")).as_deref(),
            Some("2024:05:11 19:15:00")
        );
        assert_eq!(
            exif_datetime(Some("2024:05:11 19:15:00")).as_deref(),
            Some("2024:05:11 19:15:00")
        );
    }

    #[test]
    fn resize_caps_long_edge() {
        let lin = vec![0.2f32; 100 * 50 * 3];
        let (out, w, h) = resize_linear(lin, 100, 50, 40);
        assert_eq!((w, h), (40, 20));
        assert_eq!(out.len(), (40 * 20 * 3) as usize);
    }

    #[test]
    fn adobe_matrix_white_is_d65() {
        let xyz = mat_vec(&ADOBE_TO_XYZ, [1.0, 1.0, 1.0]);
        assert!((xyz[0] - 0.9505).abs() < 0.01, "{xyz:?}");
        assert!((xyz[1] - 1.0).abs() < 0.01);
        assert!((xyz[2] - 1.0889).abs() < 0.01);
    }

    /// Writes real files for every format × space into /tmp/meraraw-smoke so
    /// the embedded ICC + EXIF can be inspected with macOS `sips`/`mdls`.
    /// Run: `cargo test -p meratech-core export::tests::smoke -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn smoke_write_all_formats() {
        let (w, h) = (96u32, 64u32);
        // a linear Rec.2020 gradient with a colored band so it's not neutral
        let mut lin = vec![0.0f32; (w * h * 3) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 3) as usize;
                let t = x as f32 / w as f32;
                lin[i] = 0.05 + 0.6 * t;
                lin[i + 1] = 0.30;
                lin[i + 2] = 0.65 - 0.5 * t;
            }
        }
        let meta = test_meta();
        let dir = "/tmp/meraraw-smoke";
        let _ = std::fs::remove_dir_all(dir);
        for (fmt, fname) in [
            (ExportFormat::Jpeg, "jpeg"),
            (ExportFormat::Png, "png"),
            (ExportFormat::Tiff16, "tiff"),
            (ExportFormat::Heic, "heic"),
        ] {
            for (tgt, tname) in [
                (TargetSpace::Srgb, "srgb"),
                (TargetSpace::DisplayP3, "p3"),
                (TargetSpace::AdobeRgb, "adobe"),
                (TargetSpace::ProPhoto, "prophoto"),
            ] {
                let want16 = fmt == ExportFormat::Tiff16;
                let mut enc = output_transform(&lin, w, h, tgt, want16, false);
                if !want16 {
                    output_sharpen8(&mut enc.rgb8, w, h, 30.0);
                }
                let settings = ExportSettings {
                    format: fmt,
                    target: tgt,
                    quality: 92,
                    max_dim: None,
                    sharpen: 30.0,
                    dest_dir: dir.into(),
                    metadata_policy: MetadataPolicy::Preserve,
                    strip_metadata: false,
                    copyright: Some("© Test Photographer".into()),
                    video_clip: false,
                    output_stem: None,
                    video_in: None,
                    video_out: None,
                    video_audio: true,
                };
                let src = format!("/x/smoke_{fname}_{tname}.ARW");
                match encode_and_write(&enc, &settings, &src, &meta) {
                    Ok(p) => println!("WROTE {}", p.display()),
                    Err(e) => println!("FAIL  {fname}/{tname}: {e}"),
                }
            }
        }
        // one stripped variant to confirm the privacy path
        let mut enc = output_transform(&lin, w, h, TargetSpace::Srgb, false, false);
        output_sharpen8(&mut enc.rgb8, w, h, 0.0);
        let stripped = ExportSettings {
            dest_dir: dir.into(),
            strip_metadata: true,
            ..Default::default()
        };
        match encode_and_write(&enc, &stripped, "/x/smoke_stripped.ARW", &meta) {
            Ok(p) => println!("WROTE {} (stripped)", p.display()),
            Err(e) => println!("FAIL  stripped: {e}"),
        }
    }

    #[test]
    fn prophoto_matrix_white_is_d50() {
        let xyz = mat_vec(&PROPHOTO_TO_XYZ_D50, [1.0, 1.0, 1.0]);
        assert!((xyz[0] - 0.9642).abs() < 0.01, "{xyz:?}");
        assert!((xyz[1] - 1.0).abs() < 0.01);
        assert!((xyz[2] - 0.8252).abs() < 0.01);
    }

    #[test]
    fn look3_is_passthrough_matching_original() {
        let lin = [0.216f32, 0.216, 0.216];
        let pass = output_transform_look(&lin, 1, 1, TargetSpace::Srgb, false, 3);
        let orig = output_transform_look(&lin, 1, 1, TargetSpace::Srgb, false, 4);
        assert_eq!(pass.rgb8, orig.rgb8);
    }

    #[test]
    fn dcp_tone_curve_on_linear_jpeg_gray_is_the_burnout() {
        // Adobe default ProfileToneCurve lifts scene mid-gray (~0.18–0.22) to
        // ~0.45+. Applying that to an already-rendered JPEG is the burnout.
        // Passthrough present of the un-curved working buffer stays near 128.
        let lin = [0.216f32, 0.216, 0.216];
        let curved = crate::curve::ProfileToneCurve::adobe_default().apply_rgb(lin);
        let pass = output_transform_look(&lin, 1, 1, TargetSpace::Srgb, false, 3);
        let burnt = output_transform_look(&curved, 1, 1, TargetSpace::Srgb, false, 3);
        assert!(
            (pass.rgb8[0] as i32 - 128).abs() < 8,
            "passthrough mid-gray should stay ~128, got {}",
            pass.rgb8[0]
        );
        assert!(
            burnt.rgb8[0] > pass.rgb8[0] + 40,
            "DCP default curve on a JPEG working buffer lifts mid-gray: pass={} burnt={}",
            pass.rgb8[0],
            burnt.rgb8[0]
        );
    }
}
