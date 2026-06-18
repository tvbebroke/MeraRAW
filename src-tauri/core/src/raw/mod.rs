//! RAW decode. Contract B1: `Decoder` trait isolates the decode dependency
//! (rawler primary — pure Rust, clean macOS build; LibRaw swap stays
//! contained here if coverage demands it).

mod rawler_decoder;

pub use rawler_decoder::RawlerDecoder;

use crate::error::CoreError;
use crate::image::RgbF32Buf;
use std::path::Path;

/// Metadata surfaced to the UI + assistant. serde camelCase for the wire.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMeta {
    pub path: String,
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
    fn decode(&self, path: &Path) -> Result<DecodedImage, CoreError>;
}
