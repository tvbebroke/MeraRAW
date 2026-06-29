use super::Engine;
use super::*;
use std::sync::Arc;

const TILE: u32 = 1024;

pub(super) struct ExportJob {
    settings: crate::export::ExportSettings,
    reply: Option<oneshot::Sender<Result<String, CoreError>>>,
    full: Vec<f32>,
    w: u32,
    h: u32,
    tile_tx: u32,
    tile_ty: u32,
    tiles_done: u32,
    tiles_total: u32,
    source_path: String,
    look: u32,
    meta: ImageMeta,
    dcp: Option<Arc<DcpProfile>>,
    cct: f32,
}

impl Engine {
    pub(super) fn export_image(
        &mut self,
        settings: crate::export::ExportSettings,
        reply: oneshot::Sender<Result<String, CoreError>>,
    ) {
        if self.export_job.is_some() {
            let _ = reply.send(Err(CoreError::Engine("export already in progress".into())));
            return;
        }
        self.flush_sidecar_now();

        let setup: Result<ExportJob, CoreError> = (|| {
            self.ensure_segmentations();
            let cur = self.current.as_ref().ok_or(CoreError::NoImage)?;
            let source_path = cur.path.to_string_lossy().into_owned();
            let (_, _, w, h) = *cur
                .working
                .as_ref()
                .ok_or(CoreError::Engine("decode not finished".into()))?;
            let gpu = self.gpu.as_ref().ok_or(CoreError::Gpu("no gpu".into()))?;
            if self.export_graph.is_none() {
                self.export_graph = Some(RenderGraph::new(gpu));
            }
            let tiles_x = w.div_ceil(TILE);
            let tiles_y = h.div_ceil(TILE);
            let dcp = cur.dcp_profile.clone();
            let display_look = self.display_look;
            if let Some(g) = self.export_graph.as_mut() {
                g.set_look(display_look);
            }
            Ok(ExportJob {
                settings,
                reply: None,
                full: vec![0.0f32; (w as usize) * (h as usize) * 3],
                w,
                h,
                tile_tx: 0,
                tile_ty: 0,
                tiles_done: 0,
                tiles_total: tiles_x * tiles_y,
                source_path,
                look: display_look,
                meta: cur.meta.clone(),
                dcp,
                cct: cur.as_shot_cct(),
            })
        })();

        match setup {
            Err(e) => {
                let _ = reply.send(Err(e));
            }
            Ok(mut job) => {
                job.reply = Some(reply);
                let total = job.tiles_total;
                self.export_job = Some(job);
                self.emit(EngineEvent::ExportProgress {
                    phase: "render".into(),
                    done: 0,
                    total,
                });
                let _ = self.self_tx.try_send(EngineMsg::ExportStep);
            }
        }
    }

