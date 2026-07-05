//! End-to-end RAW pipeline wiring (reference scaffold).
//!
//! Mirrors the documented stage order. This is a *reference skeleton* — the
//! live, GPU-accelerated pipeline the app actually runs lives in the
//! `meratech-core` crate (`core/src/graph/*.wgsl` + `core/src/graph/render.rs`).
//! Each stage module here points at its real counterpart so the two never get
//! mistaken for one another.
#![allow(dead_code, unused_variables)]

use crate::raw::bayer::CfaPattern;
use std::path::Path;

/// Mosaiced sensor data straight off the decoder.
pub struct RawImage {
    pub width: usize,
    pub height: usize,
    pub cfa: CfaPattern,
    /// Row-major single-channel sensor samples.
    pub data: Vec<u16>,
    pub black_level: [u16; 4],
    pub white_level: u16,
    /// As-shot white-balance channel multipliers (R, G1, B, G2).
    pub wb_coeffs: [f32; 4],
    /// Camera-native RGB -> CIE XYZ (D65) matrix.
    pub cam_to_xyz: [[f32; 3]; 3],
}

/// Interleaved linear RGB working image (f32 per channel, len = w*h*3).
pub struct RgbImage {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl RgbImage {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, data: vec![0.0; width * height * 3] }
    }
}

/// Creative grading parameters (subset — the shipping app's full surface is in
/// `core/src/registry.rs`).
#[derive(Default, Clone)]
pub struct GradeParams {
    pub exposure_stops: f32,
    pub contrast: f32,
}

/// Documented flow:
/// RAW -> black level -> demosaic -> WB -> camera matrix -> XYZ ->
/// working space -> exposure -> tone curve -> grade -> output space.
pub fn run(path: &Path, grade: &GradeParams) -> RgbImage {
    let mut raw = crate::raw::loader::load(path).expect("decode raw");
    crate::raw::calibration::apply_black_level(&mut raw);

    let mut rgb =
        crate::isp::demosaic::demosaic(&raw, crate::isp::demosaic::DemosaicAlgo::Malvar);
    let wb = [raw.wb_coeffs[0], raw.wb_coeffs[1], raw.wb_coeffs[2]];
    crate::isp::white_balance::apply_white_balance(&mut rgb, wb);

    crate::color::camera_matrix::camera_to_xyz(&mut rgb, raw.cam_to_xyz);
    crate::color::color_space::xyz_to_working(&mut rgb, crate::color::color_space::WorkingSpace::Rec2020);

    crate::tone::exposure::apply(&mut rgb, grade.exposure_stops);
    crate::tone::tone_curve::apply(&mut rgb);
    crate::grading::grading_pipeline::apply(&mut rgb, grade);

    crate::color::color_space::working_to_output(&mut rgb, crate::color::color_space::WorkingSpace::Srgb);
    rgb
}
