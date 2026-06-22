//! Engine actor: dedicated thread + current-thread tokio runtime owning all
//! heavy state (GPU, images, the canonical EditDoc). Single owner. Commands
//! talk via mpsc + oneshot; decode runs on worker threads posting results
//! back as internal messages; renders are debounced (op storms coalesce —
//! latest doc wins) and sidecar writes happen on settle.

use crate::doc::EditDoc;
use crate::error::CoreError;
use crate::gpu::display::{upload_working_texture, ViewParams};
use crate::gpu::GpuContext;
use crate::graph::RenderGraph;
use crate::image::RgbF32Buf;
use crate::message::{
    DecodedPayload, EngineEvent, EngineInfo, EngineMsg, EngineStatus, Frame, FrameInfo,
};
use crate::ops::{self, DocDelta, History, Op};
use crate::raw::{Decoder, ImageMeta, RawlerDecoder};
use crate::sidecar;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

const RENDER_DEBOUNCE: Duration = Duration::from_millis(8);
const SETTLE_DEBOUNCE: Duration = Duration::from_millis(600);

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("engine channel closed")]
    ChannelClosed,
    #[error("engine dropped reply")]
    ReplyDropped,
}

#[derive(Clone)]
pub struct EngineHandle {
    tx: mpsc::Sender<EngineMsg>,
}

impl EngineHandle {
    async fn request<T>(
        &self,
        build: impl FnOnce(oneshot::Sender<T>) -> EngineMsg,
    ) -> Result<T, EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(build(tx))
            .await
            .map_err(|_| EngineError::ChannelClosed)?;
        rx.await.map_err(|_| EngineError::ReplyDropped)
    }

    pub async fn ping(&self) -> Result<EngineStatus, EngineError> {
        self.request(|reply| EngineMsg::Ping { reply }).await
    }

    pub async fn info(&self) -> Result<EngineInfo, EngineError> {
        self.request(|reply| EngineMsg::Info { reply }).await
    }

    pub async fn test_frame(&self, width: u32, height: u32) -> Result<Frame, EngineError> {
        self.request(|reply| EngineMsg::TestFrame {
            width,
            height,
            reply,
        })
        .await
    }

    pub async fn open_image(
        &self,
        path: PathBuf,
    ) -> Result<Result<ImageMeta, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::OpenImage { path, reply })
            .await
    }

    pub async fn request_frame(
        &self,
        view: ViewParams,
    ) -> Result<Result<FrameInfo, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::RequestFrame { view, reply })
            .await
    }

    pub async fn get_frame(&self) -> Result<Option<Frame>, EngineError> {
        self.request(|reply| EngineMsg::GetFrame { reply }).await
    }

    pub async fn get_metadata(&self) -> Result<Option<ImageMeta>, EngineError> {
        self.request(|reply| EngineMsg::GetMetadata { reply }).await
    }

    pub async fn close_image(&self) -> Result<(), EngineError> {
        self.request(|reply| EngineMsg::CloseImage { reply }).await
    }

    pub async fn apply_op(&self, op: Op) -> Result<Result<DocDelta, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::ApplyOp { op, reply }).await
    }

    pub async fn undo(&self) -> Result<Result<DocDelta, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::Undo { reply }).await
    }

    pub async fn redo(&self) -> Result<Result<DocDelta, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::Redo { reply }).await
    }

    pub async fn get_doc(&self) -> Result<Option<serde_json::Value>, EngineError> {
        self.request(|reply| EngineMsg::GetDoc { reply }).await
    }

    pub async fn get_history(&self) -> Result<Vec<String>, EngineError> {
        self.request(|reply| EngineMsg::GetHistory { reply }).await
    }

    pub async fn snapshot(&self, name: String) -> Result<Result<(), CoreError>, EngineError> {
        self.request(|reply| EngineMsg::Snapshot { name, reply })
            .await
    }

    pub async fn list_snapshots(&self) -> Result<Vec<String>, EngineError> {
        self.request(|reply| EngineMsg::ListSnapshots { reply })
            .await
    }

    pub async fn restore_snapshot(
        &self,
        name: String,
    ) -> Result<Result<DocDelta, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::RestoreSnapshot { name, reply })
            .await
    }

    pub async fn virtual_copy(&self) -> Result<Result<String, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::VirtualCopy { reply }).await
    }

    pub async fn switch_doc(
        &self,
        doc_id: String,
    ) -> Result<Result<DocDelta, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::SwitchDoc { doc_id, reply })
            .await
    }

    pub async fn save_preset(
        &self,
        modules: Vec<String>,
    ) -> Result<Result<serde_json::Value, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::SavePreset { modules, reply })
            .await
    }

    pub async fn flush_sidecar(&self) -> Result<Result<(), CoreError>, EngineError> {
        self.request(|reply| EngineMsg::FlushSidecar { reply })
            .await
    }

    pub async fn get_stats(
        &self,
    ) -> Result<Option<crate::message::FrameStats>, EngineError> {
        self.request(|reply| EngineMsg::GetStats { reply }).await
    }

    pub async fn wb_from_point(
        &self,
        x: f32,
        y: f32,
    ) -> Result<Result<DocDelta, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::WbFromPoint { x, y, reply })
            .await
    }

    pub async fn set_mask_overlay(&self, id: Option<String>) -> Result<(), EngineError> {
        self.request(|reply| EngineMsg::SetMaskOverlay { id, reply })
            .await
    }

    pub async fn set_preview_bypass(&self, on: bool) -> Result<(), EngineError> {
        self.request(|reply| EngineMsg::SetPreviewBypass { on, reply })
            .await
    }

    // ---- Phase 5: catalog ----

    pub async fn import_folder(
        &self,
        path: PathBuf,
    ) -> Result<Result<u64, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::ImportFolder { path, reply })
            .await
    }

    pub async fn get_grid(
        &self,
        query: crate::catalog::GridQuery,
    ) -> Result<Result<Vec<crate::catalog::GridItem>, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::GetGrid { query, reply })
            .await
    }

    pub async fn set_asset_meta(
        &self,
        ids: Vec<i64>,
        patch: crate::catalog::MetaPatch,
    ) -> Result<Result<(), CoreError>, EngineError> {
        self.request(|reply| EngineMsg::SetAssetMeta { ids, patch, reply })
            .await
    }

    pub async fn rebuild_index(&self) -> Result<Result<u64, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::RebuildIndex { reply })
            .await
    }

    pub async fn get_preview_file(
        &self,
        id: i64,
        tier: String,
    ) -> Result<Option<PathBuf>, EngineError> {
        self.request(|reply| EngineMsg::GetPreviewFile { id, tier, reply })
            .await
    }

    // ---- Phase 6: assistant eyes ----

    pub async fn render_preview_jpeg(
        &self,
        max_dim: u32,
    ) -> Result<Result<Vec<u8>, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::RenderPreviewJpeg { max_dim, reply })
            .await
    }

    pub async fn sample_color(
        &self,
        x: f32,
        y: f32,
    ) -> Result<Result<crate::message::SampledColor, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::SampleColor { x, y, reply })
            .await
    }

    // ---- Phase 7 ----

    pub async fn export_image(
        &self,
        settings: crate::export::ExportSettings,
    ) -> Result<Result<String, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::ExportImage { settings, reply })
            .await
    }

    pub async fn save_preset_to_disk(
        &self,
        name: String,
        modules: Vec<String>,
    ) -> Result<Result<(), CoreError>, EngineError> {
        self.request(|reply| EngineMsg::SavePresetToDisk {
            name,
            modules,
            reply,
        })
        .await
    }

    pub async fn list_presets(&self) -> Result<Vec<String>, EngineError> {
        self.request(|reply| EngineMsg::ListPresets { reply }).await
    }

    pub async fn apply_preset_by_name(
        &self,
        name: String,
    ) -> Result<Result<DocDelta, CoreError>, EngineError> {
        self.request(|reply| EngineMsg::ApplyPresetByName { name, reply })
            .await
    }

    pub async fn get_perf_stats(&self) -> Result<crate::message::PerfStats, EngineError> {
        self.request(|reply| EngineMsg::GetPerfStats { reply }).await
    }
}

