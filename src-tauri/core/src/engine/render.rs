use super::Engine;
use super::*;

impl Engine {
    pub(super) fn compute_stats(&self) -> Option<crate::message::FrameStats> {
        const BINS: usize = 64;
        let frame = self.latest_frame.as_ref()?;
        let mut r = vec![0u32; BINS];
        let mut g = vec![0u32; BINS];
        let mut b = vec![0u32; BINS];
        let mut luma = vec![0u32; BINS];
        let (mut hi, mut lo, mut n) = (0u64, 0u64, 0u64);
        for px in frame.rgba.chunks_exact(4) {
            if px[0] == 22 && px[1] == 22 && px[2] == 24 {
                continue; // letterbox bg
            }
            n += 1;
            r[px[0] as usize * BINS / 256] += 1;
            g[px[1] as usize * BINS / 256] += 1;
            b[px[2] as usize * BINS / 256] += 1;
            let l =
                (0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32) as usize;
            luma[(l * BINS / 256).min(BINS - 1)] += 1;
            if px[0] >= 254 || px[1] >= 254 || px[2] >= 254 {
                hi += 1;
            }
            if px[0] <= 1 && px[1] <= 1 && px[2] <= 1 {
                lo += 1;
            }
        }
        let n = n.max(1) as f32;
        Some(crate::message::FrameStats {
            bins: BINS,
            r,
            g,
            b,
            luma,
            clip_high_pct: 100.0 * hi as f32 / n,
            clip_low_pct: 100.0 * lo as f32 / n,
        })
    }
    pub(super) fn wb_from_point(&mut self, x: f32, y: f32) -> Result<DocDelta, CoreError> {
        let c = self.current.as_mut().ok_or(CoreError::NoImage)?;
        let small = c.small_cpu.as_ref().ok_or(CoreError::NoImage)?;
        let (w, h) = (small.width as i64, small.height as i64);
        let cx = ((x.clamp(0.0, 1.0) * w as f32) as i64).clamp(0, w - 1);
        let cy = ((y.clamp(0.0, 1.0) * h as f32) as i64).clamp(0, h - 1);
        let mut acc = [0.0f64; 3];
        let mut count = 0.0f64;
        for dy in -2..=2i64 {
            for dx in -2..=2i64 {
                let px = (cx + dx).clamp(0, w - 1);
                let py = (cy + dy).clamp(0, h - 1);
                let i = ((py * w + px) * 3) as usize;
                acc[0] += small.data[i] as f64;
                acc[1] += small.data[i + 1] as f64;
                acc[2] += small.data[i + 2] as f64;
                count += 1.0;
            }
        }
        let sample = [
            (acc[0] / count) as f32,
            (acc[1] / count) as f32,
            (acc[2] / count) as f32,
        ];
        if sample[1] <= 1e-5 {
            return Err(CoreError::InvalidOp("sampled patch too dark".into()));
        }
        let (temp, tint) = crate::color::solve_wb_for_neutral(sample, c.as_shot_cct());
        tracing::info!(temp, tint, ?sample, "wb eyedropper");

        let before = c.doc().clone();
        ops::apply_op(
            c.doc_mut(),
            &Op::SetParam {
                path: "white_balance.temp".into(),
                value: serde_json::json!(temp),
            },
        )?;
        ops::apply_op(
            c.doc_mut(),
            &Op::SetParam {
                path: "white_balance.tint".into(),
                value: serde_json::json!(tint),
            },
        )?;
        c.history.record(before, "wb eyedropper".into());
        c.doc_dirty = true;
        let delta = c.delta("wb eyedropper".into(), None);
        if let Some(g) = &mut self.graph {
            g.invalidate_from_module("white_balance");
        }
        self.schedule_render();
        self.schedule_settle();
        Ok(delta)
    }
    pub(super) fn full_redraw(&mut self) {
        if let Some(g) = &mut self.graph {
            g.invalidate_all();
        }
        self.schedule_render();
    }

