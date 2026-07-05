//! Decode a RAW file -> mosaiced sensor data + metadata.
//!
//! Live implementation: `core/src/raw/rawler_decoder.rs` (pure-Rust `rawler`;
//! supplies black/white level, WB multipliers, camera->XYZ matrix, demosaic).
#![allow(dead_code, unused_variables)]

use crate::pipeline::RawImage;
use std::path::Path;

pub fn load(path: &Path) -> Result<RawImage, String> {
    todo!("scaffold — delegate to core/src/raw/rawler_decoder.rs")
}