pub fn spawn_with_events(events: Option<mpsc::UnboundedSender<EngineEvent>>) -> EngineHandle {
    let (tx, rx) = mpsc::channel::<EngineMsg>(256);
    let self_tx = tx.clone();
    std::thread::Builder::new()
        .name("meratech-engine".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("engine runtime");
            rt.block_on(run(rx, self_tx, events));
        })
        .expect("spawn engine thread");
    EngineHandle { tx }
}

pub fn spawn() -> EngineHandle {
    spawn_with_events(None)
}

struct CurrentImage {
    path: PathBuf,
    meta: ImageMeta,
    /// (texture, view, w, h) — the working master (contract A4).
    working: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
    /// Retained downsized CPU copy (histogram / segmentation / fallback).
    small_cpu: Option<std::sync::Arc<RgbF32Buf>>,
    /// All docs for this image: [0] = primary, rest = virtual copies.
    docs: Vec<EditDoc>,
    active_doc: usize,
    history: History,
    snapshots: Vec<(String, EditDoc)>,
    doc_dirty: bool, // unsaved sidecar changes (primary doc only)
    /// Segmentation cache: mask id → (source hash, uploaded small mask).
    masks_gpu: std::collections::HashMap<String, (u64, wgpu::Texture)>,
    pending_segments: std::collections::HashSet<String>,
}

impl CurrentImage {
    fn doc(&self) -> &EditDoc {
        &self.docs[self.active_doc]
    }
    fn doc_mut(&mut self) -> &mut EditDoc {
        &mut self.docs[self.active_doc]
    }
    fn as_shot_cct(&self) -> f32 {
        self.meta.estimated_cct.unwrap_or(5200.0)
    }
    fn delta(&self, label: String, new_mask_id: Option<String>) -> DocDelta {
        let (u, r) = self.history.depths();
        DocDelta {
            doc: self.doc().to_json(),
            label,
            undo_depth: u,
            redo_depth: r,
            new_mask_id,
        }
    }
}

struct Engine {
    gpu: Option<GpuContext>,
    graph: Option<RenderGraph>,
    decoder: RawlerDecoder,
    events: Option<mpsc::UnboundedSender<EngineEvent>>,
    self_tx: mpsc::Sender<EngineMsg>,
    current: Option<CurrentImage>,
    latest_frame: Option<Frame>,
    frame_version: u64,
    generation: u64,
    last_view: Option<ViewParams>,
    render_at: Option<Instant>,
    settle_at: Option<Instant>,
    overlay_mask: Option<String>,
    catalog: Option<crate::catalog::Catalog>,
    import_state: Option<ImportState>,
    /// Before/after: when true, render the un-edited base.
    preview_bypass: bool,
    /// Dedicated graph for assistant previews (own small caches — never
    /// thrashes the viewport graph).
    preview_graph: Option<RenderGraph>,
    /// Dedicated graph for tiled export (tile-sized caches).
    export_graph: Option<RenderGraph>,
    perf: crate::message::PerfStats,
}

struct ImportState {
    id: u64,
    total: u64,
    done: u64,
    /// further roots to import once this one finishes (rebuild path)
    queued_roots: Vec<PathBuf>,
}

