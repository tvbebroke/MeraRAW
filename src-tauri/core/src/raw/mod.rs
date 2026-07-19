//! RAW decode. Contract B1: `Decoder` trait isolates the decode dependency
//! (rawler primary — pure Rust, clean macOS build; LibRaw swap stays
//! contained here if coverage demands it).

mod rawler_decoder;
mod standard_decoder;

pub use rawler_decoder::RawlerDecoder;
pub use standard_decoder::StandardDecoder;

/// Pick the right decoder for a path: RAW extensions → rawler, standard image
/// extensions (JPEG/PNG/TIFF/WebP/…) → the rendered-image decoder.
pub fn decoder_for(path: &Path) -> Box<dyn Decoder> {
    let raw = RawlerDecoder::default();
    if raw.probe(path) {
        Box::new(raw)
    } else {
        Box::new(StandardDecoder)
    }
}

use crate::error::CoreError;
use crate::image::RgbF32Buf;
use std::path::Path;

/// Whether the source is sensor data (needs full RAW development) or an
/// already-rendered image (JPEG/PNG/… — display-referred, must skip the
/// camera stages so it isn't double-processed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageKind {
    Raw,
    Rendered,
}

/// Metadata surfaced to the UI + assistant. serde camelCase for the wire.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMeta {
    pub path: String,
    /// Sensor RAW vs already-rendered image (drives pipeline + UI badge).
    pub kind: ImageKind,
    /// Uppercase format tag for the UI, e.g. "ARW", "JPEG", "PNG".
    pub format: String,
    /// Source bit depth per channel (8/14/16/32) — for the UI.
    pub bit_depth: u8,
    pub camera_make: String,
    pub camera_model: String,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    pub shutter: Option<String>,
    pub aperture: Option<f32>,
    pub focal_mm: Option<f32>,
    pub captured_at: Option<String>,
    /// Working-buffer dims AFTER orientation bake.
    pub width: u32,
    pub height: u32,
    pub orientation: String,
    pub as_shot_wb: [f32; 3],
    /// Estimated as-shot correlated color temperature (Kelvin).
    pub estimated_cct: Option<f32>,
    /// Autoloaded DCP profile name, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_profile: Option<String>,
    /// All matched DCP profiles for this camera (for UI picker).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_profiles: Vec<String>,
    /// Parallel to `available_profiles` — DCP filenames for switching.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_profile_files: Vec<String>,
    /// Demosaic algorithm in effect (name, e.g. "rcd"); drives the UI picker.
    #[serde(default)]
    pub demosaic: String,
    /// Demosaic algorithms currently usable (sidecar entries drop out when
    /// their worker binary is missing). Empty for non-RAW sources.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_demosaic: Vec<String>,
}

/// Which demosaic algorithm runs at decode time. `Rawler` = rawler's built-in
/// PPG interpolation; `Bilinear..Ddfapd` are the in-process merawler engine;
/// the `Rt*`/`Dht` variants are the zerawler sidecar engine (external
/// reference binaries). The choice is stored in the doc; changing it
/// re-decodes the RAW (as darktable does).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Demosaic {
    /// rawler's built-in demosaic (the original path).
    Rawler,
    Bilinear,
    Malvar,
    /// darktable's default — the MeraRAW default too.
    #[default]
    Rcd,
    Lmmse,
    Amaze,
    Igv,
    Ddfapd,
    /// RCD via the RawTherapee sidecar (zerawler).
    RtRcd,
    /// LMMSE via the RawTherapee sidecar (zerawler).
    RtLmmse,
    /// AMaZE via the RawTherapee sidecar (zerawler).
    RtAmaze,
    /// DHT via the LibRaw sidecar (zerawler).
    Dht,
}

