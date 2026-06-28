//! CPU ↔ GPU texture I/O helpers (RGBA16F readback/upload, DCP look pass).

use crate::error::CoreError;
use crate::gpu::GpuContext;
use crate::profile::DcpProfile;

/// Read an RGBA16F texture back as linear RGB f32 (alpha dropped).
pub fn readback_rgba16f_from_texture(
    gpu: &GpuContext,
    tex: &wgpu::Texture,
    out_w: u32,
    out_h: u32,
) -> Result<Vec<f32>, CoreError> {
    let bytes_per_row_packed = out_w * 8; // RGBA16F
    let bytes_per_row = bytes_per_row_packed.div_ceil(256) * 256;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("linear-readback"),
        size: (bytes_per_row * out_h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("linear-readback-encoder"),
        });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(out_h),
            },
        },
        wgpu::Extent3d {
            width: out_w,
            height: out_h,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    gpu.device.poll(wgpu::Maintain::Wait);
    rx.recv()
        .map_err(|_| CoreError::Gpu("readback dropped".into()))?
        .map_err(|e| CoreError::Gpu(format!("map: {e:?}")))?;
    let data = slice.get_mapped_range();
    // RGBA f16 → RGB f32
    let mut out = Vec::with_capacity((out_w * out_h * 3) as usize);
    for row in 0..out_h {
        let off = (row * bytes_per_row) as usize;
        for x in 0..out_w {
            let i = off + (x * 8) as usize;
            for c in 0..3 {
                let b = [data[i + c * 2], data[i + c * 2 + 1]];
                out.push(half::f16::from_le_bytes(b).to_f32());
            }
        }
    }
    drop(data);
    readback.unmap();
    Ok(out)
}

/// Upload RGBA f32 pixels into an RGBA16F texture (row padding per wgpu rules).
pub fn upload_rgba16f(
    gpu: &GpuContext,
    tex: &wgpu::Texture,
    out_w: u32,
    out_h: u32,
    pixels: &[f32],
) {
    debug_assert_eq!(
        pixels.len(),
        (out_w * out_h * 4) as usize,
        "upload_rgba16f expects RGBA f32"
    );
    let bytes_per_row_packed = out_w * 8;
    let bytes_per_row = bytes_per_row_packed.div_ceil(256) * 256;
    let mut bytes = vec![0u8; (bytes_per_row * out_h) as usize];
    for row in 0..out_h {
        let src_row = (row * out_w * 4) as usize;
        let dst_off = (row * bytes_per_row) as usize;
        for x in 0..out_w {
            let si = src_row + (x * 4) as usize;
            let di = dst_off + (x * 8) as usize;
            for c in 0..4 {
                let f = half::f16::from_f32(pixels[si + c]).to_le_bytes();
                bytes[di + c * 2] = f[0];
                bytes[di + c * 2 + 1] = f[1];
            }
        }
    }
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(bytes_per_row),
            rows_per_image: Some(out_h),
        },
        wgpu::Extent3d {
            width: out_w,
            height: out_h,
            depth_or_array_layers: 1,
        },
    );
}

/// Expand interleaved RGB f32 to RGBA (alpha = 1.0).
pub fn rgb_f32_to_rgba_f32(rgb: &[f32]) -> Vec<f32> {
    let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
    for px in rgb.chunks(3) {
        rgba.extend_from_slice(px);
        rgba.push(1.0);
    }
    rgba
}

/// Apply DCP look table in-place on RGBA f32 pixels (RGB channels only).
pub fn apply_dcp_look_f16(pixels: &mut [f32], dcp: &DcpProfile, cct: f32) {
    for chunk in pixels.chunks_mut(4) {
        if chunk.len() < 3 {
            break;
        }
        let rgb = dcp.apply_look([chunk[0], chunk[1], chunk[2]], cct);
        chunk[0] = rgb[0];
        chunk[1] = rgb[1];
        chunk[2] = rgb[2];
    }
}
