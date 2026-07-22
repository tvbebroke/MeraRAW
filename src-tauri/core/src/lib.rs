//! Meratech engine core. Owns pixels, GPU, and (Phase 2+) the canonical edit doc.
//! Tauri-free by design — unit-testable without a window.

pub mod catalog;
pub mod color;
pub mod crop;
pub mod curve;
pub mod denoise;
pub mod doc;
pub mod engine;
pub mod error;
pub mod export;
pub mod gpu;
pub mod graph;
pub mod image;
pub mod lut;
pub mod message;
pub mod metadata;
pub mod ops;
pub mod path_safety;
pub mod profile;
pub mod raw;
pub mod registry;
pub mod retouch;
pub mod segment;
pub mod sidecar;