async fn run(
    mut rx: mpsc::Receiver<EngineMsg>,
    self_tx: mpsc::Sender<EngineMsg>,
    events: Option<mpsc::UnboundedSender<EngineEvent>>,
) {
    let gpu = match GpuContext::init().await {
        Ok(g) => {
            tracing::info!(adapter = %g.adapter_name(), backend = %g.backend_name(), "gpu ready");
            Some(g)
        }
        Err(e) => {
            tracing::error!(error = %e, "gpu init failed; engine continues without gpu");
            None
        }
    };
    let mut engine = Engine {
        gpu,
        graph: None,
        decoder: RawlerDecoder::default(),
        events,
        self_tx,
        current: None,
        latest_frame: None,
        frame_version: 0,
        generation: 0,
        last_view: None,
        render_at: None,
        settle_at: None,
        overlay_mask: None,
        catalog: None,
        import_state: None,
        preview_bypass: false,
        preview_graph: None,
        export_graph: None,
        perf: Default::default(),
    };
    tracing::info!("engine actor up");

    loop {
        let next_deadline = [engine.render_at, engine.settle_at]
            .into_iter()
            .flatten()
            .min();
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Some(m) => engine.handle(m),
                    None => break,
                }
            }
            _ = deadline_sleep(next_deadline) => {
                let now = Instant::now();
                if engine.render_at.is_some_and(|t| t <= now) {
                    engine.render_at = None;
                    engine.render_now();
                }
                if engine.settle_at.is_some_and(|t| t <= now) {
                    engine.settle_at = None;
                    engine.on_settle();
                }
            }
        }
    }
    tracing::info!("engine actor shut down");
}

async fn deadline_sleep(deadline: Option<Instant>) {
    match deadline {
        Some(t) => tokio::time::sleep_until(tokio::time::Instant::from_std(t)).await,
        None => std::future::pending().await,
    }
}

impl Engine {
    fn emit(&self, ev: EngineEvent) {
        if let Some(tx) = &self.events {
            let _ = tx.send(ev);
        }
    }

    fn next_version(&mut self) -> u64 {
        self.frame_version += 1;
        self.frame_version
    }

    fn schedule_render(&mut self) {
        self.render_at = Some(Instant::now() + RENDER_DEBOUNCE);
    }

    fn schedule_settle(&mut self) {
        self.settle_at = Some(Instant::now() + SETTLE_DEBOUNCE);
    }

