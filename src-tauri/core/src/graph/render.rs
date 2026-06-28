//! Render pass execution, cache invalidation, and module chain.

use super::config::{mask_node_configs, node_configs, NodeConfig};
use super::mask_stage_index;
use super::resources::{
    make_chain_tex, make_mask_tex, make_tex, BlendUniforms, ExtractUniforms, MaskGeomUniforms,
    MaskSampleUniforms, PassResources, PresentUniforms, PipeKind, MAX_STROKE_POINTS, NODE_PIPES,
};
use super::{FinalTag, RenderGraph, NODES};
use crate::doc::EditDoc;
use crate::error::CoreError;
use crate::gpu::display::ViewParams;
use crate::gpu::texture_io::{
    apply_dcp_look_f16, readback_rgba16f_from_texture, rgb_f32_to_rgba_f32, upload_rgba16f,
};
use crate::gpu::GpuContext;
use crate::profile::DcpProfile;
use std::collections::HashMap;

impl RenderGraph {
    /// Set the display look (false = Neutral, true = Camera/punchy).
    pub fn set_look(&mut self, camera: bool) {
        self.look = camera as u32;
    }

    pub fn look(&self) -> bool {
        self.look != 0
    }

    pub fn invalidate_from_module(&mut self, module: &str) {
        let idx = if module == "masks" {
            mask_stage_index()
        } else {
            NODES.iter().position(|(_, m)| *m == module).unwrap_or(0)
        };
        self.dirty_from = self.dirty_from.min(idx);
    }

    pub fn invalidate_all(&mut self) {
        self.dirty_from = 0;
        self.last_view_key = None;
    }

