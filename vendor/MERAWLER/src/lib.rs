//! MERAWLER — a custom RAW demosaic engine for MeraRAW.
//!
//! The engine is split so the interesting math has no external dependencies:
//!
//! * [`image`] — the [`CfaImage`] (normalized Bayer mosaic) and [`RgbImage`]
//!   value types every algorithm shares.
//! * [`demosaic`] — the [`Demosaic`] trait plus the algorithm implementations.
//!   These take a [`CfaImage`] and return an [`RgbImage`]; they never touch a
//!   file or a decoder, so they can be unit-tested against synthetic mosaics.
//! * [`rawler_adapter`] (feature `decode`) — turns a decoded `rawler::RawImage`
//!   into a [`CfaImage`] (black/white-level normalization + pattern detection).
//!
//! Roadmap of demosaic algorithms (see [`demosaic::Algorithm`]):
//! Bilinear and Malvar ship today as baselines; RCD, LMMSE, AMaZE, IGV and
//! DDFAPD are the planned high-quality algorithms and slot into the same trait.

pub mod demosaic;
pub mod image;

#[cfg(feature = "decode")]
pub mod rawler_adapter;

pub use demosaic::{demosaic, Algorithm, Demosaic};
pub use image::{CfaImage, CfaPattern, RgbImage};