    fn handle(&mut self, msg: EngineMsg) {
        match msg {
            EngineMsg::Ping { reply } => {
                let _ = reply.send(EngineStatus {
                    alive: true,
                    gpu_ready: self.gpu.is_some(),
                    adapter: self.gpu.as_ref().map(|g| g.adapter_name()),
                });
            }
            EngineMsg::Info { reply } => {
                let _ = reply.send(EngineInfo {
                    gpu_adapter: self.gpu.as_ref().map(|g| g.adapter_name()),
                    gpu_backend: self.gpu.as_ref().map(|g| g.backend_name()),
                });
            }
            EngineMsg::TestFrame {
                width,
                height,
                reply,
            } => {
                let v = self.frame_version;
                let _ = reply.send(render_test_frame(width, height, v));
            }
            EngineMsg::OpenImage { path, reply } => self.open_image(path, reply),
            EngineMsg::PreviewDone {
                generation,
                rgba,
                width,
                height,
            } => {
                if generation != self.generation {
                    return;
                }
                if self
                    .current
                    .as_ref()
                    .map(|c| c.working.is_some())
                    .unwrap_or(true)
                {
                    return;
                }
                let version = self.next_version();
                self.latest_frame = Some(Frame {
                    width,
                    height,
                    rgba,
                    version,
                });
                tracing::info!(version, width, height, "preview frame ready");
                self.emit(EngineEvent::PreviewReady { version });
                self.emit(EngineEvent::FrameReady { version });
            }
            EngineMsg::DecodeDone { generation, result } => {
                if generation != self.generation {
                    return;
                }
                match result {
                    Ok(payload) => self.finish_decode(*payload),
                    Err(e) => {
                        tracing::error!(error = %e, "decode failed");
                        self.emit(EngineEvent::DecodeError {
                            message: e.to_string(),
                        });
                    }
                }
            }
            EngineMsg::RequestFrame { view, reply } => {
                self.last_view = Some(view);
                let _ = reply.send(self.render_view(view));
            }
            EngineMsg::GetFrame { reply } => {
                let _ = reply.send(self.latest_frame.clone());
            }
            EngineMsg::GetMetadata { reply } => {
                let _ = reply.send(self.current.as_ref().map(|c| c.meta.clone()));
            }
            EngineMsg::CloseImage { reply } => {
                self.flush_sidecar_now();
                self.generation += 1;
                self.current = None;
                self.latest_frame = None;
                self.render_at = None;
                self.settle_at = None;
                if let Some(g) = &mut self.graph {
                    g.invalidate_all();
                }
                let _ = reply.send(());
            }
            // ---- ops ----
            EngineMsg::ApplyOp { op, reply } => {
                let result = self.do_apply_op(op);
                if let Ok(delta) = &result {
                    self.emit(EngineEvent::DocUpdated {
                        delta: serde_json::to_value(delta).unwrap_or_default(),
                    });
                }
                let _ = reply.send(result);
            }
            EngineMsg::Undo { reply } => {
                let _ = reply.send(self.do_undo(true));
            }
            EngineMsg::Redo { reply } => {
                let _ = reply.send(self.do_undo(false));
            }
            EngineMsg::GetDoc { reply } => {
                let _ = reply.send(self.current.as_ref().map(|c| c.doc().to_json()));
            }
            EngineMsg::GetHistory { reply } => {
                let _ = reply.send(
                    self.current
                        .as_ref()
                        .map(|c| c.history.labels())
                        .unwrap_or_default(),
                );
            }
            EngineMsg::Snapshot { name, reply } => {
                let _ = reply.send(match &mut self.current {
                    Some(c) => {
                        let doc = c.doc().clone();
                        c.snapshots.retain(|(n, _)| *n != name);
                        c.snapshots.push((name, doc));
                        Ok(())
                    }
                    None => Err(CoreError::NoImage),
                });
            }
            EngineMsg::ListSnapshots { reply } => {
                let _ = reply.send(
                    self.current
                        .as_ref()
                        .map(|c| c.snapshots.iter().map(|(n, _)| n.clone()).collect())
                        .unwrap_or_default(),
                );
            }
            EngineMsg::RestoreSnapshot { name, reply } => {
                let result = (|| {
                    let c = self.current.as_mut().ok_or(CoreError::NoImage)?;
                    let snap = c
                        .snapshots
                        .iter()
                        .find(|(n, _)| *n == name)
                        .map(|(_, d)| d.clone())
                        .ok_or_else(|| CoreError::InvalidOp(format!("no snapshot '{name}'")))?;
                    let before = c.doc().clone();
                    c.history.record(before, format!("restore '{name}'"));
                    *c.doc_mut() = snap;
                    c.doc_dirty = true;
                    Ok(c.delta(format!("restore '{name}'"), None))
                })();
                if result.is_ok() {
                    self.full_redraw();
                }
                if let Ok(delta) = &result {
                    self.emit(EngineEvent::DocUpdated {
                        delta: serde_json::to_value(delta).unwrap_or_default(),
                    });
                }
                let _ = reply.send(result);
            }
            EngineMsg::VirtualCopy { reply } => {
                let _ = reply.send(match &mut self.current {
                    Some(c) => {
                        let mut copy = c.doc().clone();
                        copy.doc_id = format!("vc-{}", c.docs.len());
                        let id = copy.doc_id.clone();
                        c.docs.push(copy); // shares the decoded base buffer
                        Ok(id)
                    }
                    None => Err(CoreError::NoImage),
                });
            }
            EngineMsg::SwitchDoc { doc_id, reply } => {
                let result = (|| {
                    let c = self.current.as_mut().ok_or(CoreError::NoImage)?;
                    let idx = c
                        .docs
                        .iter()
                        .position(|d| d.doc_id == doc_id)
                        .ok_or_else(|| CoreError::InvalidOp(format!("no doc {doc_id}")))?;
                    c.active_doc = idx;
                    c.history.clear();
                    Ok(c.delta(format!("switch to {doc_id}"), None))
                })();
                if result.is_ok() {
                    self.full_redraw();
                }
                let _ = reply.send(result);
            }
            EngineMsg::SavePreset { modules, reply } => {
                let _ = reply.send(match &self.current {
                    Some(c) => {
                        let mut partial = crate::doc::PartialDoc {
                            modules: Default::default(),
                        };
                        for m in modules {
                            if let Some(params) = c.doc().modules.get(&m) {
                                partial.modules.insert(m, params.clone());
                            }
                        }
                        serde_json::to_value(&partial)
                            .map_err(|e| CoreError::Engine(e.to_string()))
                    }
                    None => Err(CoreError::NoImage),
                });
            }
            EngineMsg::FlushSidecar { reply } => {
                self.flush_sidecar_now();
                let _ = reply.send(Ok(()));
            }
            EngineMsg::GetStats { reply } => {
                let _ = reply.send(self.compute_stats());
            }
            EngineMsg::SetMaskOverlay { id, reply } => {
                self.overlay_mask = id;
                if let Some(g) = &mut self.graph {
                    g.invalidate_from_module("masks");
                }
                self.schedule_render();
                let _ = reply.send(());
            }
            EngineMsg::SetPreviewBypass { on, reply } => {
                if self.preview_bypass != on {
                    self.preview_bypass = on;
                    if let Some(g) = &mut self.graph {
                        g.invalidate_all();
                    }
                    self.render_now();
                }
                let _ = reply.send(());
            }
            // ---- Phase 5: catalog ----
            EngineMsg::ImportFolder { path, reply } => {
                let _ = reply.send(self.start_import(path));
            }
            EngineMsg::GetGrid { query, reply } => {
                let _ = reply.send(
                    self.catalog_mut()
                        .and_then(|c| c.grid(&query)),
                );
            }
            EngineMsg::SetAssetMeta { ids, patch, reply } => {
                let _ = reply.send(self.set_asset_meta(&ids, &patch));
            }
            EngineMsg::RebuildIndex { reply } => {
                let _ = reply.send(self.rebuild_index());
            }
            EngineMsg::GetPreviewFile { id, tier, reply } => {
                let p = self.catalog_mut().ok().map(|c| {
                    if tier == "p" {
                        c.preview_path(id)
                    } else {
                        c.thumb_path(id)
                    }
                });
                let _ = reply.send(p.filter(|p| p.exists()));
            }
            EngineMsg::ImportFileDone { import_id, file } => {
                self.import_file_done(import_id, *file);
            }
            EngineMsg::ImportFinished { import_id } => {
                if let Some(st) = &self.import_state {
                    if st.id == import_id {
                        let total = st.done;
                        let queued = self.import_state.take().unwrap().queued_roots;
                        tracing::info!(total, "import finished");
                        self.emit(EngineEvent::ImportDone { total });
                        self.emit(EngineEvent::CatalogChanged);
                        // continue a multi-root rebuild
                        if let Some(next) = queued.first().cloned() {
                            match self.start_import(next) {
                                Ok(_) => {
                                    if let Some(st) = &mut self.import_state {
                                        st.queued_roots = queued[1..].to_vec();
                                    }
                                }
                                Err(e) => tracing::error!(error = %e, "queued import failed"),
                            }
                        }
                    }
                }
            }
            // ---- Phase 7 ----
            EngineMsg::ExportImage { settings, reply } => {
                self.export_image(settings, reply);
            }
            EngineMsg::SavePresetToDisk {
                name,
                modules,
                reply,
            } => {
                let _ = reply.send(self.save_preset_to_disk(&name, &modules));
            }
            EngineMsg::ListPresets { reply } => {
                let _ = reply.send(list_presets());
            }
            EngineMsg::ApplyPresetByName { name, reply } => {
                let result = (|| {
                    let partial = load_preset(&name)?;
                    self.do_apply_op(Op::ApplyPreset { preset: partial })
                })();
                if let Ok(delta) = &result {
                    self.emit(EngineEvent::DocUpdated {
                        delta: serde_json::to_value(delta).unwrap_or_default(),
                    });
                }
                let _ = reply.send(result);
            }
            EngineMsg::GetPerfStats { reply } => {
                let _ = reply.send(self.perf.clone());
            }
            // ---- Phase 6: assistant eyes ----
            EngineMsg::RenderPreviewJpeg { max_dim, reply } => {
                let _ = reply.send(self.render_preview_jpeg(max_dim));
            }
            EngineMsg::SampleColor { x, y, reply } => {
                let _ = reply.send(self.sample_color(x, y));
            }
            EngineMsg::SegmentDone {
                generation,
                mask_id,
                source_hash,
                result,
            } => {
                if generation != self.generation {
                    return;
                }
                let Some(cur) = &mut self.current else { return };
                cur.pending_segments.remove(&mask_id);
                match result {
                    Ok(mask) => {
                        let Some(gpu) = &self.gpu else { return };
                        let tex = crate::graph::upload_small_mask(
                            gpu,
                            &mask.data,
                            mask.width as u32,
                            mask.height as u32,
                        );
                        cur.masks_gpu.insert(mask_id.clone(), (source_hash, tex));
                        if let Some(g) = &mut self.graph {
                            g.invalidate_from_module("masks");
                        }
                        self.schedule_render();
                        self.emit(EngineEvent::MaskReady { id: mask_id });
                    }
                    Err(e) => {
                        tracing::error!(error = %e, mask_id, "segmentation failed");
                    }
                }
            }
            EngineMsg::WbFromPoint { x, y, reply } => {
                let result = self.wb_from_point(x, y);
                if let Ok(delta) = &result {
                    self.emit(EngineEvent::DocUpdated {
                        delta: serde_json::to_value(delta).unwrap_or_default(),
                    });
                }
                let _ = reply.send(result);
            }
        }
    }

