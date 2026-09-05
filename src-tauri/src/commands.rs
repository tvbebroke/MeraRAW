//! Thin #[tauri::command] handlers. Validate → EngineMsg → await oneshot →
//! Result<T, AppError>. Never do heavy work here (contract C2).

use crate::error::AppError;
use meratech_core::engine::EngineHandle;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};

fn file_dialog(app: &AppHandle) -> FileDialogBuilder<tauri::Wry> {
    let mut dialog = app.dialog().file();
    if let Some(win) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&win);
    }
    dialog
}

fn filepath_to_string(path: FilePath) -> Result<String, AppError> {
    path.into_path()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| AppError::Internal(format!("resolve picked path: {e}")))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub build: String,
    pub gpu_adapter: Option<String>,
    pub gpu_backend: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub path: String,
    pub exists: bool,
    pub is_dir: bool,
    pub size: u64,
    pub modified_ms: Option<u64>,
    pub ext: Option<String>,
}

#[tauri::command]
pub async fn app_info(engine: State<'_, EngineHandle>) -> Result<AppInfo, AppError> {
    let info = engine.info().await?;
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        build: if cfg!(debug_assertions) {
            "debug".to_string()
        } else {
            "release".to_string()
        },
        gpu_adapter: info.gpu_adapter,
        gpu_backend: info.gpu_backend,
    })
}

#[tauri::command]
pub async fn ping_engine(
    engine: State<'_, EngineHandle>,
) -> Result<meratech_core::message::EngineStatus, AppError> {
    Ok(engine.ping().await?)
}

pub const PHOTO_EXTENSIONS: &[&str] = &[
    // RAW (rawler)
    "arw", "nef", "nrw", "cr2", "cr3", "crw", "dng", "raf", "orf", "rw2", "pef", "srw", "erf",
    "kdc", "dcs", "dcr", "iiq", "3fr", "mef", "mos",
    // rendered (image crate + jxl-oxide; HEIC via macOS sips; PSD composite)
    "jpg", "jpeg", "png", "tif", "tiff", "webp", "bmp", "gif", "jxl", "heic", "heif", "hif", "psd",
];

pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "m4v", "mkv", "webm"];

pub const IMAGE_EXTENSIONS: &[&str] = &[
    // PHOTO_EXTENSIONS + VIDEO_EXTENSIONS — keep in sync with the two lists above.
    "arw", "nef", "nrw", "cr2", "cr3", "crw", "dng", "raf", "orf", "rw2", "pef", "srw", "erf",
    "kdc", "dcs", "dcr", "iiq", "3fr", "mef", "mos", "jpg", "jpeg", "png", "tif", "tiff", "webp",
    "bmp", "gif", "jxl", "heic", "heif", "hif", "psd", "mp4", "mov", "m4v", "mkv", "webm",
];

#[tauri::command]
pub async fn pick_file(app: AppHandle, kind: Option<String>) -> Result<Option<String>, AppError> {
    let kind = kind.unwrap_or_default();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let dlg = file_dialog(&app);
        let dlg = match kind.as_str() {
            "video" => dlg
                .set_title("Open video")
                .add_filter("Video", VIDEO_EXTENSIONS),
            "photo" => dlg
                .set_title("Open photo")
                .add_filter("Photos", PHOTO_EXTENSIONS),
            _ => dlg
                .add_filter("All media", IMAGE_EXTENSIONS)
                .add_filter("Photos", PHOTO_EXTENSIONS)
                .add_filter("Video", VIDEO_EXTENSIONS),
        };
        dlg.blocking_pick_file()
    })
    .await
    .map_err(|e| AppError::Internal(format!("file dialog join: {e}")))?;
    match picked {
        Some(p) => Ok(Some(filepath_to_string(p)?)),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn pick_files(app: AppHandle, kind: Option<String>) -> Result<Option<Vec<String>>, AppError> {
    let kind = kind.unwrap_or_default();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let dlg = file_dialog(&app);
        let dlg = match kind.as_str() {
            "video" => dlg
                .set_title("Import videos — choose one or more")
                .add_filter("Video", VIDEO_EXTENSIONS),
            "photo" => dlg
                .set_title("Import photos — choose one or more")
                .add_filter("Photos", PHOTO_EXTENSIONS),
            _ => dlg
                .set_title("Import — choose one or more")
                .add_filter("All media", IMAGE_EXTENSIONS)
                .add_filter("Photos", PHOTO_EXTENSIONS)
                .add_filter("Video", VIDEO_EXTENSIONS),
        };
        dlg.blocking_pick_files()
    })
    .await
    .map_err(|e| AppError::Internal(format!("files dialog join: {e}")))?;
    match picked {
        Some(paths) => {
            let mut out = Vec::with_capacity(paths.len());
            for p in paths {
                out.push(filepath_to_string(p)?);
            }
            Ok(Some(out))
        }
        None => Ok(None),
    }
}

