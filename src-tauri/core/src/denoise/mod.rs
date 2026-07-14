//! Advanced denoising — classical (GPU/CPU) + AI tracks.
//! Spec: `/denoise/*.md`. Classical CPU reference lives in `cpu`; GPU passes
//! are woven into the render graph's noise slot; AI is async + cached.

pub mod ai;
pub mod cpu;
pub mod metrics;
pub mod profile;
pub mod settings;

pub use cpu::{denoise_rgb, hot_pixel_suppress, ChainParams};
pub use profile::{estimate_from_image, NoiseProfile, ProfileSource};
pub use settings::DenoiseSettings;
