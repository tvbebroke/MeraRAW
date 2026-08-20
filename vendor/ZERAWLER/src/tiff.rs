//! Read the worker binaries' TIFF output into an [`RgbImage`].

use crate::{Error, RgbImage};
use std::path::Path;

/// Load an 8/16-bit or 32-bit-float RGB(A) TIFF as normalized f32.
/// Float TIFFs are read losslessly (no 16-bit quantization).
pub fn read_rgb(path: &Path) -> Result<RgbImage, Error> {
    let mut reader = image::ImageReader::open(path)
        .map_err(|e| Error::Output(format!("{}: {e}", path.display())))?;
    // 24 MP float TIFFs exceed the crate's default decode budget, but keep an
    // explicit ceiling — worker output is semi-trusted and a corrupt header
    // must not be able to request an unbounded allocation.
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    limits.max_alloc = Some(2 * 1024 * 1024 * 1024); // 2 GiB
    reader.limits(limits);
    let img = reader
        .decode()
        .map_err(|e| Error::Output(format!("{}: {e}", path.display())))?;
    let (w, h) = (img.width() as usize, img.height() as usize);
    let data = match img {
        image::DynamicImage::ImageRgb32F(buf) => buf.pixels().map(|p| [p[0], p[1], p[2]]).collect(),
        image::DynamicImage::ImageRgba32F(buf) => {
            buf.pixels().map(|p| [p[0], p[1], p[2]]).collect()
        }
        other => other
            .to_rgb16()
            .pixels()
            .map(|p| {
                [
                    p[0] as f32 / 65535.0,
                    p[1] as f32 / 65535.0,
                    p[2] as f32 / 65535.0,
                ]
            })
            .collect(),
    };
    Ok(RgbImage {
        width: w,
        height: h,
        data,
    })
}