    /// CPU histogram over the latest rendered frame (display-referred,
    /// viewport res — contract F1). Letterbox pixels (exact bg triplet)
    /// excluded.
    fn compute_stats(&self) -> Option<crate::message::FrameStats> {
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

    /// WB eyedropper (spec 3.5): sample a 5×5 patch of the retained CPU
    /// copy at normalized (x, y), solve temp/tint that neutralizes it,
    /// apply both params as ONE undoable history step.
    fn wb_from_point(&mut self, x: f32, y: f32) -> Result<DocDelta, CoreError> {
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

    // ---- op machinery ----

    fn do_apply_op(&mut self, op: Op) -> Result<DocDelta, CoreError> {
        let c = self.current.as_mut().ok_or(CoreError::NoImage)?;
        let before = c.doc().clone();
        let new_mask_id = ops::apply_op(c.doc_mut(), &op)?;
        let label = op.label();
        c.history.record(before, label.clone());
        c.doc_dirty = true;
        let delta = c.delta(label, new_mask_id);
        match op.affected_module() {
            Some(m) => {
                if let Some(g) = &mut self.graph {
                    g.invalidate_from_module(&m);
                }
            }
            None => {
                if let Some(g) = &mut self.graph {
                    g.invalidate_all();
                }
            }
        }
        self.schedule_render();
        self.schedule_settle();
        Ok(delta)
    }

    fn do_undo(&mut self, undo: bool) -> Result<DocDelta, CoreError> {
        let c = self.current.as_mut().ok_or(CoreError::NoImage)?;
        let cur_doc = c.doc().clone();
        let restored = if undo {
            c.history.undo(&cur_doc)
        } else {
            c.history.redo(&cur_doc)
        };
        let Some((doc, label)) = restored else {
            return Err(CoreError::InvalidOp(
                if undo { "nothing to undo" } else { "nothing to redo" }.into(),
            ));
        };
        *c.doc_mut() = doc;
        c.doc_dirty = true;
        let delta = c.delta(label, None);
        self.full_redraw();
        self.schedule_settle();
        self.emit(EngineEvent::DocUpdated {
            delta: serde_json::to_value(&delta).unwrap_or_default(),
        });
        Ok(delta)
    }

    fn full_redraw(&mut self) {
        if let Some(g) = &mut self.graph {
            g.invalidate_all();
        }
        self.schedule_render();
    }

    fn on_settle(&mut self) {
        self.flush_sidecar_now();
    }

    fn flush_sidecar_now(&mut self) {
        if let Some(c) = &mut self.current {
            // primary doc only; virtual copies are in-memory until P5
            if c.doc_dirty && c.active_doc == 0 {
                match sidecar::write_sidecar(&c.docs[0]) {
                    Ok(p) => {
                        c.doc_dirty = false;
                        tracing::debug!(path = %p.display(), "sidecar written");
                    }
                    Err(e) => tracing::error!(error = %e, "sidecar write failed"),
                }
            }
        }
    }

    // ---- open/decode ----

    fn open_image(
        &mut self,
        path: PathBuf,
        reply: oneshot::Sender<Result<ImageMeta, CoreError>>,
    ) {
        self.flush_sidecar_now();
        self.generation += 1;
        let generation = self.generation;

        if !self.decoder.probe(&path) {
            let _ = reply.send(Err(CoreError::Decode(format!(
                "unsupported file: {}",
                path.display()
            ))));
            return;
        }
        let meta = match self.decoder.metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                let _ = reply.send(Err(e));
                return;
            }
        };

        // sidecar = canonical for edits; else fresh doc
        let doc = match sidecar::load_sidecar(&path) {
            Ok(Some(d)) => {
                tracing::info!("sidecar loaded");
                d
            }
            Ok(None) => EditDoc::new(&path.to_string_lossy()),
            Err(e) => {
                tracing::warn!(error = %e, "sidecar unreadable; starting fresh");
                EditDoc::new(&path.to_string_lossy())
            }
        };

        self.current = Some(CurrentImage {
            path: path.clone(),
            meta: meta.clone(),
            working: None,
            small_cpu: None,
            docs: vec![doc],
            active_doc: 0,
            history: History::default(),
            snapshots: Vec::new(),
            doc_dirty: false,
            masks_gpu: Default::default(),
            pending_segments: Default::default(),
        });
        self.latest_frame = None;
        if let Some(g) = &mut self.graph {
            g.invalidate_all();
        }
        let _ = reply.send(Ok(meta));

        // fast path: embedded preview
        {
            let tx = self.self_tx.clone();
            let path = path.clone();
            std::thread::Builder::new()
                .name("preview-worker".into())
                .spawn(move || {
                    let dec = RawlerDecoder::default();
                    match dec.embedded_preview(&path, 2560) {
                        Ok(Some((rgba, width, height))) => {
                            let _ = tx.blocking_send(EngineMsg::PreviewDone {
                                generation,
                                rgba,
                                width,
                                height,
                            });
                        }
                        Ok(None) => tracing::warn!("no embedded preview"),
                        Err(e) => tracing::warn!(error = %e, "preview extract failed"),
                    }
                })
                .expect("spawn preview worker");
        }
        // full decode
        {
            let tx = self.self_tx.clone();
            std::thread::Builder::new()
                .name("decode-worker".into())
                .spawn(move || {
                    let dec = RawlerDecoder::default();
                    let started = Instant::now();
                    let result = dec
                        .decode(&path)
                        .map(|img| Box::new(DecodedPayload::from_decoded(img)));
                    tracing::info!(
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        ok = result.is_ok(),
                        "full decode finished"
                    );
                    let _ = tx.blocking_send(EngineMsg::DecodeDone { generation, result });
                })
                .expect("spawn decode worker");
        }
    }