    fn ensure_pools(&mut self, gpu: &GpuContext, uniforms: usize, luts: usize, strokes: usize) {
        while self.pool.len() < uniforms {
            self.pool.push(gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mask-pool-u"),
                size: 512,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        while self.lut_pool.len() < luts {
            self.lut_pool
                .push(gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("mask-pool-lut"),
                    size: (crate::curve::LUT_SIZE * 4 * 4) as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
        }
        while self.strokes_pool.len() < strokes {
            self.strokes_pool
                .push(gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("mask-pool-strokes"),
                    size: (MAX_STROKE_POINTS * 16) as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        gpu: &GpuContext,
        working_view: &wgpu::TextureView,
        img_w: u32,
        img_h: u32,
        view: &ViewParams,
        doc: &EditDoc,
        as_shot_cct: f32,
        seg_masks: &HashMap<String, wgpu::TextureView>,
        overlay_mask: Option<&str>,
        dcp_profile: Option<&DcpProfile>,
    ) -> Result<Vec<u8>, CoreError> {
        self.last_passes_run.clear();
        // clamp to a safe texture size — never exceed the GPU 2D limit (the
        // viewport is a screen preview; 8192 is ample and panic-proof)
        const MAX_VIEW: u32 = 8192;
        let out_w = view.out_w.clamp(1, MAX_VIEW);
        let out_h = view.out_h.clamp(1, MAX_VIEW);
        let scale = view.effective_scale(img_w, img_h);
        let view_key = [
            out_w,
            out_h,
            scale.to_bits(),
            view.center_x.to_bits(),
            view.center_y.to_bits(),
        ];
        let view_changed = self.last_view_key != Some(view_key);

        // totally clean → out_tex still holds the right pixels
        if !view_changed && self.dirty_from == usize::MAX && self.out_tex.is_some() {
            return self.readback(gpu, out_w, out_h);
        }

        if self.cache_size != Some((out_w, out_h)) {
            self.extract_tex = Some(make_chain_tex(gpu, out_w, out_h, "extract-out"));
            for (i, slot) in self.node_tex.iter_mut().enumerate() {
                *slot = Some(make_chain_tex(gpu, out_w, out_h, NODES[i].0));
            }
            self.scratch = (0..2)
                .map(|i| make_chain_tex(gpu, out_w, out_h, if i == 0 { "scratch-a" } else { "scratch-b" }))
                .collect();
            self.composite = (0..2)
                .map(|i| make_chain_tex(gpu, out_w, out_h, if i == 0 { "comp-a" } else { "comp-b" }))
                .collect();
            self.mask_tex.clear();
            self.out_tex = Some(make_tex(
                gpu,
                out_w,
                out_h,
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
                "present-out",
            ));
            self.cache_size = Some((out_w, out_h));
            self.dirty_from = 0;
        } else if view_changed {
            self.dirty_from = 0;
        }

        // mask textures + pools sized up-front (avoids borrow tangles)
        for m in &doc.masks {
            if !self.mask_tex.contains_key(&m.id) {
                self.mask_tex
                    .insert(m.id.clone(), make_mask_tex(gpu, out_w, out_h, "mask"));
            }
        }
        let n_masks = doc.masks.len();
        self.ensure_pools(
            gpu,
            n_masks * (2 + NODES.len()),
            n_masks,
            n_masks,
        );

        let configs = node_configs(doc, as_shot_cct, out_w, out_h);
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("graph-encoder"),
            });

        // ---- extract ----
        let extract_tex = self.extract_tex.as_ref().unwrap();
        // Extract + dcp_look depend ONLY on the view (their inputs are the
        // working master + view params, never the doc). So they re-run on a
        // view change — which `invalidate_all` also forces via last_view_key =
        // None. Gating on `dirty_from == 0` was wrong: editing module 0
        // (exposure) sets dirty_from = 0 and needlessly re-ran the ~1.3s CPU
        // dcp_look every slider tick.
        let run_extract = view_changed;
        if run_extract {
            let u = ExtractUniforms {
                out_w,
                out_h,
                img_w: img_w as f32,
                img_h: img_h as f32,
                scale,
                center_x: view.center_x,
                center_y: view.center_y,
                _pad: 0.0,
            };
            gpu.queue
                .write_buffer(&self.extract_uniforms, 0, bytemuck::bytes_of(&u));
            let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("extract-bind"),
                layout: &self.extract.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(working_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(
                            &extract_tex.create_view(&Default::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.extract_uniforms.as_entire_binding(),
                    },
                ],
            });
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("extract"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.extract.pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(out_w.div_ceil(16), out_h.div_ceil(16), 1);
            self.last_passes_run.push("extract".into());
        }

        // DCP hue/sat + look table on the extracted viewport buffer (not full-res decode).
        if run_extract {
            if let Some(dcp) = dcp_profile.filter(|d| d.has_look()) {
                gpu.queue.submit([encoder.finish()]);
                let rgb = readback_rgba16f_from_texture(gpu, extract_tex, out_w, out_h)?;
                let mut rgba = rgb_f32_to_rgba_f32(&rgb);
                apply_dcp_look_f16(&mut rgba, dcp, as_shot_cct);
                upload_rgba16f(gpu, extract_tex, out_w, out_h, &rgba);
                self.last_passes_run.push("dcp_look".into());
                encoder = gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("graph-encoder-post-dcp"),
                    });
            }
        }

        // ---- global module chain ----
        let mut upstream: &wgpu::Texture = extract_tex;
        let mut final_tag = FinalTag::Extract;
        for (i, cfg) in configs.iter().enumerate() {
            let NodeConfig::Run { uniforms, lut } = cfg else {
                continue;
            };
            let must_run = i >= self.dirty_from || run_extract;
            let out = self.node_tex[i].as_ref().unwrap();
            if must_run {
                gpu.queue.write_buffer(&self.node_uniforms[i], 0, uniforms);
                if let Some(lut_data) = lut {
                    gpu.queue
                        .write_buffer(&self.lut_buffer, 0, bytemuck::cast_slice(lut_data));
                }
                let pipe = self.pipe_for(NODE_PIPES[i]);
                dispatch_node(
                    gpu,
                    &mut encoder,
                    pipe,
                    NODE_PIPES[i] == PipeKind::Curve,
                    upstream,
                    out,
                    &self.node_uniforms[i],
                    &self.lut_buffer,
                    NODES[i].0,
                    out_w,
                    out_h,
                );
                self.last_passes_run.push(NODES[i].0.to_string());
            }
            upstream = out;
            final_tag = FinalTag::Node(i);
        }

        // ---- mask stage ----
        let masks_run = !doc.masks.is_empty();
        if masks_run {
            let mut pool_i = 0usize;
            let mut lut_i = 0usize;
            let mut strokes_i = 0usize;
            let mut comp_flip = 0usize;
            for mask in &doc.masks {
                let mtex = &self.mask_tex[&mask.id];
                let opacity = (mask.opacity / 100.0).clamp(0.0, 1.0);
                let feather = (mask.feather / 100.0).clamp(0.0, 1.0);
                let src_type = mask
                    .source
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                let produced = match src_type {
                    "radial" | "linear" | "brush" => {
                        let (kind, pa, pb, rotation, strokes) =
                            parse_geometry(&mask.source);
                        let ub = &self.pool[pool_i];
                        pool_i += 1;
                        let sb = &self.strokes_pool[strokes_i];
                        strokes_i += 1;
                        let u = MaskGeomUniforms {
                            out_w,
                            out_h,
                            img_w: img_w as f32,
                            img_h: img_h as f32,
                            scale,
                            center_x: view.center_x,
                            center_y: view.center_y,
                            kind,
                            pa,
                            pb,
                            rotation,
                            feather,
                            opacity,
                            invert: mask.invert as u32,
                            stroke_count: strokes.len() as u32,
                            _p0: 0,
                            _p1: 0,
                            _p2: 0,
                        };
                        gpu.queue.write_buffer(ub, 0, bytemuck::bytes_of(&u));
                        if !strokes.is_empty() {
                            gpu.queue
                                .write_buffer(sb, 0, bytemuck::cast_slice(&strokes));
                        }
                        let mview = mtex.create_view(&Default::default());
                        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("mask-geom-bind"),
                            layout: &self.mask_geom.layout,
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: wgpu::BindingResource::TextureView(&mview),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: ub.as_entire_binding(),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 2,
                                    resource: sb.as_entire_binding(),
                                },
                            ],
                        });
                        let mut pass =
                            encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                label: Some("mask-geom"),
                                timestamp_writes: None,
                            });
                        pass.set_pipeline(&self.mask_geom.pipeline);
                        pass.set_bind_group(0, &bind, &[]);
                        pass.dispatch_workgroups(out_w.div_ceil(16), out_h.div_ceil(16), 1);
                        self.last_passes_run.push(format!("mask:{}", mask.kind));
                        true
                    }
                    "segmented" => {
                        if let Some(small_view) = seg_masks.get(&mask.id) {
                            let ub = &self.pool[pool_i];
                            pool_i += 1;
                            // background kind = inverse of the subject mask
                            let invert =
                                (mask.invert ^ (mask.kind == "background")) as u32;
                            let u = MaskSampleUniforms {
                                out_w,
                                out_h,
                                img_w: img_w as f32,
                                img_h: img_h as f32,
                                scale,
                                center_x: view.center_x,
                                center_y: view.center_y,
                                feather,
                                opacity,
                                invert,
                                _p0: 0,
                                _p1: 0,
                            };
                            gpu.queue.write_buffer(ub, 0, bytemuck::bytes_of(&u));
                            let mview = mtex.create_view(&Default::default());
                            let eview = extract_tex.create_view(&Default::default());
                            let bind =
                                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                                    label: Some("mask-sample-bind"),
                                    layout: &self.mask_sample.layout,
                                    entries: &[
                                        wgpu::BindGroupEntry {
                                            binding: 0,
                                            resource: wgpu::BindingResource::TextureView(
                                                small_view,
                                            ),
                                        },
                                        wgpu::BindGroupEntry {
                                            binding: 1,
                                            resource: wgpu::BindingResource::Sampler(
                                                &self.sampler,
                                            ),
                                        },
                                        wgpu::BindGroupEntry {
                                            binding: 2,
                                            resource: wgpu::BindingResource::TextureView(&eview),
                                        },
                                        wgpu::BindGroupEntry {
                                            binding: 3,
                                            resource: wgpu::BindingResource::TextureView(&mview),
                                        },
                                        wgpu::BindGroupEntry {
                                            binding: 4,
                                            resource: ub.as_entire_binding(),
                                        },
                                    ],
                                });
                            let mut pass =
                                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                    label: Some("mask-sample"),
                                    timestamp_writes: None,
                                });
                            pass.set_pipeline(&self.mask_sample.pipeline);
                            pass.set_bind_group(0, &bind, &[]);
                            pass.dispatch_workgroups(
                                out_w.div_ceil(16),
                                out_h.div_ceil(16),
                                1,
                            );
                            self.last_passes_run.push(format!("mask:{}", mask.kind));
                            true
                        } else {
                            false // inference pending — mask contributes nothing yet
                        }
                    }
                    _ => false,
                };
                if !produced {
                    continue;
                }

                // local module stack (same passes, scoped params)
                let cfgs = mask_node_configs(mask, as_shot_cct, out_w, out_h);
                let mut local_src: &wgpu::Texture = upstream;
                let mut flip = 0usize;
                let mut ran_local = false;
                for (i, cfg) in cfgs.iter().enumerate() {
                    let NodeConfig::Run { uniforms, lut } = cfg else {
                        continue;
                    };
                    let ub = &self.pool[pool_i];
                    pool_i += 1;
                    gpu.queue.write_buffer(ub, 0, uniforms);
                    let lut_buf = if let Some(lut_data) = lut {
                        let lb = &self.lut_pool[lut_i];
                        lut_i += 1;
                        gpu.queue.write_buffer(lb, 0, bytemuck::cast_slice(lut_data));
                        lb
                    } else {
                        &self.lut_buffer
                    };
                    let out = &self.scratch[flip];
                    flip ^= 1;
                    let pipe = self.pipe_for(NODE_PIPES[i]);
                    dispatch_node(
                        gpu,
                        &mut encoder,
                        pipe,
                        NODE_PIPES[i] == PipeKind::Curve,
                        local_src,
                        out,
                        ub,
                        lut_buf,
                        NODES[i].0,
                        out_w,
                        out_h,
                    );
                    self.last_passes_run
                        .push(format!("mask-local:{}", NODES[i].0));
                    local_src = out;
                    ran_local = true;
                }
                if !ran_local {
                    continue; // mask has no edits — nothing to blend
                }

                // blend over the running composite
                let dst = &self.composite[comp_flip];
                comp_flip ^= 1;
                let ub = &self.pool[pool_i];
                pool_i += 1;
                gpu.queue.write_buffer(
                    ub,
                    0,
                    bytemuck::bytes_of(&BlendUniforms {
                        width: out_w,
                        height: out_h,
                        _p0: 0,
                        _p1: 0,
                    }),
                );
                let base_view = upstream.create_view(&Default::default());
                let local_view = local_src.create_view(&Default::default());
                let mview = mtex.create_view(&Default::default());
                let dview = dst.create_view(&Default::default());
                let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("blend-bind"),
                    layout: &self.blend.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&base_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&local_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&mview),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(&dview),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: ub.as_entire_binding(),
                        },
                    ],
                });
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("blend"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.blend.pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(out_w.div_ceil(16), out_h.div_ceil(16), 1);
                self.last_passes_run.push("blend".into());
                upstream = dst;
                final_tag = FinalTag::Comp(comp_flip ^ 1);
            }
        }
        self.last_final = final_tag;

        // ---- present (terminal) ----
        let out_tex = self.out_tex.as_ref().unwrap();
        {
            let overlay_view = overlay_mask
                .and_then(|id| self.mask_tex.get(id))
                .unwrap_or(&self.dummy_mask)
                .create_view(&Default::default());
            let u = PresentUniforms {
                width: out_w,
                height: out_h,
                overlay: if overlay_mask.is_some() { 0.55 } else { 0.0 },
                look: self.look,
            };
            gpu.queue
                .write_buffer(&self.present_uniforms, 0, bytemuck::bytes_of(&u));
            let upstream_view = upstream.create_view(&Default::default());
            let out_view = out_tex.create_view(&Default::default());
            let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("present-bind"),
                layout: &self.present.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&upstream_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&out_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.present_uniforms.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&overlay_view),
                    },
                ],
            });
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("present"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.present.pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(out_w.div_ceil(16), out_h.div_ceil(16), 1);
            self.last_passes_run.push("present".into());
        }

        gpu.queue.submit([encoder.finish()]);
        self.last_view_key = Some(view_key);
        self.dirty_from = usize::MAX;
        self.readback(gpu, out_w, out_h)
    }

    fn pipe_for(&self, kind: PipeKind) -> &PassResources {
        match kind {
            PipeKind::Matrix => &self.simple_pipes["matrix"],
            PipeKind::Calibration => &self.simple_pipes["calibration"],
            PipeKind::Noise => &self.simple_pipes["noise"],
            PipeKind::Grade => &self.simple_pipes["grade"],
            PipeKind::Hsl => &self.simple_pipes["hsl"],
            PipeKind::Sharpen => &self.simple_pipes["sharpen"],
            PipeKind::Curve => &self.curve_pipe,
        }
    }

    fn readback(&self, gpu: &GpuContext, out_w: u32, out_h: u32) -> Result<Vec<u8>, CoreError> {
        let out_tex = self
            .out_tex
            .as_ref()
            .ok_or_else(|| CoreError::Gpu("no output texture".into()))?;
        let bytes_per_row_packed = out_w * 4;
        let bytes_per_row = bytes_per_row_packed.div_ceil(256) * 256;
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("graph-readback"),
            size: (bytes_per_row * out_h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback-encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: out_tex,
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
        let mut out = vec![0u8; (bytes_per_row_packed * out_h) as usize];
        for row in 0..out_h {
            let s = (row * bytes_per_row) as usize;
            let d = (row * bytes_per_row_packed) as usize;
            out[d..d + bytes_per_row_packed as usize]
                .copy_from_slice(&data[s..s + bytes_per_row_packed as usize]);
        }
        drop(data);
        readback.unmap();
        Ok(out)
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch_node(
    gpu: &GpuContext,
    encoder: &mut wgpu::CommandEncoder,
    pipe: &PassResources,
    is_curve: bool,
    input: &wgpu::Texture,
    output: &wgpu::Texture,
    uniforms: &wgpu::Buffer,
    lut: &wgpu::Buffer,
    label: &str,
    out_w: u32,
    out_h: u32,
) {
    let in_view = input.create_view(&Default::default());
    let out_view = output.create_view(&Default::default());
    let mut entries = vec![
        wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&in_view),
        },
        wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::TextureView(&out_view),
        },
        wgpu::BindGroupEntry {
            binding: 2,
            resource: uniforms.as_entire_binding(),
        },
    ];
    if is_curve {
        entries.push(wgpu::BindGroupEntry {
            binding: 3,
            resource: lut.as_entire_binding(),
        });
    }
    let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: &pipe.layout,
        entries: &entries,
    });
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some(label),
        timestamp_writes: None,
    });
    pass.set_pipeline(&pipe.pipeline);
    pass.set_bind_group(0, &bind, &[]);
    pass.dispatch_workgroups(out_w.div_ceil(16), out_h.div_ceil(16), 1);
}

