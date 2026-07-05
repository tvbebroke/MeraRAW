//! Black-level subtraction + sensor normalization to [0,1].
//! Live equivalent runs on the GPU: `core/src/graph/calibration.wgsl`.
#![allow(dead_code, unused_variables)]

use crate::pipeline::RawImage;

pub fn apply_black_level(raw: &mut RawImage) {
    let bl = raw.black_level[0];
    let range = (raw.white_level.saturating_sub(bl)).max(1) as f32;
    for v in raw.data.iter_mut() {
        let n = ((*v).saturating_sub(bl) as f32 / range).clamp(0.0, 1.0);
        *v = (n * u16::MAX as f32) as u16;
    }
}