    fn finish_decode(&mut self, payload: DecodedPayload) {
        let Some(gpu) = &self.gpu else {
            self.emit(EngineEvent::DecodeError {
                message: "gpu unavailable; cannot display decoded image".into(),
            });
            return;
        };
        let tex = upload_working_texture(gpu, &payload.rgba_f16, payload.width, payload.height);
        let view = tex.create_view(&Default::default());
        if let Some(cur) = &mut self.current {
            cur.working = Some((tex, view, payload.width, payload.height));
            cur.small_cpu = Some(payload.small_cpu);
            cur.meta = payload.meta;
        }
        let vw = self
            .last_view
            .map(|v| (v.out_w, v.out_h))
            .unwrap_or((1440, 860));
        let fit = ViewParams::fit(vw.0, vw.1);
        match self.render_view(fit) {
            Ok(info) => {
                self.emit(EngineEvent::ImageReady {
                    version: info.version,
                });
                self.emit(EngineEvent::FrameReady {
                    version: info.version,
                });
            }
            Err(e) => {
                tracing::error!(error = %e, "post-decode render failed");
                self.emit(EngineEvent::DecodeError {
                    message: e.to_string(),
                });
            }
        }
    }

    // ---- rendering ----

    // ---- catalog machinery ----

    fn catalog_mut(&mut self) -> Result<&mut crate::catalog::Catalog, CoreError> {
        if self.catalog.is_none() {
            self.catalog = Some(crate::catalog::Catalog::open_default()?);
        }
        Ok(self.catalog.as_mut().unwrap())
    }