/// Parse geometry source → (shader kind, pa, pb, rotation, stroke points).
fn parse_geometry(
    source: &serde_json::Value,
) -> (u32, [f32; 2], [f32; 2], f32, Vec<[f32; 4]>) {
    let get2 = |key: &str, default: [f32; 2]| -> [f32; 2] {
        source
            .get(key)
            .and_then(|v| v.as_array())
            .and_then(|a| {
                Some([
                    a.first()?.as_f64()? as f32,
                    a.get(1)?.as_f64()? as f32,
                ])
            })
            .unwrap_or(default)
    };
    match source.get("type").and_then(|t| t.as_str()) {
        Some("radial") => {
            let center = get2("center", [0.5, 0.5]);
            let radii = get2("radii", [0.25, 0.25]);
            let rotation = source
                .get("rotation")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32;
            (0, center, radii, rotation, vec![])
        }
        Some("linear") => {
            let start = get2("start", [0.5, 0.0]);
            let end = get2("end", [0.5, 1.0]);
            (1, start, end, 0.0, vec![])
        }
        Some("brush") => {
            let mut pts = Vec::new();
            if let Some(strokes) = source.get("strokes").and_then(|s| s.as_array()) {
                for stroke in strokes {
                    let radius = stroke
                        .get("radius")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.05) as f32;
                    let hardness = stroke
                        .get("hardness")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.5) as f32;
                    let sub = stroke.get("mode").and_then(|m| m.as_str()) == Some("subtract");
                    let r = if sub { -radius } else { radius };
                    if let Some(points) = stroke.get("points").and_then(|p| p.as_array()) {
                        for p in points {
                            if let Some(a) = p.as_array() {
                                if let (Some(x), Some(y)) =
                                    (a.first().and_then(|v| v.as_f64()), a.get(1).and_then(|v| v.as_f64()))
                                {
                                    if pts.len() < MAX_STROKE_POINTS {
                                        pts.push([x as f32, y as f32, r, hardness]);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            (2, [0.0; 2], [0.0; 2], 0.0, pts)
        }
        _ => (0, [0.5, 0.5], [0.0, 0.0], 0.0, vec![]),
    }
}
