//! Render graph — contract A5. Chain: working master → extract (view region
//! @ viewport res) → module nodes in fixed order (spec 3.2) → mask stage
//! (per mask: scoped module stack + blend, spec 4.5) → present (display
//! transform, always terminal) → RGBA8 readback.
//!
//! Cache-the-chain: each global node caches its output; an op dirties its
//! stage and rendering re-runs from the first dirty stage forward. The
//! mask stage re-runs as a unit when any mask changes (per-mask caching is
//! a perf-hardening item). Identity-at-default nodes are skipped.

mod config;
mod export_tile;
mod render;
mod resources;

pub use config::NODES;
pub use resources::upload_small_mask;

use resources::{DcpMeta, PassResources};
use std::collections::HashMap;

/// Index of the mask-composite stage (after the global nodes).
pub fn mask_stage_index() -> usize {
    NODES.len()
}

pub struct RenderGraph {
    extract: PassResources,
    present: PassResources,
    simple_pipes: HashMap<&'static str, PassResources>,
    curve_pipe: PassResources,
    lut_pipe: PassResources,
    mask_geom: PassResources,
    mask_sample: PassResources,
    blend: PassResources,
    sampler: wgpu::Sampler,
    extract_uniforms: wgpu::Buffer,
    present_uniforms: wgpu::Buffer,
    node_uniforms: Vec<wgpu::Buffer>,
    lut_buffer: wgpu::Buffer,
    /// Dedicated storage buffer for the 3D LUT node (never aliases lut_buffer).
    lut3d_buffer: wgpu::Buffer,
    /// per-render uniform pool for mask passes (write once per render)
    pool: Vec<wgpu::Buffer>,
    lut_pool: Vec<wgpu::Buffer>,
    strokes_pool: Vec<wgpu::Buffer>,
    dummy_mask: wgpu::Texture,
    extract_tex: Option<wgpu::Texture>,
    node_tex: Vec<Option<wgpu::Texture>>,
    mask_tex: HashMap<String, wgpu::Texture>,
    scratch: Vec<wgpu::Texture>,   // 2× local-stack ping-pong
    composite: Vec<wgpu::Texture>, // 2× composite ping-pong
    out_tex: Option<wgpu::Texture>,
    cache_size: Option<(u32, u32)>,
    last_view_key: Option<[u32; 5]>,
    last_crop_key: Option<u64>,
    /// First dirty stage index (NODES.len() = mask stage). usize::MAX = clean.
    dirty_from: usize,
    pub last_passes_run: Vec<String>,
    /// What fed present on the last render (export reads this).
    last_final: FinalTag,
    /// Display look: 0 = Neutral, 1 = Camera (punchy).
    look: u32,
    /// Highlight / shadow clipping overlays (viewport only).
    clip_hi: bool,
    clip_lo: bool,
    /// Soft-proof target: 0 off, 1 sRGB, 2 P3, 3 Adobe RGB, 4 ProPhoto.
    proof_space: u32,
    proof_gamut: bool,

    // ---- DCP look (GPU port; replaces the CPU readback pass) ----
    dcp_look: PassResources,
    dcp_look_uniforms: wgpu::Buffer,
    /// Looked extract, persisted across renders (only re-run on view change).
    look_tex: Option<wgpu::Texture>,
    /// HSV delta tables (map1|map2|look concatenated) + tone LUT, uploaded once
    /// per profile; `dcp_sig` keys the cache, `dcp_meta` holds offsets/dims.
    dcp_tables_buf: Option<wgpu::Buffer>,
    dcp_tone_buf: Option<wgpu::Buffer>,
    dcp_sig: Option<String>,
    dcp_meta: Option<DcpMeta>,
}

#[derive(Clone, Copy)]
pub(super) enum FinalTag {
    Extract,
    Node(usize),
    Comp(usize),
}