    fn start_import(&mut self, root: PathBuf) -> Result<u64, CoreError> {
        if self.import_state.is_some() {
            return Err(CoreError::Engine("import already running".into()));
        }
        let cat = self.catalog_mut()?;
        let files = crate::catalog::scan_folder(&root);
        // incremental: skip files already imported at the same mtime
        let todo: Vec<PathBuf> = files
            .into_iter()
            .filter(|p| {
                let m = std::fs::metadata(p)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                !cat.is_current(&p.to_string_lossy(), m)
            })
            .collect();
        cat.remember_folder(&root.to_string_lossy())?;
        let total = todo.len() as u64;
        let import_id = self.generation.wrapping_add(1000) + total;
        self.import_state = Some(ImportState {
            id: import_id,
            total,
            done: 0,
            queued_roots: Vec::new(),
        });
        if total == 0 {
            self.import_state = None;
            self.emit(EngineEvent::ImportDone { total: 0 });
            return Ok(0);
        }
        // 4 parallel workers chew chunks; results post back to the actor
        let chunks: Vec<Vec<PathBuf>> = {
            let n = 4.min(todo.len());
            let mut cs: Vec<Vec<PathBuf>> = (0..n).map(|_| Vec::new()).collect();
            for (i, p) in todo.into_iter().enumerate() {
                cs[i % n].push(p);
            }
            cs
        };
        let pending = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(chunks.len()));
        for chunk in chunks {
            let tx = self.self_tx.clone();
            let pending = pending.clone();
            std::thread::Builder::new()
                .name("import-worker".into())
                .spawn(move || {
                    for p in chunk {
                        let result = crate::catalog::import_one(&p);
                        let _ = tx.blocking_send(EngineMsg::ImportFileDone {
                            import_id,
                            file: Box::new(result),
                        });
                    }
                    if pending.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                        let _ = tx.blocking_send(EngineMsg::ImportFinished { import_id });
                    }
                })
                .expect("spawn import worker");
        }
        Ok(total)
    }

    fn import_file_done(
        &mut self,
        import_id: u64,
        file: Result<crate::catalog::ImportedFile, CoreError>,
    ) {
        let Some(st) = &mut self.import_state else { return };
        if st.id != import_id {
            return; // stale import
        }
        st.done += 1;
        let (done, total) = (st.done, st.total);
        match file {
            Ok(f) => {
                if let Ok(cat) = self.catalog_mut() {
                    match cat.upsert_asset(
                        &f.path,
                        &f.folder,
                        &f.partial_hash,
                        f.size,
                        f.modified_ms,
                        &f.meta,
                        f.sidecar_meta.as_ref(),
                        f.has_edits,
                        f.phash.as_deref(),
                        f.blur_score,
                    ) {
                        Ok(id) => {
                            if let Some(bytes) = &f.thumb_jpeg {
                                let _ = std::fs::write(cat.thumb_path(id), bytes);
                                let _ = cat.mark_thumb(id);
                            }
                            if let Some(bytes) = &f.preview_jpeg {
                                let _ = std::fs::write(cat.preview_path(id), bytes);
                            }
                        }
                        Err(e) => tracing::error!(error = %e, path = %f.path, "upsert failed"),
                    }
                }
            }
            Err(e) => tracing::warn!(error = %e, "import file failed"),
        }
        self.emit(EngineEvent::ImportProgress { done, total });
        if done % 8 == 0 {
            self.emit(EngineEvent::CatalogChanged); // stream the grid in
        }
    }

    /// DB write + sidecar mirror (+ live doc update if the image is open).
    fn set_asset_meta(
        &mut self,
        ids: &[i64],
        patch: &crate::catalog::MetaPatch,
    ) -> Result<(), CoreError> {
        let touched = self.catalog_mut()?.set_meta(ids, patch)?;
        for (_, path) in &touched {
            // mirror to sidecar (sidecar = portable truth)
            let p = std::path::Path::new(path);
            let mut doc = match sidecar::load_sidecar(p) {
                Ok(Some(d)) => d,
                _ => EditDoc::new(path),
            };
            apply_meta_patch(&mut doc, patch);
            if let Err(e) = sidecar::write_sidecar(&doc) {
                tracing::warn!(error = %e, path, "sidecar mirror failed");
            }
            // live doc update if this image is open
            if let Some(cur) = &mut self.current {
                if cur.path.to_string_lossy() == *path {
                    apply_meta_patch(cur.doc_mut(), patch);
                    cur.doc_dirty = true;
                }
            }
        }
        self.emit(EngineEvent::CatalogChanged);
        Ok(())
    }

    /// Wipe + rescan the manifest roots (R8 recovery path). Roots run
    /// sequentially; later roots are queued onto the import state.
    fn rebuild_index(&mut self) -> Result<u64, CoreError> {
        let dir = crate::catalog::data_dir();
        let roots = crate::catalog::Catalog::folders_from_manifest(&dir);
        self.catalog_mut()?.wipe_assets()?;
        let mut iter = roots.into_iter();
        let Some(first) = iter.next() else {
            return Ok(0);
        };
        let queued = self.start_import(PathBuf::from(first))?;
        if let Some(st) = &mut self.import_state {
            st.queued_roots = iter.map(PathBuf::from).collect();
        }
        Ok(queued)
    }

    /// Kick off segmentation inference for any segmented mask whose source
    /// changed (cache by source hash — never re-infer on param edits).
    fn ensure_segmentations(&mut self) {
        let Some(cur) = &mut self.current else { return };
        let Some(small) = cur.small_cpu.clone() else { return };
        let generation = self.generation;
        // collect work first (avoid holding doc borrow while mutating)
        let jobs: Vec<(String, String, u64, serde_json::Value)> = cur.docs[cur.active_doc]
            .masks
            .iter()
            .filter(|m| {
                m.source.get("type").and_then(|t| t.as_str()) == Some("segmented")
            })
            .map(|m| {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                m.kind.hash(&mut h);
                m.source.to_string().hash(&mut h);
                (m.id.clone(), m.kind.clone(), h.finish(), m.source.clone())
            })
            .collect();
        for (id, kind, hash, source) in jobs {
            let cached = cur.masks_gpu.get(&id).map(|(h, _)| *h) == Some(hash);
            if cached || cur.pending_segments.contains(&id) {
                continue;
            }
            cur.pending_segments.insert(id.clone());
            let tx = self.self_tx.clone();
            let img = small.clone();
            std::thread::Builder::new()
                .name("segment-worker".into())
                .spawn(move || {
                    use crate::segment::{Segmenter, TractSegmenter};
                    let seg = TractSegmenter;
                    let started = Instant::now();
                    // segmentation runs on a further-downscaled copy
                    let input = img.downscale_to(768);
                    let result = match kind.as_str() {
                        "subject" | "background" => seg.subject(&input),
                        "sky" => seg.sky(&input),
                        "object" => {
                            let p = source
                                .get("hint")
                                .and_then(|h| h.get("point"))
                                .and_then(|p| p.as_array())
                                .and_then(|a| {
                                    Some((
                                        a.first()?.as_f64()? as f32,
                                        a.get(1)?.as_f64()? as f32,
                                    ))
                                })
                                .unwrap_or((0.5, 0.5));
                            seg.object(&input, p)
                        }
                        other => Err(CoreError::InvalidOp(format!(
                            "kind {other} is not segmented"
                        ))),
                    };
                    tracing::info!(
                        kind = %kind,
                        ms = started.elapsed().as_millis() as u64,
                        ok = result.is_ok(),
                        "segmentation finished"
                    );
                    let _ = tx.blocking_send(EngineMsg::SegmentDone {
                        generation,
                        mask_id: id,
                        source_hash: hash,
                        result,
                    });
                })
                .expect("spawn segment worker");
        }
    }

    /// Render the current doc through the graph at `view`. Synchronous on
    /// the actor (proxy-res, <16ms-class) — debouncing upstream keeps op
    /// storms from stacking renders.
    fn render_view(&mut self, view: ViewParams) -> Result<FrameInfo, CoreError> {
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

    /// Tiled full-res export (gpu-memory spec §5): linear tiles on the
    /// actor's GPU, then resize/transform/sharpen/encode on a worker.
    fn export_image(
        &mut self,
        settings: crate::export::ExportSettings,
        reply: oneshot::Sender<Result<String, CoreError>>,
    ) {
        const TILE: u32 = 1024;
        let result: Result<(Vec<f32>, u32, u32, String, bool, ImageMeta), CoreError> = (|| {
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
            let cur = self.current.as_ref().unwrap();
            let (_, tex_view, _, _) = cur.working.as_ref().unwrap();
            let seg_views: std::collections::HashMap<String, wgpu::TextureView> = cur
                .masks_gpu
                .iter()
                .map(|(id, (_, tex))| (id.clone(), tex.create_view(&Default::default())))
                .collect();
            let graph = self.export_graph.as_mut().unwrap();

            let mut full = vec![0.0f32; (w as usize) * (h as usize) * 3];
            let t0 = Instant::now();
            for ty in (0..h).step_by(TILE as usize) {
                for tx in (0..w).step_by(TILE as usize) {
                    let tw = TILE.min(w - tx);
                    let th = TILE.min(h - ty);
                    let view = ViewParams {
                        out_w: tw,
                        out_h: th,
                        scale: Some(1.0),
                        center_x: (tx as f32 + tw as f32 / 2.0) / w as f32,
                        center_y: (ty as f32 + th as f32 / 2.0) / h as f32,
                    };
                    let tile = graph.render_linear_tile(
                        gpu,
                        tex_view,
                        w,
                        h,
                        &view,
                        cur.doc(),
                        cur.as_shot_cct(),
                        &seg_views,
                    )?;
                    for row in 0..th as usize {
                        let src = row * tw as usize * 3;
                        let dst = ((ty as usize + row) * w as usize + tx as usize) * 3;
                        full[dst..dst + tw as usize * 3]
                            .copy_from_slice(&tile[src..src + tw as usize * 3]);
                    }
                }
            }
            tracing::info!(
                ms = t0.elapsed().as_millis() as u64,
                w,
                h,
                "export render (tiled) done"
            );
            Ok((full, w, h, source_path, graph.look(), cur.meta.clone()))
        })();

        match result {
            Err(e) => {
                let _ = reply.send(Err(e));
            }
            Ok((full, w, h, source_path, camera_look, meta)) => {
                // CPU-heavy half on a worker; reply when written
                std::thread::Builder::new()
                    .name("export-worker".into())
                    .spawn(move || {
                        let t0 = Instant::now();
                        let result = (|| {
                            let (lin, w, h) = match settings.max_dim {
                                Some(d) => crate::export::resize_linear(full, w, h, d),
                                None => (full, w, h),
                            };
                            let want16 =
                                settings.format == crate::export::ExportFormat::Tiff16;
                            let mut enc = crate::export::output_transform(
                                &lin,
                                w,
                                h,
                                settings.target,
                                want16,
                                camera_look,
                            );
                            if !want16 {
                                crate::export::output_sharpen8(
                                    &mut enc.rgb8,
                                    w,
                                    h,
                                    settings.sharpen,
                                );
                            }
                            crate::export::encode_and_write(&enc, &settings, &source_path, &meta)
                                .map(|p| p.to_string_lossy().into_owned())
                        })();
                        tracing::info!(
                            ms = t0.elapsed().as_millis() as u64,
                            ok = result.is_ok(),
                            "export encode done"
                        );
                        let _ = reply.send(result);
                    })
                    .expect("spawn export worker");
            }
        }
    }

    fn save_preset_to_disk(
        &mut self,
        name: &str,
        modules: &[String],
    ) -> Result<(), CoreError> {
        let c = self.current.as_ref().ok_or(CoreError::NoImage)?;
        let mut partial = crate::doc::PartialDoc {
            modules: Default::default(),
        };
        for m in modules {
            if let Some(params) = c.doc().modules.get(m) {
                partial.modules.insert(m.clone(), params.clone());
            }
        }
        let dir = presets_dir();
        std::fs::create_dir_all(&dir)?;
        let safe: String = name
            .chars()
            .map(|ch| if ch.is_alphanumeric() || ch == '-' || ch == '_' { ch } else { '_' })
            .collect();
        std::fs::write(
            dir.join(format!("{safe}.json")),
            serde_json::to_string_pretty(&partial).map_err(|e| CoreError::Io(e.to_string()))?,
        )?;
        Ok(())
    }

    /// Current-state preview for the assistant (≤ max_dim, JPEG). Uses a
    /// dedicated graph so viewport caches stay warm.
    fn render_preview_jpeg(&mut self, max_dim: u32) -> Result<Vec<u8>, CoreError> {
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

    /// Working + display color at a normalized point (assistant see-tool).
    fn sample_color(&self, x: f32, y: f32) -> Result<crate::message::SampledColor, CoreError> {
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

    /// Debounced render tick: re-render last view, push frame event.
    fn render_now(&mut self) {
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

fn presets_dir() -> PathBuf {
    crate::catalog::data_dir().join("presets")
}

fn list_presets() -> Vec<String> {
    std::fs::read_dir(presets_dir())
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let p = e.path();
                    (p.extension()? == "json")
                        .then(|| p.file_stem().map(|s| s.to_string_lossy().into_owned()))?
                })
                .collect()
        })
        .unwrap_or_default()
}