    pub(super) fn on_settle(&mut self) {
        self.flush_sidecar_now();
    }
    pub(super) fn render_view(&mut self, view: ViewParams) -> Result<FrameInfo, CoreError> {
        self.ensure_segmentations();
        let Some(cur) = &self.current else {
            return Err(CoreError::NoImage);
        };
        let Some((_, tex_view, w, h)) = &cur.working else {
            return self
                .latest_frame
                .as_ref()
                .map(|f| FrameInfo {
                    version: f.version,
                    width: f.width,
                    height: f.height,
                })
                .ok_or(CoreError::NoImage);
        };
        let gpu = self.gpu.as_ref().ok_or(CoreError::Gpu("no gpu".into()))?;
        if self.graph.is_none() {
            self.graph = Some(RenderGraph::new(gpu));
        }
        // Rendered images (JPEG/PNG/…) are display-referred — force the
        // passthrough look (3) so they aren't re-tonemapped by the RAW view.
        let display_look = match self.current.as_ref() {
            Some(c) if c.meta.kind == crate::raw::ImageKind::Rendered => 3,
            _ => self.display_look,
        };
        self.graph.as_mut().unwrap().set_look(display_look);
        let (w, h) = (*w, *h);
        let seg_views: std::collections::HashMap<String, wgpu::TextureView> = cur
            .masks_gpu
            .iter()
            .map(|(id, (_, tex))| (id.clone(), tex.create_view(&Default::default())))
            .collect();
        // before/after: render the all-defaults base when bypass is on
        let base_doc;
        let render_doc = if self.preview_bypass {
            base_doc = EditDoc::new(&cur.path.to_string_lossy());
            &base_doc
        } else {
            cur.doc()
        };
        let as_shot_cct = cur.as_shot_cct();
        let dcp = cur.dcp_profile.clone();
        let started = Instant::now();
        let rgba = {
            let graph = self.graph.as_mut().unwrap();
            graph.render(
                gpu,
                tex_view,
                w,
                h,
                &view,
                render_doc,
                as_shot_cct,
                &seg_views,
                self.overlay_mask.as_deref(),
                dcp.as_deref(),
            )?
        };
        let elapsed = started.elapsed();
        if let Some(g) = &self.graph {
            tracing::debug!(
                ms = elapsed.as_millis() as u64,
                passes = ?g.last_passes_run,
                "render"
            );
        }
        self.last_view = Some(view);
        // perf instrumentation (spec 7.5)
        self.perf.last_render_ms = elapsed.as_millis() as u64;
        self.perf.renders += 1;
        if let Some(g) = &self.graph {
            self.perf.last_passes = g.last_passes_run.clone();
            // no extract pass ⇒ upstream caches were reused this render
            if !g.last_passes_run.iter().any(|p| p == "extract") {
                self.perf.cached_renders += 1;
            }
        }
        let version = self.next_version();
        let frame = Frame {
            width: view.out_w,
            height: view.out_h,
            rgba,
            version,
        };
        let info = FrameInfo {
            version,
            width: frame.width,
            height: frame.height,
        };
        self.latest_frame = Some(frame);
        Ok(info)
    }
    pub(super) fn render_preview_jpeg(&mut self, max_dim: u32) -> Result<Vec<u8>, CoreError> {
        self.ensure_segmentations();
        let Some(cur) = &self.current else {
            return Err(CoreError::NoImage);
        };
        let Some((_, tex_view, w, h)) = &cur.working else {
            return Err(CoreError::NoImage);
        };
        let gpu = self.gpu.as_ref().ok_or(CoreError::Gpu("no gpu".into()))?;
        if self.preview_graph.is_none() {
            self.preview_graph = Some(RenderGraph::new(gpu));
        }
        let (w, h) = (*w, *h);
        let max_dim = max_dim.clamp(256, 1600);
        let scale = (max_dim as f32 / w.max(h) as f32).min(1.0);
        let view = ViewParams {
            out_w: ((w as f32 * scale) as u32).max(1),
            out_h: ((h as f32 * scale) as u32).max(1),
            scale: Some(scale),
            center_x: 0.5,
            center_y: 0.5,
            crop_preview: false,
        };
        let seg_views: std::collections::HashMap<String, wgpu::TextureView> = cur
            .masks_gpu
            .iter()
            .map(|(id, (_, tex))| (id.clone(), tex.create_view(&Default::default())))
            .collect();
        let graph = self.preview_graph.as_mut().unwrap();
        graph.invalidate_all();
        let rgba = graph.render(
            gpu,
            tex_view,
            w,
            h,
            &view,
            cur.doc(),
            cur.as_shot_cct(),
            &seg_views,
            None,
            cur.dcp_profile.as_deref(),
        )?;
        let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        let img = image::RgbImage::from_raw(view.out_w, view.out_h, rgb)
            .ok_or_else(|| CoreError::Engine("preview buffer".into()))?;
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82)
            .encode_image(&img)
            .map_err(|e| CoreError::Engine(e.to_string()))?;
        Ok(out)
    }
    pub(super) fn sample_color(&self, x: f32, y: f32) -> Result<crate::message::SampledColor, CoreError> {
        let cur = self.current.as_ref().ok_or(CoreError::NoImage)?;
        let small = cur.small_cpu.as_ref().ok_or(CoreError::NoImage)?;
        let (w, h) = (small.width, small.height);
        let cx = ((x.clamp(0.0, 1.0) * w as f32) as usize).min(w - 1);
        let cy = ((y.clamp(0.0, 1.0) * h as f32) as usize).min(h - 1);
        let i = (cy * w + cx) * 3;
        let lin = [small.data[i], small.data[i + 1], small.data[i + 2]];
        // friendly display value (same math as the present shader)
        let m = crate::color::mat_mul(
            &crate::color::XYZ_TO_SRGB,
            &crate::color::REC2020_TO_XYZ,
        );
        let srgb_lin = crate::color::mat_vec(&m, lin);
        let l = 0.2126 * srgb_lin[0].max(0.0)
            + 0.7152 * srgb_lin[1].max(0.0)
            + 0.0722 * srgb_lin[2].max(0.0);
        let k = if l > 1e-8 {
            (l * (1.0 + l / 16.0) / (1.0 + l)) / l
        } else {
            0.0
        };
        let display = srgb_lin.map(|c| {
            let c = (c.max(0.0) * k).clamp(0.0, 1.0);
            let enc = if c <= 0.0031308 {
                12.92 * c
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            };
            (enc * 255.0).round() as u8
        });
        Ok(crate::message::SampledColor {
            working: lin,
            display,
        })
    }
    pub(super) fn render_now(&mut self) {
        let Some(view) = self.last_view else { return };
        match self.render_view(view) {
            Ok(info) => self.emit(EngineEvent::FrameReady {
                version: info.version,
            }),
            Err(CoreError::NoImage) => {}
            Err(e) => tracing::error!(error = %e, "debounced render failed"),
        }
    }
}
