//! EXIF / camera metadata carried alongside the RAW.
//! Live impl: core decoder + `core/src/profile/`.
#![allow(dead_code)]

#[derive(Default, Clone)]
pub struct RawMetadata {
    pub make: String,
    pub model: String,
    pub iso: u32,
    pub exposure_time: f32,
    pub aperture: f32,
    pub focal_length: f32,
    pub orientation: u16,
}