fn load_preset(name: &str) -> Result<crate::doc::PartialDoc, CoreError> {
    let text = std::fs::read_to_string(presets_dir().join(format!("{name}.json")))?;
    serde_json::from_str(&text).map_err(|e| CoreError::Io(format!("preset parse: {e}")))
}

/// Apply a catalog MetaPatch onto a doc's meta block (sidecar mirror).
fn apply_meta_patch(doc: &mut EditDoc, patch: &crate::catalog::MetaPatch) {
    if let Some(r) = patch.rating {
        doc.meta.rating = r.min(5);
    }
    if let Some(f) = &patch.flag {
        doc.meta.flag = f.clone();
    }
    if let Some(l) = &patch.label {
        doc.meta.label = l.clone();
    }
    if let Some(kw) = &patch.add_keyword {
        if !doc.meta.keywords.contains(kw) {
            doc.meta.keywords.push(kw.clone());
        }
    }
    if let Some(kw) = &patch.remove_keyword {
        doc.meta.keywords.retain(|k| k != kw);
    }
    doc.touch();
}

/// P0 transport-proof pattern; also the no-image fallback frame.
fn render_test_frame(width: u32, height: u32, version: u64) -> Frame {
    let (w, h) = (width.max(1), height.max(1));
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            rgba[i] = (fx * 200.0) as u8 + 20;
            rgba[i + 1] = 40;
            rgba[i + 2] = (fy * 200.0) as u8 + 35;
            rgba[i + 3] = 255;
            let (cx, cy) = (w / 2, h / 2);
            if (x == cx && y.abs_diff(cy) < 40) || (y == cy && x.abs_diff(cx) < 40) {
                rgba[i] = 255;
                rgba[i + 1] = 255;
                rgba[i + 2] = 255;
            }
        }
    }
    Frame {
        width: w,
        height: h,
        rgba,
        version,
    }
}
