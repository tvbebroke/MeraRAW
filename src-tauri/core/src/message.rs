//! EngineMsg: the command→engine channel vocabulary (contract C1). Grows
//! per phase (P2 ApplyOp/Undo, P4 AddMask, P5 Import/Search, P7 Export).

use crate::error::CoreError;
use crate::gpu::display::ViewParams;
use crate::raw::{DecodedImage, ImageMeta};
use serde::Serialize;
use std::path::PathBuf;
use tokio::sync::oneshot;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub alive: bool,
    pub gpu_ready: bool,
    pub adapter: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub gpu_adapter: Option<String>,
    pub gpu_backend: Option<String>,
}

/// A CPU-side RGBA8 frame ready for the webview canvas.
#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampledColor {
    /// linear Rec.2020 working-space value (scene-referred, pre-edit)
    pub working: [f32; 3],
    /// friendly display sRGB 0-255 of the base image at that point
    pub display: [u8; 3],
}

/// Instrumentation readout (spec 7.5; budgets in test strategy §7).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfStats {
    pub last_render_ms: u64,
    pub last_passes: Vec<String>,
    pub renders: u64,
    /// renders that reused at least one upstream cache (cache-the-chain proof)
    pub cached_renders: u64,
    pub last_decode_ms: u64,
}

/// RGB + luma histogram and clip percentages (contract F1).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameStats {
    pub bins: usize,
    pub r: Vec<u32>,
    pub g: Vec<u32>,
    pub b: Vec<u32>,
    pub luma: Vec<u32>,
    pub clip_high_pct: f32,
    pub clip_low_pct: f32,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameInfo {
    pub version: u64,
    pub width: u32,
    pub height: u32,
}

/// Engine → frontend push events (Tauri layer forwards as named events,
/// contract C3). Core stays Tauri-free.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum EngineEvent {
    PreviewReady { version: u64 },
    ImageReady { version: u64 },
    FrameReady { version: u64 },
    DecodeError { message: String },
    /// Canonical doc changed (op/undo/redo/restore) — frontend reconciles.
    DocUpdated { delta: serde_json::Value },
    /// A segmentation mask finished inference and is now rendering.
    MaskReady { id: String },
    ImportProgress { done: u64, total: u64 },
    ImportDone { total: u64 },
    /// Catalog rows changed (ratings/flags/imports) — grids should refresh.
    CatalogChanged,
    /// Tiled export progress (phase: "render" | "encode", done/total tiles or 1/1).
    ExportProgress {
        phase: String,
        done: u32,
        total: u32,
    },
    /// Engine thread recovered from a panic; UI should prompt restart.
    EngineCrashed { message: String },
}

