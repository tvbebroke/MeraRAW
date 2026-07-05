//! Tone stage: exposure, contrast, curves, filmic, master tone curve.
//! Live impl: `core/src/curve.rs` + `core/src/graph/curve.wgsl`
//! (params: `exposure.stops`, `tone_curve.*`).
pub mod contrast;
pub mod curves;
pub mod exposure;
pub mod filmic;
pub mod tone_curve;
