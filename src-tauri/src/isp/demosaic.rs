//! Bayer -> linear RGB. The shipping build demosaics inside `rawler`.
#![allow(dead_code, unused_variables)]

use crate::pipeline::{RawImage, RgbImage};

#[derive(Clone, Copy)]
pub enum DemosaicAlgo {
    Bilinear,
    Malvar,
    Ahd,
    Amaze,
}

pub fn demosaic(raw: &RawImage, algo: DemosaicAlgo) -> RgbImage {
    match algo {
        DemosaicAlgo::Bilinear => bilinear(raw),
        // Higher-quality kernels (Malvar/AHD/AMaZE) TODO — see rawler.
        _ => bilinear(raw),
    }
}

fn bilinear(raw: &RawImage) -> RgbImage {
    // Minimal channel-drop placeholder so the scaffold type-checks; real
    // interpolation is done by rawler in the shipping decoder.
    let mut out = RgbImage::new(raw.width, raw.height);
    for y in 0..raw.height {
        for x in 0..raw.width {
            let s = raw.data[y * raw.width + x] as f32 / u16::MAX as f32;
            let c = raw.cfa.color_at(x, y) as usize;
            let i = (y * raw.width + x) * 3;
            out.data[i + c] = s; // TODO: interpolate the two missing channels
        }
    }
    out
}
