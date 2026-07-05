//! Export dispatch. Live impl: `core/src/export.rs`
//! (+ tiled full-res path in `core/src/graph/export_tile.rs`).
#![allow(dead_code, unused_variables)]

use crate::pipeline::RgbImage;
use std::path::Path;

pub enum Format {
    Jpeg,
    Png,
    Tiff,
}

pub fn save(img: &RgbImage, path: &Path, format: Format) -> Result<(), String> {
    match format {
        Format::Jpeg => crate::export::jpeg::encode(img, path),
        Format::Png => crate::export::png::encode(img, path),
        Format::Tiff => crate::export::tiff::encode(img, path),
    }
}
