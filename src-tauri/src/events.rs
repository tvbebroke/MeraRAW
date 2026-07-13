//! Event name constants + emit helpers. Mirror of src/ipc/events.ts — keep in sync.

use tauri::{AppHandle, Emitter};

pub const ENGINE_READY: &str = "engine-ready";
pub const LOG: &str = "log";
pub const FILE_OPENED: &str = "file-opened";
pub const FOLDER_OPENED: &str = "folder-opened";
pub const PREVIEW_READY: &str = "preview-ready";
pub const IMAGE_READY: &str = "image-ready";
pub const FRAME_READY: &str = "frame-ready";
pub const DECODE_ERROR: &str = "decode-error";
pub const DOC_UPDATED: &str = "doc-updated";
pub const MASK_READY: &str = "mask-ready";
pub const IMPORT_PROGRESS: &str = "import-progress";
pub const IMPORT_DONE: &str = "import-done";
pub const CATALOG_CHANGED: &str = "catalog-changed";
pub const EXPORT_PROGRESS: &str = "export-progress";
pub const EXPORT_BATCH_PROGRESS: &str = "export-batch-progress";
pub const EXPORT_BATCH_DONE: &str = "export-batch-done";
pub const ENGINE_CRASHED: &str = "engine-crashed";
pub const EXPORT_REQUESTED: &str = "export-requested";
pub const IMPORT_REQUESTED: &str = "import-requested";
pub const SETTINGS_REQUESTED: &str = "settings-requested";
pub const ASSISTANT_PROGRESS: &str = "assistant-progress";

/// Forward core EngineEvents to the webview as named events (contract C3).
pub fn forward_engine_event(app: &AppHandle, ev: meratech_core::message::EngineEvent) {
    use meratech_core::message::EngineEvent as E;
    let result = match &ev {
        E::PreviewReady { version } => app.emit(PREVIEW_READY, version),
        E::ImageReady { version } => app.emit(IMAGE_READY, version),
        E::FrameReady { version } => app.emit(FRAME_READY, version),
        E::DecodeError { message } => app.emit(DECODE_ERROR, message.clone()),
        E::DocUpdated { delta } => app.emit(DOC_UPDATED, delta.clone()),
        E::MaskReady { id } => app.emit(MASK_READY, id.clone()),
        E::ImportProgress { done, total } => {
            app.emit(IMPORT_PROGRESS, serde_json::json!({"done": done, "total": total}))
        }
        E::ImportDone { total } => app.emit(IMPORT_DONE, total),
        E::CatalogChanged => app.emit(CATALOG_CHANGED, ()),
        E::ExportProgress { phase, done, total } => app.emit(
            EXPORT_PROGRESS,
            serde_json::json!({"phase": phase, "done": done, "total": total}),
        ),
        E::ExportBatchProgress {
            index,
            count,
            path,
            phase,
            done,
            total,
        } => app.emit(
            EXPORT_BATCH_PROGRESS,
            serde_json::json!({
                "index": index, "count": count, "path": path,
                "phase": phase, "done": done, "total": total,
            }),
        ),
        E::ExportBatchDone {
            ok,
            failed,
            cancelled,
        } => app.emit(
            EXPORT_BATCH_DONE,
            serde_json::json!({
                "ok": ok,
                "failed": failed.iter().map(|(p, e)| {
                    serde_json::json!({"path": p, "error": e})
                }).collect::<Vec<_>>(),
                "cancelled": cancelled,
            }),
        ),
        E::EngineCrashed { message } => app.emit(ENGINE_CRASHED, message.clone()),
    };
    if let Err(e) = result {
        tracing::error!(error = %e, "forward engine event failed");
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EngineReadyPayload {
    pub adapter: Option<String>,
    pub gpu_ready: bool,
}

pub fn emit_engine_ready(app: &AppHandle, adapter: Option<String>, gpu_ready: bool) {
    if let Err(e) = app.emit(ENGINE_READY, EngineReadyPayload { adapter, gpu_ready }) {
        tracing::error!(error = %e, "emit engine-ready failed");
    }
}

pub fn emit_path_event(app: &AppHandle, event: &str, path: &str) {
    if let Err(e) = app.emit(event, path) {
        tracing::error!(error = %e, event, "emit failed");
    }
}
