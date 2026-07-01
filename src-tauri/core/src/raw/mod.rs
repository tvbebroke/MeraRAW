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
}
