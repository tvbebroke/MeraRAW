//! Filmic / ACES-style tone mappers. A selectable display transform not yet in
//! the shipping build — implemented for real here so it can become a curve mode.
#![allow(dead_code)]

/// ACES filmic approximation (Narkowicz 2015).
pub fn aces(x: f32) -> f32 {
    let (a, b, c, d, e) = (2.51, 0.03, 2.43, 0.59, 0.14);
    ((x * (a * x + b)) / (x * (c * x + d) + e)).clamp(0.0, 1.0)
}

/// Reinhard.
pub fn reinhard(x: f32) -> f32 {
    x / (1.0 + x)
}

/// Hable / Uncharted-2 filmic (un-normalized partial).
pub fn hable(x: f32) -> f32 {
    let (a, b, c, d, e, f) = (0.15, 0.50, 0.10, 0.20, 0.02, 0.30);
    ((x * (a * x + c * b) + d * e) / (x * (a * x + b) + d * f)) - e / f
}
