//! Lanczos resize isolated in its own crate so the generic
//! `image::imageops::resize` monomorphizes here, where the dev profile
//! sets opt-level = 2 (in meratech-core it would compile at opt-level 0
//! and a 33MP export resize takes minutes in debug builds).

/// Resize an RGB f32 buffer so its long edge is `max_dim`, Lanczos3.
/// Returns the buffer unchanged when it already fits, or when the buffer
/// length does not match `width * height * 3` (never panics).
pub fn resize_rgb_f32(
    linear: Vec<f32>,
    width: u32,
    height: u32,
    max_dim: u32,
) -> (Vec<f32>, u32, u32) {
    let Some(expected) = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(3))
    else {
        return (linear, width, height);
    };
    if linear.len() != expected {
        return (linear, width, height);
    }
    if width.max(height) <= max_dim {
        return (linear, width, height);
    }
    let Some(img) = image::Rgb32FImage::from_raw(width, height, linear) else {
        // Length was checked; this branch should be unreachable.
        return (Vec::new(), width, height);
    };
    let scale = max_dim as f32 / width.max(height) as f32;
    let (nw, nh) = (
        ((width as f32 * scale) as u32).max(1),
        ((height as f32 * scale) as u32).max(1),
    );
    let resized = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Lanczos3);
    (resized.into_raw(), nw, nh)
}
