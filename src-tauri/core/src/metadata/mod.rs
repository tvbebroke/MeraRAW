//! EXIF + input ICC helpers (phases 11 / 11.2).

mod exif_read;
mod icc;

pub use exif_read::enrich_from_file;
pub use icc::{encoded_rgb_to_working, extract_icc, probe_input_color, InputColorInfo};
