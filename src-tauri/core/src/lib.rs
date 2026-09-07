//! Meratech engine core. Owns pixels, GPU, and (Phase 2+) the canonical edit doc.
//! Tauri-free by design — unit-testable without a window.
//!
//! Clippy allows: color-science matrices carry extra digits on purpose
//! (rounding them is a silent color bug); index-style loops match the
//! papers they implement; export-worker IIFEs exist so `?` works off-thread.
#![allow(clippy::excessive_precision)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::redundant_closure_call)]

pub mod catalog;
pub mod color;
pub mod crop;
pub mod curve;
pub mod dehaze;
pub mod denoise;
pub mod doc;
pub mod engine;
pub mod error;
pub mod export;
pub mod face_detect;
pub mod gpu;
pub mod graph;
pub mod highlights;
pub mod idt;
pub mod image;
pub mod look;
pub mod lut;
pub mod message;
pub mod metadata;
pub mod ops;
pub mod path_safety;
pub mod pipeline;
pub mod profile;
pub mod raw;
pub mod registry;
pub mod retouch;
pub mod segment;
pub mod sidecar;
pub mod video;