    pub(super) fn export_step(&mut self) {
        let Some(job) = self.export_job.as_ref() else {
            return;
        };
        let tx = job.tile_tx;
        let ty = job.tile_ty;
        let job_w = job.w;
        let job_h = job.h;
        let cct = job.cct;
        let dcp = job.dcp.clone();

        let tile_result: Result<Vec<f32>, CoreError> = (|| {
            let gpu = self.gpu.as_ref().ok_or(CoreError::Gpu("no gpu".into()))?;
            let cur = self.current.as_ref().ok_or(CoreError::NoImage)?;
            let (_, tex_view, w, h) = cur
                .working
                .as_ref()
                .ok_or(CoreError::Engine("decode not finished".into()))?;
            let seg_views: std::collections::HashMap<String, wgpu::TextureView> = cur
                .masks_gpu
                .iter()
                .map(|(id, (_, tex))| (id.clone(), tex.create_view(&Default::default())))
                .collect();
            let graph = self
                .export_graph
                .as_mut()
                .ok_or(CoreError::Engine("export graph".into()))?;

            let tw = TILE.min(*w - tx);
            let th = TILE.min(*h - ty);
            let view = ViewParams {
                out_w: tw,
                out_h: th,
                scale: Some(1.0),
                center_x: (tx as f32 + tw as f32 / 2.0) / *w as f32,
                center_y: (ty as f32 + th as f32 / 2.0) / *h as f32,
            };
            let mut tile = graph.render_linear_tile(
                gpu,
                tex_view,
                *w,
                *h,
                &view,
                cur.doc(),
                cct,
                &seg_views,
            )?;
            if let Some(dcp) = dcp.as_ref() {
                for px in tile.chunks_mut(3) {
                    let out = dcp.apply_look([px[0], px[1], px[2]], cct);
                    px.copy_from_slice(&out);
                }
            }
            Ok(tile)
        })();

        let tile = match tile_result {
            Ok(t) => t,
            Err(e) => {
                if let Some(job) = self.export_job.take() {
                    if let Some(reply) = job.reply {
                        let _ = reply.send(Err(e));
                    }
                }
                return;
            }
        };

        {
            let job = self.export_job.as_mut().unwrap();
            let tw = TILE.min(job_w - tx);
            let th = TILE.min(job_h - ty);
            for row in 0..th as usize {
                let src = row * tw as usize * 3;
                let dst = ((ty as usize + row) * job.w as usize + tx as usize) * 3;
                job.full[dst..dst + tw as usize * 3]
                    .copy_from_slice(&tile[src..src + tw as usize * 3]);
            }
            drop(tile);
            job.tiles_done += 1;
            job.tile_tx += TILE;
            if job.tile_tx >= job.w {
                job.tile_tx = 0;
                job.tile_ty += TILE;
            }
        }

        let (done, total, more_tiles) = {
            let j = self.export_job.as_ref().unwrap();
            (j.tiles_done, j.tiles_total, j.tile_ty < j.h)
        };

        self.emit(EngineEvent::ExportProgress {
            phase: "render".into(),
            done,
            total,
        });

        if more_tiles {
            let _ = self.self_tx.try_send(EngineMsg::ExportStep);
            return;
        }

        let mut job = self.export_job.take().unwrap();
        let look = job.look;
        tracing::info!(
            w = job.w,
            h = job.h,
            tiles = job.tiles_total,
            profile = job
                .dcp
                .as_ref()
                .map(|d| d.profile_name.as_str())
                .unwrap_or("none"),
            "export render (tiled) done"
        );

        self.emit(EngineEvent::ExportProgress {
            phase: "encode".into(),
            done: 0,
            total: 1,
        });

        let settings = job.settings;
        let full = job.full;
        let w = job.w;
        let h = job.h;
        let source_path = job.source_path;
        let meta = job.meta;
        let reply = job.reply.take().expect("export reply");
        let events = self.events.clone();

        std::thread::Builder::new()
            .name("export-worker".into())
            .spawn(move || {
                let t0 = Instant::now();
                let result = (|| {
                    let (lin, w, h) = match settings.max_dim {
                        Some(d) => crate::export::resize_linear(full, w, h, d),
                        None => (full, w, h),
                    };
                    let want16 = settings.format == crate::export::ExportFormat::Tiff16;
                    let mut enc = crate::export::output_transform_look(
                        &lin,
                        w,
                        h,
                        settings.target,
                        want16,
                        look,
                    );
                    if !want16 {
                        crate::export::output_sharpen8(&mut enc.rgb8, w, h, settings.sharpen);
                    }
                    crate::export::encode_and_write(&enc, &settings, &source_path, &meta)
                        .map(|p| p.to_string_lossy().into_owned())
                })();
                tracing::info!(
                    ms = t0.elapsed().as_millis() as u64,
                    ok = result.is_ok(),
                    "export encode done"
                );
                if let Some(tx) = &events {
                    let _ = tx.send(EngineEvent::ExportProgress {
                        phase: "encode".into(),
                        done: 1,
                        total: 1,
                    });
                }
                let _ = reply.send(result);
            })
            .expect("spawn export worker");
    }
}