impl Demosaic {
    pub fn name(self) -> &'static str {
        match self {
            Demosaic::Rawler => "rawler",
            Demosaic::Bilinear => "bilinear",
            Demosaic::Malvar => "malvar",
            Demosaic::Rcd => "rcd",
            Demosaic::Lmmse => "lmmse",
            Demosaic::Amaze => "amaze",
            Demosaic::Igv => "igv",
            Demosaic::Ddfapd => "ddfapd",
            Demosaic::RtRcd => "rt-rcd",
            Demosaic::RtLmmse => "rt-lmmse",
            Demosaic::RtAmaze => "rt-amaze",
            Demosaic::Dht => "dht",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "rawler" => Demosaic::Rawler,
            "bilinear" => Demosaic::Bilinear,
            "malvar" => Demosaic::Malvar,
            "rcd" => Demosaic::Rcd,
            "lmmse" => Demosaic::Lmmse,
            "amaze" => Demosaic::Amaze,
            "igv" => Demosaic::Igv,
            "ddfapd" | "menon" => Demosaic::Ddfapd,
            "rt-rcd" => Demosaic::RtRcd,
            "rt-lmmse" => Demosaic::RtLmmse,
            "rt-amaze" => Demosaic::RtAmaze,
            "dht" => Demosaic::Dht,
            _ => return None,
        })
    }

    /// Parse a name, falling back to the default for `None`/unknown.
    pub fn parse_or_default(s: Option<&str>) -> Self {
        s.and_then(Self::from_name).unwrap_or_default()
    }

    /// All variants, in UI order.
    pub fn all() -> &'static [Demosaic] {
        &[
            Demosaic::Rawler,
            Demosaic::Bilinear,
            Demosaic::Malvar,
            Demosaic::Rcd,
            Demosaic::Lmmse,
            Demosaic::Amaze,
            Demosaic::Igv,
            Demosaic::Ddfapd,
            Demosaic::RtRcd,
            Demosaic::RtLmmse,
            Demosaic::RtAmaze,
            Demosaic::Dht,
        ]
    }

    /// The merawler algorithm, or `None` for non-merawler paths.
    pub fn merawler_algo(self) -> Option<merawler::Algorithm> {
        Some(match self {
            Demosaic::Bilinear => merawler::Algorithm::Bilinear,
            Demosaic::Malvar => merawler::Algorithm::Malvar,
            Demosaic::Rcd => merawler::Algorithm::Rcd,
            Demosaic::Lmmse => merawler::Algorithm::Lmmse,
            Demosaic::Amaze => merawler::Algorithm::Amaze,
            Demosaic::Igv => merawler::Algorithm::Igv,
            Demosaic::Ddfapd => merawler::Algorithm::Ddfapd,
            _ => return None,
        })
    }

    /// The zerawler (sidecar) algorithm, or `None` for in-process paths.
    pub fn zerawler_algo(self) -> Option<zerawler::Algorithm> {
        Some(match self {
            Demosaic::RtRcd => zerawler::Algorithm::Rcd,
            Demosaic::RtLmmse => zerawler::Algorithm::Lmmse,
            Demosaic::RtAmaze => zerawler::Algorithm::Amaze,
            Demosaic::Dht => zerawler::Algorithm::Dht,
            _ => return None,
        })
    }

    /// Names of the algorithms currently usable: in-process ones always,
    /// sidecar ones only when their worker binary resolves. Feeds the UI so
    /// unavailable options can be disabled instead of silently falling back.
    pub fn available() -> Vec<String> {
        let engine = zerawler::Engine::detect();
        Demosaic::all()
            .iter()
            .filter(|d| match d.zerawler_algo() {
                None => true,
                Some(a) => match a.backend() {
                    zerawler::Backend::RawTherapee => engine.rt_cli.is_some(),
                    zerawler::Backend::LibRaw => engine.dcraw_emu.is_some(),
                },
            })
            .map(|d| d.name().to_string())
            .collect()
    }
}

/// Full decode result: working buffer is linear Rec.2020 scene-referred,
/// orientation baked, headroom (>1.0) preserved. Contract A4 input.
pub struct DecodedImage {
    pub working: RgbF32Buf,
    pub meta: ImageMeta,
}

pub trait Decoder: Send + Sync {
    /// Can this decoder handle the file? Cheap check (extension/magic).
    fn probe(&self, path: &Path) -> bool;
    /// Metadata only — fast, no pixel decode.
    fn metadata(&self, path: &Path) -> Result<ImageMeta, CoreError>;
    /// Embedded camera preview (JPEG) as RGBA8, orientation applied,
    /// downscaled to ~`max_dim`. Fast path for instant display.
    fn embedded_preview(
        &self,
        path: &Path,
        max_dim: u32,
    ) -> Result<Option<(Vec<u8>, u32, u32)>, CoreError>;
    /// Full decode → scene-referred linear Rec.2020 working buffer.
    fn decode(&self, path: &Path) -> Result<DecodedImage, CoreError> {
        self.decode_with_profile(path, None)
    }

    /// Full decode with an optional DCP camera profile for the base matrix.
    fn decode_with_profile(
        &self,
        path: &Path,
        profile_path: Option<&Path>,
    ) -> Result<DecodedImage, CoreError>;

    /// Full decode choosing the demosaic algorithm. The default ignores it
    /// (correct for already-rendered/non-CFA sources); `RawlerDecoder` overrides
    /// it to route between rawler's built-in demosaic and the merawler engine.
    fn decode_with_options(
        &self,
        path: &Path,
        profile_path: Option<&Path>,
        _demosaic: Demosaic,
    ) -> Result<DecodedImage, CoreError> {
        self.decode_with_profile(path, profile_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_always_includes_in_process_merawler() {
        let a = Demosaic::available();
        for name in [
            "rawler", "bilinear", "malvar", "rcd", "lmmse", "amaze", "igv", "ddfapd",
        ] {
            assert!(
                a.iter().any(|n| n == name),
                "available() missing in-process demosaic '{name}' (got {a:?})"
            );
        }
    }
}