pub enum EngineMsg {
    Ping {
        reply: oneshot::Sender<EngineStatus>,
    },
    Info {
        reply: oneshot::Sender<EngineInfo>,
    },
    /// Test pattern (P0 transport proof; also the no-image fallback).
    TestFrame {
        width: u32,
        height: u32,
        reply: oneshot::Sender<Frame>,
    },
    // ---- Phase 1 ----
    OpenImage {
        path: PathBuf,
        reply: oneshot::Sender<Result<ImageMeta, CoreError>>,
    },
    RequestFrame {
        view: ViewParams,
        reply: oneshot::Sender<Result<FrameInfo, CoreError>>,
    },
    GetFrame {
        reply: oneshot::Sender<Option<Frame>>,
    },
    GetMetadata {
        reply: oneshot::Sender<Option<ImageMeta>>,
    },
    CloseImage {
        reply: oneshot::Sender<()>,
    },
    // ---- Phase 2: ops on the canonical doc ----
    ApplyOp {
        op: crate::ops::Op,
        /// true during an interactive drag — coalesce into one undo entry.
        live: bool,
        reply: oneshot::Sender<Result<crate::ops::DocDelta, CoreError>>,
    },
    Undo {
        reply: oneshot::Sender<Result<crate::ops::DocDelta, CoreError>>,
    },
    Redo {
        reply: oneshot::Sender<Result<crate::ops::DocDelta, CoreError>>,
    },
    GetDoc {
        reply: oneshot::Sender<Option<serde_json::Value>>,
    },
    GetHistory {
        reply: oneshot::Sender<Vec<String>>,
    },
    Snapshot {
        name: String,
        reply: oneshot::Sender<Result<(), CoreError>>,
    },
    ListSnapshots {
        reply: oneshot::Sender<Vec<String>>,
    },
    RestoreSnapshot {
        name: String,
        reply: oneshot::Sender<Result<crate::ops::DocDelta, CoreError>>,
    },
    /// Clone the active doc (new id), sharing the decoded base buffer.
    VirtualCopy {
        reply: oneshot::Sender<Result<String, CoreError>>,
    },
    SwitchDoc {
        doc_id: String,
        reply: oneshot::Sender<Result<crate::ops::DocDelta, CoreError>>,
    },
    /// Extract chosen modules' params as a PartialDoc (preset save).
    SavePreset {
        modules: Vec<String>,
        reply: oneshot::Sender<Result<serde_json::Value, CoreError>>,
    },
    /// Force sidecar write now (close/quit path).
    FlushSidecar {
        reply: oneshot::Sender<Result<(), CoreError>>,
    },
    // ---- Phase 3 ----
    /// Histogram + clip stats over the latest rendered frame (contract F1).
    GetStats {
        reply: oneshot::Sender<Option<FrameStats>>,
    },
    /// WB eyedropper: neutralize the sampled (normalized) point via two
    /// guard-walled SetParam ops recorded as one undoable step.
    WbFromPoint {
        x: f32,
        y: f32,
        reply: oneshot::Sender<Result<crate::ops::DocDelta, CoreError>>,
    },
    // ---- Phase 4 ----
    /// Toggle the viewport mask overlay (None = off).
    SetMaskOverlay {
        id: Option<String>,
        reply: oneshot::Sender<()>,
    },
    /// Before/after: render the un-edited base (edit chain bypassed) while on.
    SetPreviewBypass {
        on: bool,
        reply: oneshot::Sender<()>,
    },
    /// Display look: 0 = Neutral, 1 = Camera/punchy, 2 = Filmic/AgX.
    SetDisplayLook {
        look: u32,
        reply: oneshot::Sender<()>,
    },
    // ---- Phase 5: catalog ----
    ImportFolder {
        path: PathBuf,
        reply: oneshot::Sender<Result<u64, CoreError>>,
    },
    ScanImportFolder {
        path: PathBuf,
        reply: oneshot::Sender<Result<Vec<crate::catalog::ImportCandidate>, CoreError>>,
    },
    ImportSelected {
        root: PathBuf,
        paths: Vec<PathBuf>,
        reply: oneshot::Sender<Result<u64, CoreError>>,
    },
    GetAssetDetail {
        id: i64,
        reply: oneshot::Sender<Result<Option<crate::catalog::AssetDetail>, CoreError>>,
    },
    ListAlbums {
        reply: oneshot::Sender<Result<Vec<crate::catalog::AlbumItem>, CoreError>>,
    },
    CreateAlbum {
        name: String,
        reply: oneshot::Sender<Result<i64, CoreError>>,
    },
    DeleteAlbum {
        id: i64,
        reply: oneshot::Sender<Result<(), CoreError>>,
    },
    AddToAlbum {
        album_id: i64,
        asset_ids: Vec<i64>,
        reply: oneshot::Sender<Result<(), CoreError>>,
    },
    RemoveFromAlbum {
        album_id: i64,
        asset_ids: Vec<i64>,
        reply: oneshot::Sender<Result<(), CoreError>>,
    },
    SetCameraProfile {
        profile_file: String,
        reply: oneshot::Sender<Result<ImageMeta, CoreError>>,
    },
    GetGrid {
        query: crate::catalog::GridQuery,
        reply: oneshot::Sender<Result<Vec<crate::catalog::GridItem>, CoreError>>,
    },
    ListFolders {
        reply: oneshot::Sender<Result<Vec<crate::catalog::FolderItem>, CoreError>>,
    },
    SetAssetMeta {
        ids: Vec<i64>,
        patch: crate::catalog::MetaPatch,
        reply: oneshot::Sender<Result<(), CoreError>>,
    },
    RebuildIndex {
        reply: oneshot::Sender<Result<u64, CoreError>>,
    },
    GetPreviewFile {
        id: i64,
        tier: String, // "t" | "p"
        reply: oneshot::Sender<Option<PathBuf>>,
    },
    // ---- Phase 6: assistant eyes ----
    /// Small current-state preview as JPEG (the only pixels that ever
    /// leave the device — privacy contract 6.7).
    RenderPreviewJpeg {
        max_dim: u32,
        reply: oneshot::Sender<Result<Vec<u8>, CoreError>>,
    },
    /// Working-space + display color at a normalized point (see-tool).
    SampleColor {
        x: f32,
        y: f32,
        reply: oneshot::Sender<Result<SampledColor, CoreError>>,
    },
    // ---- Phase 7: export + presets + perf ----
    /// Export the OPEN image: tiled full-res linear render on the actor,
    /// then transform/encode on a worker. Replies when the file is written.
    ExportImage {
        settings: crate::export::ExportSettings,
        reply: oneshot::Sender<Result<String, CoreError>>,
    },
    SavePresetToDisk {
        name: String,
        modules: Vec<String>,
        reply: oneshot::Sender<Result<(), CoreError>>,
    },
    ListPresets {
        reply: oneshot::Sender<Vec<String>>,
    },
    ApplyPresetByName {
        name: String,
        reply: oneshot::Sender<Result<crate::ops::DocDelta, CoreError>>,
    },
    GetPerfStats {
        reply: oneshot::Sender<PerfStats>,
    },
    // ---- internal (workers → engine) ----
    PreviewDone {
        generation: u64,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
    DecodeDone {
        generation: u64,
        result: Result<Box<DecodedPayload>, CoreError>,
    },
    SegmentDone {
        generation: u64,
        mask_id: String,
        source_hash: u64,
        result: Result<crate::segment::Mask01, CoreError>,
    },
    ImportFileDone {
        import_id: u64,
        file: Box<Result<crate::catalog::ImportedFile, CoreError>>,
    },
    ImportFinished {
        import_id: u64,
    },
    /// Continue incremental tiled export (one tile per actor tick).
    ExportStep,
}

/// Carried from the decode worker thread: f16-packed working master +
/// retained small CPU copy (histogram/segmentation/fallback) + metadata.
pub struct DecodedPayload {
    pub rgba_f16: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub small_cpu: std::sync::Arc<crate::image::RgbF32Buf>,
    pub meta: ImageMeta,
}

impl DecodedPayload {
    pub fn from_decoded(img: DecodedImage) -> Self {
        let rgba_f16 = img.working.to_rgba_f16_bytes();
        let small_cpu = std::sync::Arc::new(img.working.downscale_to(2048));
        Self {
            rgba_f16,
            width: img.working.width as u32,
            height: img.working.height as u32,
            small_cpu,
            meta: img.meta,
        }
    }
}
