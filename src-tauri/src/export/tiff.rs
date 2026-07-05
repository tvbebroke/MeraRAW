//! tiff encode. Live impl: `core/src/export.rs` (image crate + ICC/EXIF embedding).
#![allow(dead_code, unused_variables)]

use crate::pipeline::RgbImage;
use std::path::Path;

pub fn encode(img: &RgbImage, path: &Path) -> Result<(), String> {
    todo!("scaffold — delegate to core/src/export.rs")
}