/// Native picker for a 3D look LUT (`.cube`).
#[tauri::command]
pub async fn pick_lut(app: AppHandle) -> Result<Option<String>, AppError> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        file_dialog(&app)
            .add_filter("3D LUT", &["cube"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| AppError::Internal(format!("lut dialog join: {e}")))?;
    match picked {
        Some(p) => Ok(Some(filepath_to_string(p)?)),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>, AppError> {
    tracing::info!("pick_folder: opening native folder dialog");
    let picked = tauri::async_runtime::spawn_blocking(move || {
        file_dialog(&app)
            .set_title("Add Photos — choose a folder")
            .blocking_pick_folder()
    })
    .await
    .map_err(|e| AppError::Internal(format!("folder dialog join: {e}")))?;
    match picked {
        Some(p) => {
            let path = filepath_to_string(p)?;
            tracing::info!(%path, "pick_folder: selected");
            Ok(Some(path))
        }
        None => {
            tracing::info!("pick_folder: cancelled");
            Ok(None)
        }
    }
}

#[tauri::command]
pub async fn read_file_meta(path: String) -> Result<FileMeta, AppError> {
    let p = crate::paths::validate_user_path(&path)?;
    let path = p.to_string_lossy().into_owned();
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase());
    match tokio::fs::metadata(&p).await {
        Ok(md) => Ok(FileMeta {
            path,
            exists: true,
            is_dir: md.is_dir(),
            size: md.len(),
            modified_ms: md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64),
            ext,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(FileMeta {
            path,
            exists: false,
            is_dir: false,
            size: 0,
            modified_ms: None,
            ext,
        }),
        Err(e) => Err(e.into()),
    }
}

#[tauri::command]
/// Tauri deserializes invoke args by field name; these camelCase names are the
/// IPC contract with `src/ipc/commands.ts` and must not be snake_cased.
#[allow(non_snake_case)]
pub async fn open_image(
    engine: State<'_, EngineHandle>,
    path: String,
    docId: Option<String>,
) -> Result<meratech_core::raw::ImageMeta, AppError> {
    let path = crate::paths::validate_existing_path(&path)?;
    // Fail fast with a clear macOS TCC / iCloud message — RawSource reports
    // the same failure as a cryptic "Operation not permitted (os error 1)".
    crate::paths::ensure_readable(&path)?;
    engine
        .open_image(path, docId.filter(|s| !s.is_empty()))
        .await?
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn request_frame(
    engine: State<'_, EngineHandle>,
    view: meratech_core::gpu::display::ViewParams,
) -> Result<meratech_core::message::FrameInfo, AppError> {
    engine.request_frame(view).await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn get_metadata(
    engine: State<'_, EngineHandle>,
) -> Result<Option<meratech_core::raw::ImageMeta>, AppError> {
    Ok(engine.get_metadata().await?)
}

#[tauri::command]
pub async fn close_image(engine: State<'_, EngineHandle>) -> Result<(), AppError> {
    Ok(engine.close_image().await?)
}

// ---- Phase 2: ops on the canonical doc ----

#[tauri::command]
pub async fn apply_op(
    engine: State<'_, EngineHandle>,
    op: meratech_core::ops::Op,
    live: Option<bool>,
) -> Result<meratech_core::ops::DocDelta, AppError> {
    let res = if live.unwrap_or(false) {
        engine.apply_op_live(op).await
    } else {
        engine.apply_op(op).await
    };
    res?.map_err(AppError::from)
}

#[tauri::command]
pub async fn undo(
    engine: State<'_, EngineHandle>,
) -> Result<meratech_core::ops::DocDelta, AppError> {
    engine.undo().await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn redo(
    engine: State<'_, EngineHandle>,
) -> Result<meratech_core::ops::DocDelta, AppError> {
    engine.redo().await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn get_doc(
    engine: State<'_, EngineHandle>,
) -> Result<Option<serde_json::Value>, AppError> {
    Ok(engine.get_doc().await?)
}

#[tauri::command]
pub async fn get_history(engine: State<'_, EngineHandle>) -> Result<Vec<String>, AppError> {
    Ok(engine.get_history().await?)
}

/// Registry is pure static data — no engine round-trip needed.
#[tauri::command]
pub fn get_registry() -> Vec<meratech_core::registry::ParamSpec> {
    meratech_core::registry::all_specs()
        .into_iter()
        .cloned()
        .collect()
}

#[tauri::command]
pub async fn snapshot(engine: State<'_, EngineHandle>, name: String) -> Result<(), AppError> {
    engine.snapshot(name).await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn list_snapshots(engine: State<'_, EngineHandle>) -> Result<Vec<String>, AppError> {
    Ok(engine.list_snapshots().await?)
}

#[tauri::command]
pub async fn restore_snapshot(
    engine: State<'_, EngineHandle>,
    name: String,
) -> Result<meratech_core::ops::DocDelta, AppError> {
    engine.restore_snapshot(name).await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn virtual_copy(
    engine: State<'_, EngineHandle>,
    path: Option<String>,
) -> Result<String, AppError> {
    let path = match path {
        Some(p) if !p.is_empty() => Some(
            crate::paths::validate_existing_path(&p)?
                .to_string_lossy()
                .into_owned(),
        ),
        _ => None,
    };
    engine.virtual_copy(path).await?.map_err(AppError::from)
}

#[tauri::command]
/// Tauri deserializes invoke args by field name; these camelCase names are the
/// IPC contract with `src/ipc/commands.ts` and must not be snake_cased.
#[allow(non_snake_case)]
pub async fn switch_doc(
    engine: State<'_, EngineHandle>,
    docId: String,
) -> Result<meratech_core::ops::DocDelta, AppError> {
    engine.switch_doc(docId).await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn list_docs(
    engine: State<'_, EngineHandle>,
) -> Result<Vec<meratech_core::message::DocRef>, AppError> {
    Ok(engine.list_docs().await?)
}

#[tauri::command]
/// Tauri deserializes invoke args by field name; these camelCase names are the
/// IPC contract with `src/ipc/commands.ts` and must not be snake_cased.
#[allow(non_snake_case)]
pub async fn delete_virtual_copy(
    engine: State<'_, EngineHandle>,
    docId: String,
) -> Result<(), AppError> {
    engine
        .delete_virtual_copy(docId)
        .await?
        .map_err(AppError::from)
}

#[tauri::command]
/// Tauri deserializes invoke args by field name; these camelCase names are the
/// IPC contract with `src/ipc/commands.ts` and must not be snake_cased.
#[allow(non_snake_case)]
pub async fn apply_grade_to_paths(
    engine: State<'_, EngineHandle>,
    paths: Vec<String>,
    modules: meratech_core::doc::ModuleParams,
    lutFile: Option<String>,
) -> Result<u32, AppError> {
    let mut validated = Vec::with_capacity(paths.len());
    for p in paths {
        let path = crate::paths::validate_existing_path(&p)?;
        validated.push(path.to_string_lossy().into_owned());
    }
    engine
        .apply_grade_to_paths(validated, modules, lutFile)
        .await?
        .map_err(AppError::from)
}

fn dig_surface_enabled() -> bool {
    // Dig/selftest probes are debug-only. Production ACL also omits allow-dev-probes;
    // keep this closed in release even if MERATECH_* env vars are set.
    cfg!(debug_assertions)
}

/// Dev/test hook: MERATECH_OPEN=<path> auto-opens a file on launch.
/// Frontend polls this once at boot (deterministic, no event race).
#[tauri::command]
pub async fn autoopen_path() -> Result<Option<String>, AppError> {
    if !dig_surface_enabled() {
        return Ok(None);
    }
    Ok(std::env::var("MERATECH_OPEN")
        .ok()
        .filter(|s| !s.is_empty()))
}

#[tauri::command]
pub async fn get_stats(
    engine: State<'_, EngineHandle>,
) -> Result<Option<meratech_core::message::FrameStats>, AppError> {
    Ok(engine.get_stats().await?)
}

#[tauri::command]
pub async fn wb_from_point(
    engine: State<'_, EngineHandle>,
    x: f32,
    y: f32,
) -> Result<meratech_core::ops::DocDelta, AppError> {
    engine.wb_from_point(x, y).await?.map_err(AppError::from)
}

/// Crop-tool auto-level: dominant line deviation in degrees (original image
/// space, mod-90 folded); 0.0 = no dominant direction found.
#[tauri::command]
pub async fn auto_level(engine: State<'_, EngineHandle>) -> Result<f32, AppError> {
    engine.auto_level().await?.map_err(AppError::from)
}

// ---- Phase 7: export + presets + perf ----

#[tauri::command]
pub async fn export_image(
    app: AppHandle,
    engine: State<'_, EngineHandle>,
    mut settings: meratech_core::export::ExportSettings,
) -> Result<String, AppError> {
    if settings.dest_dir.trim().is_empty() {
        let app2 = app.clone();
        let picked = tauri::async_runtime::spawn_blocking(move || {
            file_dialog(&app2)
                .set_title("Export destination")
                .blocking_pick_folder()
        })
        .await
        .map_err(|e| AppError::Internal(format!("export dialog join: {e}")))?;
        settings.dest_dir = match picked {
            Some(p) => filepath_to_string(p)?,
            None => return Err(AppError::InvalidOp("export cancelled".into())),
        };
    }
    settings.dest_dir = crate::paths::validate_user_path(&settings.dest_dir)?
        .to_string_lossy()
        .into_owned();
    engine.export_image(settings).await?.map_err(AppError::from)
}

/// Batch export: engine decodes + renders each path off the open image.
/// Returns the accepted queue length; progress arrives as
/// export-batch-progress / export-batch-done events.
const MAX_EXPORT_BATCH: usize = 500;

#[tauri::command]
pub async fn export_batch(
    app: AppHandle,
    engine: State<'_, EngineHandle>,
    paths: Vec<String>,
    mut settings: meratech_core::export::ExportSettings,
) -> Result<u32, AppError> {
    if paths.is_empty() {
        return Err(AppError::InvalidOp("export batch empty".into()));
    }
    if paths.len() > MAX_EXPORT_BATCH {
        return Err(AppError::InvalidOp(format!(
            "export batch too large (max {MAX_EXPORT_BATCH})"
        )));
    }
    if settings.dest_dir.trim().is_empty() {
        let app2 = app.clone();
        let picked = tauri::async_runtime::spawn_blocking(move || {
            file_dialog(&app2)
                .set_title("Export destination")
                .blocking_pick_folder()
        })
        .await
        .map_err(|e| AppError::Internal(format!("export dialog join: {e}")))?;
        settings.dest_dir = match picked {
            Some(p) => filepath_to_string(p)?,
            None => return Err(AppError::InvalidOp("export cancelled".into())),
        };
    }
    settings.dest_dir = crate::paths::validate_user_path(&settings.dest_dir)?
        .to_string_lossy()
        .into_owned();
    let mut validated = Vec::with_capacity(paths.len());
    for p in paths {
        validated.push(crate::paths::validate_existing_path(&p)?);
    }
    engine
        .export_batch(validated, settings)
        .await?
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn cancel_export_batch(engine: State<'_, EngineHandle>) -> Result<(), AppError> {
    Ok(engine.export_batch_cancel().await?)
}

#[tauri::command]
pub async fn save_preset(
    engine: State<'_, EngineHandle>,
    name: String,
    modules: Vec<String>,
    grade: Option<meratech_core::doc::ModuleParams>,
) -> Result<(), AppError> {
    engine
        .save_preset_to_disk(name, modules, grade)
        .await?
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn list_presets(engine: State<'_, EngineHandle>) -> Result<Vec<String>, AppError> {
    Ok(engine.list_presets().await?)
}

#[tauri::command]
pub async fn list_preset_catalog(
    engine: State<'_, EngineHandle>,
) -> Result<Vec<meratech_core::doc::PresetCatalogEntry>, AppError> {
    Ok(engine.list_preset_catalog().await?)
}

#[tauri::command]
pub async fn apply_preset(
    engine: State<'_, EngineHandle>,
    name: String,
) -> Result<meratech_core::ops::DocDelta, AppError> {
    engine
        .apply_preset_by_name(name)
        .await?
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn get_perf_stats(
    engine: State<'_, EngineHandle>,
) -> Result<meratech_core::message::PerfStats, AppError> {
    Ok(engine.get_perf_stats().await?)
}

// ---- Denoise ----

#[tauri::command]
pub async fn denoise_estimate_profile(
    engine: State<'_, EngineHandle>,
) -> Result<meratech_core::denoise::NoiseProfile, AppError> {
    engine
        .denoise_estimate_profile()
        .await?
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn denoise_models_list(
    engine: State<'_, EngineHandle>,
) -> Result<Vec<meratech_core::denoise::ai::ModelInfo>, AppError> {
    Ok(engine.denoise_models_list().await?)
}

#[tauri::command]
pub async fn denoise_ai_start(engine: State<'_, EngineHandle>) -> Result<u64, AppError> {
    engine.denoise_ai_start().await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn denoise_ai_cancel(engine: State<'_, EngineHandle>, job: u64) -> Result<(), AppError> {
    Ok(engine.denoise_ai_cancel(job).await?)
}

#[tauri::command]
pub async fn denoise_ai_reset(engine: State<'_, EngineHandle>) -> Result<(), AppError> {
    engine.denoise_ai_reset().await?.map_err(AppError::from)
}

// ---- Phase 5: catalog ----

#[tauri::command]
pub async fn import_folder(engine: State<'_, EngineHandle>, path: String) -> Result<u64, AppError> {
    let path = crate::paths::validate_existing_path(&path)?;
    engine.import_folder(path).await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn scan_import_folder(
    engine: State<'_, EngineHandle>,
    path: String,
) -> Result<Vec<meratech_core::catalog::ImportCandidate>, AppError> {
    let path = crate::paths::validate_existing_path(&path)?;
    engine
        .scan_import_folder(path)
        .await?
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn import_selected(
    engine: State<'_, EngineHandle>,
    root: String,
    paths: Vec<String>,
) -> Result<u64, AppError> {
    let root = crate::paths::validate_existing_path(&root)?;
    // Single-file import: never touch the parent folder (avoids macOS
    // "grant folder access" and importing siblings).
    if root.is_file() {
        return engine
            .import_selected(root.clone(), vec![root])
            .await?
            .map_err(AppError::from);
    }
    if paths.len() == 1 {
        let only = crate::paths::validate_existing_path(&paths[0])?;
        if only.is_file() {
            return engine
                .import_selected(only.clone(), vec![only])
                .await?
                .map_err(AppError::from);
        }
    }
    let root_canon = root.canonicalize().unwrap_or_else(|_| root.clone());
    let mut validated = Vec::with_capacity(paths.len());
    for p in paths {
        let path = crate::paths::validate_existing_path(&p)?;
        let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
        if !canon.starts_with(&root_canon) {
            return Err(AppError::InvalidOp(
                "import path outside selected folder".into(),
            ));
        }
        validated.push(path);
    }
    engine
        .import_selected(root, validated)
        .await?
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn get_asset_detail(
    engine: State<'_, EngineHandle>,
    id: i64,
) -> Result<Option<meratech_core::catalog::AssetDetail>, AppError> {
    engine.get_asset_detail(id).await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn list_albums(
    engine: State<'_, EngineHandle>,
) -> Result<Vec<meratech_core::catalog::AlbumItem>, AppError> {
    engine.list_albums().await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn create_album(engine: State<'_, EngineHandle>, name: String) -> Result<i64, AppError> {
    engine.create_album(name).await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn delete_album(engine: State<'_, EngineHandle>, id: i64) -> Result<(), AppError> {
    engine.delete_album(id).await?.map_err(AppError::from)
}

#[tauri::command]
/// Tauri deserializes invoke args by field name; these camelCase names are the
/// IPC contract with `src/ipc/commands.ts` and must not be snake_cased.
#[allow(non_snake_case)]
pub async fn add_to_album(
    engine: State<'_, EngineHandle>,
    albumId: i64,
    assetIds: Vec<i64>,
) -> Result<(), AppError> {
    engine
        .add_to_album(albumId, assetIds)
        .await?
        .map_err(AppError::from)
}

#[tauri::command]
/// Tauri deserializes invoke args by field name; these camelCase names are the
/// IPC contract with `src/ipc/commands.ts` and must not be snake_cased.
#[allow(non_snake_case)]
pub async fn remove_from_album(
    engine: State<'_, EngineHandle>,
    albumId: i64,
    assetIds: Vec<i64>,
) -> Result<(), AppError> {
    engine
        .remove_from_album(albumId, assetIds)
        .await?
        .map_err(AppError::from)
}

#[tauri::command]
/// Tauri deserializes invoke args by field name; these camelCase names are the
/// IPC contract with `src/ipc/commands.ts` and must not be snake_cased.
#[allow(non_snake_case)]
pub async fn set_camera_profile(
    engine: State<'_, EngineHandle>,
    profileFile: String,
) -> Result<meratech_core::raw::ImageMeta, AppError> {
    engine
        .set_camera_profile(profileFile)
        .await?
        .map_err(AppError::from)
}

/// Load a 3D look LUT (`.cube`) for the current image, or clear it (path=None).
#[tauri::command]
pub async fn set_lut(
    engine: State<'_, EngineHandle>,
    path: Option<String>,
) -> Result<(), AppError> {
    let path = match path {
        Some(p) if p.starts_with("bundled:") || p.starts_with("user:") => Some(p),
        Some(p) if !p.trim().is_empty() => Some(
            crate::paths::validate_existing_path(&p)?
                .to_string_lossy()
                .into_owned(),
        ),
        _ => None,
    };
    engine.set_lut(path).await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn list_looks(
    engine: State<'_, EngineHandle>,
) -> Result<Vec<meratech_core::look::LookInfo>, AppError> {
    Ok(engine.list_looks().await?)
}

#[tauri::command]
pub fn user_looks_dir() -> Result<String, AppError> {
    let dir = meratech_core::look::looks_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|_| AppError::Io("could not create looks folder".into()))?;
    Ok(dir.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn delete_user_look(engine: State<'_, EngineHandle>, id: String) -> Result<(), AppError> {
    meratech_core::look::delete_user_look(&id).map_err(AppError::from)?;
    if let Ok(Some(doc)) = engine.get_doc().await {
        let lut = doc
            .get("meta")
            .and_then(|m| m.get("lut_file"))
            .and_then(|v| v.as_str());
        if lut == Some(id.as_str()) {
            engine.set_lut(None).await?.map_err(AppError::from)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn seek_video(
    engine: State<'_, EngineHandle>,
    frame: u32,
) -> Result<meratech_core::raw::ImageMeta, AppError> {
    engine.seek_video(frame).await?.map_err(AppError::from)
}

/// Change the demosaic algorithm for the current image and re-decode.
#[tauri::command]
pub async fn set_demosaic(
    engine: State<'_, EngineHandle>,
    algo: String,
) -> Result<meratech_core::raw::ImageMeta, AppError> {
    engine.set_demosaic(algo).await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn get_grid(
    engine: State<'_, EngineHandle>,
    query: meratech_core::catalog::GridQuery,
) -> Result<Vec<meratech_core::catalog::GridItem>, AppError> {
    engine.get_grid(query).await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn list_folders(
    engine: State<'_, EngineHandle>,
) -> Result<Vec<meratech_core::catalog::FolderItem>, AppError> {
    engine.list_folders().await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn discover_media_folders(
) -> Result<Vec<meratech_core::catalog::DiscoveredFolder>, AppError> {
    tokio::task::spawn_blocking(meratech_core::catalog::discover_media_folders)
        .await
        .map_err(|e| AppError::Internal(format!("discover join: {e}")))
}

#[tauri::command]
pub async fn list_folder_children(
    path: String,
) -> Result<Vec<meratech_core::catalog::FolderChild>, AppError> {
    let path = crate::paths::validate_existing_path(&path)?;
    if !path.is_dir() {
        return Err(AppError::InvalidOp("not a folder".into()));
    }
    tokio::task::spawn_blocking(move || meratech_core::catalog::list_folder_children(&path))
        .await
        .map_err(|e| AppError::Internal(format!("list children join: {e}")))?
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn forget_folder(engine: State<'_, EngineHandle>, root: String) -> Result<(), AppError> {
    engine.forget_folder(root).await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn set_asset_meta(
    engine: State<'_, EngineHandle>,
    ids: Vec<i64>,
    patch: meratech_core::catalog::MetaPatch,
) -> Result<(), AppError> {
    engine
        .set_asset_meta(ids, patch)
        .await?
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn rebuild_index(
    engine: State<'_, EngineHandle>,
    confirm: bool,
) -> Result<u64, AppError> {
    if !confirm {
        return Err(AppError::InvalidOp(
            "rebuild_index requires confirm: true".into(),
        ));
    }
    engine.rebuild_index().await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn set_mask_overlay(
    engine: State<'_, EngineHandle>,
    id: Option<String>,
    strength: Option<f32>,
    mode: Option<u32>,
) -> Result<(), AppError> {
    Ok(engine.set_mask_overlay(id, strength, mode).await?)
}

#[tauri::command]
pub async fn sample_color(
    engine: State<'_, EngineHandle>,
    x: f32,
    y: f32,
) -> Result<serde_json::Value, AppError> {
    let c = engine.sample_color(x, y).await??;
    Ok(serde_json::json!({
        "working": c.working,
        "display": c.display,
    }))
}

#[tauri::command]
pub async fn propose_object_masks(
    engine: State<'_, EngineHandle>,
) -> Result<serde_json::Value, AppError> {
    let props = engine.propose_object_masks().await??;
    Ok(serde_json::to_value(props).unwrap_or_default())
}

/// Before/after: render the un-edited base while `on`.
#[tauri::command]
pub async fn set_preview_bypass(engine: State<'_, EngineHandle>, on: bool) -> Result<(), AppError> {
    Ok(engine.set_preview_bypass(on).await?)
}

/// Display look: 0 = Neutral, 1 = Camera, 2 = Filmic (AgX).
#[tauri::command]
pub async fn set_display_look(engine: State<'_, EngineHandle>, look: u32) -> Result<(), AppError> {
    Ok(engine.set_display_look(look).await?)
}

/// Toggle highlight / shadow clipping blinkies on the viewport.
#[tauri::command]
pub async fn set_clip_warnings(
    engine: State<'_, EngineHandle>,
    hi: bool,
    lo: bool,
) -> Result<(), AppError> {
    Ok(engine.set_clip_warnings(hi, lo).await?)
}

/// View-only soft proof. `space`: 0 off, 1 sRGB, 2 Display P3, 3 Adobe RGB, 4 ProPhoto.
#[tauri::command]
pub async fn set_proof_target(
    engine: State<'_, EngineHandle>,
    space: u32,
    gamut: bool,
) -> Result<(), AppError> {
    Ok(engine.set_proof_target(space, gamut).await?)
}

#[tauri::command]
pub async fn selftest_enabled() -> Result<String, AppError> {
    if !dig_surface_enabled() {
        return Ok(String::new());
    }
    Ok(std::env::var("MERATECH_SELFTEST").unwrap_or_default())
}

#[tauri::command]
pub async fn live_assistant_enabled() -> Result<bool, AppError> {
    if !dig_surface_enabled() {
        return Ok(false);
    }
    Ok(std::env::var("MERATECH_LIVE_ASSISTANT").is_ok_and(|v| !v.is_empty()))
}

#[tauri::command]
pub async fn verify_slider_enabled() -> Result<bool, AppError> {
    if !dig_surface_enabled() {
        return Ok(false);
    }
    Ok(std::env::var("MERATECH_VERIFY_SLIDER").is_ok_and(|v| !v.is_empty()))
}

/// DoD item 7: intentional error → typed AppError in the frontend.
#[tauri::command]
pub async fn fail_on_purpose() -> Result<(), AppError> {
    if !dig_surface_enabled() {
        return Err(AppError::InvalidOp("probe disabled".into()));
    }
    Err(AppError::Internal("intentional error probe".into()))
}

/// Webview self-report → engine log. Lets headless test runs verify the
/// frontend booted and the frame:// path worked, by grepping dev output.
#[tauri::command]
pub async fn report_frontend_status(status: String) -> Result<(), AppError> {
    if !dig_surface_enabled() {
        return Ok(());
    }
    let status = status.chars().take(512).collect::<String>();
    tracing::info!(status = %status, "FRONTEND-REPORT");
    Ok(())
}

/// Reveal an exported file in Finder (macOS) or the system file manager.
#[tauri::command]
pub async fn reveal_in_finder(path: String) -> Result<(), AppError> {
    let p = crate::paths::validate_existing_path(&path)?;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let revealed = p.to_string_lossy().into_owned();
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &revealed])
            .spawn()
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args(["/select,", &revealed])
            .spawn()
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(parent) = p.parent() {
            std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| AppError::Internal(e.to_string()))?;
        }
    }
    Ok(())
}

/// Where beta problem reports / creator messages are sent.
const FEEDBACK_EMAIL: &str = "kaimaimeratech@gmail.com";

/// Percent-encode for a mailto query (RFC 3986 unreserved set kept as-is).
fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// "Report a problem / talk to the creator": opens the user's mail client with
/// a pre-filled message to the creator. No server, no third-party service.
#[tauri::command]
pub async fn report_problem(message: String, from: Option<String>) -> Result<(), AppError> {
    let message = message.trim();
    if message.is_empty() {
        return Err(AppError::Internal("Message is empty.".into()));
    }
    let body = match from.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(f) => format!("From: {f}\n\n{message}"),
        None => message.to_string(),
    };
    let url = format!(
        "mailto:{FEEDBACK_EMAIL}?subject={}&body={}",
        pct_encode("MeraRAW beta — problem report"),
        pct_encode(&body),
    );
    crate::open_url::open_mailto_url(&url)
}
