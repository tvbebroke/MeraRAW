//! Pipeline setup, bind-group layouts, and texture/buffer creation.

use super::{FinalTag, RenderGraph, NODES};
use crate::gpu::GpuContext;
use std::collections::HashMap;

/// Crop mapping fields shared by extract/mask uniforms — one struct so the
/// three shaders can never drift apart on the coordinate contract.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct CropUniform {
    pub crop_left: f32,
    pub crop_top: f32,
    pub crop_right: f32,
    pub crop_bottom: f32,
    pub crop_angle: f32,
    pub crop_rotate_90: u32,
    pub crop_flip_h: u32,
    pub crop_flip_v: u32,
    pub crop_mode: u32,
}

impl CropUniform {
    pub fn new(crop: &crate::crop::CropParams, mode: u32) -> Self {
        Self {
            crop_left: crop.left,
            crop_top: crop.top,
            crop_right: crop.right,
            crop_bottom: crop.bottom,
            crop_angle: crop.angle.to_radians(),
            crop_rotate_90: crop.rotate_90,
            crop_flip_h: u32::from(crop.flip_h),
            crop_flip_v: u32::from(crop.flip_v),
            crop_mode: mode,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct ExtractUniforms {
    pub out_w: u32,
    pub out_h: u32,
    pub img_w: f32,
    pub img_h: f32,
    pub scale: f32,
    pub center_x: f32,
    pub center_y: f32,
    pub crop: CropUniform,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct PresentUniforms {
    pub width: u32,
    pub height: u32,
    pub overlay: f32,
    pub look: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct MaskGeomUniforms {
    pub out_w: u32,
    pub out_h: u32,
    pub img_w: f32,
    pub img_h: f32,
    pub scale: f32,
    pub center_x: f32,
    pub center_y: f32,
    pub kind: u32,
    pub pa: [f32; 2],
    pub pb: [f32; 2],
    pub rotation: f32,
    pub feather: f32,
    pub opacity: f32,
    pub invert: u32,
    pub stroke_count: u32,
    pub crop: CropUniform,
    pub _p0: u32,
    pub _p1: u32,
    pub _p2: u32,
    pub _p3: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct MaskSampleUniforms {
    pub out_w: u32,
    pub out_h: u32,
    pub img_w: f32,
    pub img_h: f32,
    pub scale: f32,
    pub center_x: f32,
    pub center_y: f32,
    pub feather: f32,
    pub opacity: f32,
    pub invert: u32,
    pub crop: CropUniform,
    pub _p0: u32,
    pub _p1: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct BlendUniforms {
    pub width: u32,
    pub height: u32,
    pub _p0: u32,
    pub _p1: u32,
}

/// Per-render uniform for the DCP look pass — field order matches `struct U`
/// in dcp_look.wgsl exactly (20 scalars, 80 bytes).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct DcpLookUniforms {
    pub width: u32,
    pub height: u32,
    pub has_map1: u32,
    pub has_map2: u32,
    pub has_look: u32,
    pub tone_size: u32,
    pub cct_weight: f32,
    pub baseline_gain: f32,
    pub m1: [u32; 4], // off, hue_div, sat_div, val_div
    pub m2: [u32; 4],
    pub lk: [u32; 4],
}

/// Profile-dependent look constants, cached with the uploaded tables. Only
/// width/height/cct_weight vary per render; the rest come from here.
#[derive(Clone)]
pub(super) struct DcpMeta {
    pub has_map1: u32,
    pub has_map2: u32,
    pub has_look: u32,
    pub tone_size: u32,
    pub baseline_gain: f32,
    pub t1: f32,
    pub t2: f32,
    pub m1: [u32; 4],
    pub m2: [u32; 4],
    pub lk: [u32; 4],
}

pub(super) struct PassResources {
    pub pipeline: wgpu::ComputePipeline,
    pub layout: wgpu::BindGroupLayout,
}

pub(super) fn make_pass(
    gpu: &GpuContext,
    label: &str,
    wgsl: &str,
    entries: &[wgpu::BindGroupLayoutEntry],
) -> PassResources {
    let shader = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(wgsl.into()),
        });
    let layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries,
        });
    let pl = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(label),
            layout: Some(&pl),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
    PassResources { pipeline, layout }
}

fn bgl_tex(binding: u32, filterable: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn bgl_storage_tex(binding: u32, format: wgpu::TextureFormat) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

fn bgl_uniform(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn bgl_storage_buf(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn bgl_sampler(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

pub(super) fn make_tex(
    gpu: &GpuContext,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    label: &str,
) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

pub(super) fn make_chain_tex(gpu: &GpuContext, w: u32, h: u32, label: &str) -> wgpu::Texture {
    make_tex(
        gpu,
        w,
        h,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC // export tiles read the chain
            | wgpu::TextureUsages::COPY_DST, // DCP look re-upload after CPU pass
        label,
    )
}

pub(super) fn make_mask_tex(gpu: &GpuContext, w: u32, h: u32, label: &str) -> wgpu::Texture {
    make_tex(
        gpu,
        w,
        h,
        wgpu::TextureFormat::R32Float,
        wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        label,
    )
}

/// Upload a small CPU segmentation mask as an R16Float texture
/// (filterable — sampled by mask_sample.wgsl).
pub fn upload_small_mask(
    gpu: &GpuContext,
    data: &[f32],
    w: u32,
    h: u32,
) -> wgpu::Texture {
    let tex = make_tex(
        gpu,
        w,
        h,
        wgpu::TextureFormat::R16Float,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        "segmentation-mask",
    );
    let bytes: Vec<u8> = data
        .iter()
        .flat_map(|v| half::f16::from_f32(*v).to_le_bytes())
        .collect();
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 2),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    tex
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum PipeKind {
    Matrix,
    Calibration,
    Noise,
    Grade,
    Hsl,
    Curve,
    Lut3d,
    Sharpen,
}

impl PipeKind {
    /// Nodes that bind a storage-buffer LUT at binding 3 (curve LUT / 3D cube).
    pub(super) fn has_lut(self) -> bool {
        matches!(self, PipeKind::Curve | PipeKind::Lut3d)
    }
}

pub(super) const NODE_PIPES: &[PipeKind] = &[
    PipeKind::Matrix,
    PipeKind::Matrix,
    PipeKind::Calibration,
    PipeKind::Noise,
    PipeKind::Grade,
    PipeKind::Hsl,
    PipeKind::Curve,
    PipeKind::Lut3d,
    PipeKind::Sharpen,
];

/// Storage-buffer size for the 3D LUT node: MAX_SIZE³ × 3 channels × f32.
pub(super) const LUT3D_BUF_BYTES: u64 =
    (crate::lut::MAX_SIZE * crate::lut::MAX_SIZE * crate::lut::MAX_SIZE * 3 * 4) as u64;

pub(super) const MAX_STROKE_POINTS: usize = 512;

impl RenderGraph {
    pub fn new(gpu: &GpuContext) -> Self {
        let simple_bgl = [
            bgl_tex(0, true),
            bgl_storage_tex(1, wgpu::TextureFormat::Rgba16Float),
            bgl_uniform(2),
        ];
        // crop_common.wgsl holds the shared view→original-uv mapping; WGSL has
        // no include, so prepend it to every shader that inverse-maps the view.
        let with_crop =
            |src: &str| -> String { format!("{}\n{}", include_str!("crop_common.wgsl"), src) };
        let extract = make_pass(
            gpu,
            "extract",
            &with_crop(include_str!("extract.wgsl")),
            &[
                bgl_tex(0, true),
                bgl_sampler(1),
                bgl_storage_tex(2, wgpu::TextureFormat::Rgba16Float),
                bgl_uniform(3),
            ],
        );
        let present = make_pass(
            gpu,
            "present",
            include_str!("present.wgsl"),
            &[
                bgl_tex(0, true),
                bgl_storage_tex(1, wgpu::TextureFormat::Rgba8Unorm),
                bgl_uniform(2),
                bgl_tex(3, false),
            ],
        );
        let mut simple_pipes = HashMap::new();
        for (name, src) in [
            ("matrix", include_str!("color_matrix.wgsl")),
            ("calibration", include_str!("calibration.wgsl")),
            ("noise", include_str!("noise.wgsl")),
            ("grade", include_str!("grade.wgsl")),
            ("hsl", include_str!("hsl.wgsl")),
            ("sharpen", include_str!("sharpen.wgsl")),
        ] {
            simple_pipes.insert(name, make_pass(gpu, name, src, &simple_bgl));
        }
        let curve_pipe = make_pass(
            gpu,
            "tone-curve",
            include_str!("curve.wgsl"),
            &[
                bgl_tex(0, true),
                bgl_storage_tex(1, wgpu::TextureFormat::Rgba16Float),
                bgl_uniform(2),
                bgl_storage_buf(3),
            ],
        );
        // Same bind shape as the curve node (tex, storage_tex, uniform, LUT buf).
        let lut_pipe = make_pass(
            gpu,
            "lut3d",
            include_str!("lut.wgsl"),
            &[
                bgl_tex(0, true),
                bgl_storage_tex(1, wgpu::TextureFormat::Rgba16Float),
                bgl_uniform(2),
                bgl_storage_buf(3),
            ],
        );
        let mask_geom = make_pass(
            gpu,
            "mask-geom",
            &with_crop(include_str!("mask_geom.wgsl")),
            &[
                bgl_storage_tex(0, wgpu::TextureFormat::R32Float),
                bgl_uniform(1),
                bgl_storage_buf(2),
            ],
        );
        let mask_sample = make_pass(
            gpu,
            "mask-sample",
            &with_crop(include_str!("mask_sample.wgsl")),
            &[
                bgl_tex(0, true),
                bgl_sampler(1),
                bgl_tex(2, true),
                bgl_storage_tex(3, wgpu::TextureFormat::R32Float),
                bgl_uniform(4),
            ],
        );
        let blend = make_pass(
            gpu,
            "blend",
            include_str!("blend.wgsl"),
            &[
                bgl_tex(0, true),
                bgl_tex(1, true),
                bgl_tex(2, false),
                bgl_storage_tex(3, wgpu::TextureFormat::Rgba16Float),
                bgl_uniform(4),
            ],
        );

        let dcp_look = make_pass(
            gpu,
            "dcp-look",
            include_str!("dcp_look.wgsl"),
            &[
                bgl_tex(0, false),
                bgl_storage_tex(1, wgpu::TextureFormat::Rgba16Float),
                bgl_uniform(2),
                bgl_storage_buf(3),
                bgl_storage_buf(4),
            ],
        );

        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("graph-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let mk_uniform = |size: u64, label: &str| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let node_uniforms = NODES.iter().map(|(n, _)| mk_uniform(512, n)).collect();
        let lut_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tone-lut"),
            size: (crate::curve::LUT_SIZE * 4 * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Dedicated buffer for the 3D LUT node so it never aliases the shared
        // curve LUT buffer when both nodes run in one submission.
        let lut3d_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lut3d"),
            size: LUT3D_BUF_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dummy_mask = make_mask_tex(gpu, 1, 1, "dummy-mask");

        Self {
            extract,
            present,
            simple_pipes,
            curve_pipe,
            lut_pipe,
            mask_geom,
            mask_sample,
            blend,
            sampler,
            extract_uniforms: mk_uniform(std::mem::size_of::<ExtractUniforms>() as u64, "extract-u"),
            present_uniforms: mk_uniform(std::mem::size_of::<PresentUniforms>() as u64, "present-u"),
            node_uniforms,
            lut_buffer,
            lut3d_buffer,
            pool: Vec::new(),
            lut_pool: Vec::new(),
            strokes_pool: Vec::new(),
            dummy_mask,
            extract_tex: None,
            node_tex: vec![None; NODES.len()],
            mask_tex: HashMap::new(),
            scratch: Vec::new(),
            composite: Vec::new(),
            out_tex: None,
            cache_size: None,
            last_view_key: None,
            last_crop_key: None,
            dirty_from: 0,
            last_passes_run: Vec::new(),
            last_final: FinalTag::Extract,
            look: 0,
            dcp_look,
            dcp_look_uniforms: mk_uniform(
                std::mem::size_of::<DcpLookUniforms>() as u64,
                "dcp-look-u",
            ),
            look_tex: None,
            dcp_tables_buf: None,
            dcp_tone_buf: None,
            dcp_sig: None,
            dcp_meta: None,
        }
    }
}
