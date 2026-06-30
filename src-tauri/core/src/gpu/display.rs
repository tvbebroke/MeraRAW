//! View parameters + working-master upload. The display transform itself
//! lives in the render graph (graph/present.wgsl) as the terminal node.

use super::GpuContext;

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewParams {
    pub out_w: u32,
    pub out_h: u32,
    /// Output px per image px. None = fit to (out_w, out_h).
    pub scale: Option<f32>,
    /// View center in normalized image coords. Default (0.5, 0.5).
    pub center_x: f32,
    pub center_y: f32,
    /// When true, extract shows the full master (crop tool editing overlay).
    #[serde(default)]
    pub crop_preview: bool,
}

impl ViewParams {
    pub fn fit(out_w: u32, out_h: u32) -> Self {
        Self {
            out_w,
            out_h,
            scale: None,
            center_x: 0.5,
            center_y: 0.5,
            crop_preview: false,
        }
    }

    pub fn effective_scale(&self, img_w: u32, img_h: u32) -> f32 {
        self.scale.unwrap_or_else(|| {
            (self.out_w as f32 / img_w as f32).min(self.out_h as f32 / img_h as f32)
        })
    }

    pub fn effective_scale_crop(
        &self,
        img_w: u32,
        img_h: u32,
        crop: &crate::crop::CropParams,
    ) -> f32 {
        let (ew, eh) = if crop.apply_enabled(self.crop_preview) {
            crop.effective_size(img_w, img_h)
        } else {
            (img_w as f32, img_h as f32)
        };
        self.scale
            .unwrap_or_else(|| (self.out_w as f32 / ew).min(self.out_h as f32 / eh))
    }
}

/// Upload a working buffer as the RGBA16F working master (contract A4).
pub fn upload_working_texture(
    gpu: &GpuContext,
    rgba_f16: &[u8],
    width: u32,
    height: u32,
) -> wgpu::Texture {
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("working-master"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba_f16,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 8),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    tex
}
