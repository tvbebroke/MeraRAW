//! Thin #[tauri::command] handlers. Validate → EngineMsg → await oneshot →
//! Result<T, AppError>. Never do heavy work here (contract C2).

use crate::error::AppError;
use meratech_core::engine::EngineHandle;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
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

pub const IMAGE_EXTENSIONS: &[&str] = &[
    // RAW (rawler)
    "arw", "nef", "nrw", "cr2", "cr3", "crw", "dng", "raf", "orf", "rw2", "pef", "srw", "erf",
    "kdc", "dcs", "dcr", "iiq", "3fr", "mef", "mos",
    // rendered (image crate + jxl-oxide; HEIC via macOS sips; PSD composite)
    "jpg", "jpeg", "png", "tif", "tiff", "webp", "bmp", "gif", "jxl", "heic", "heif", "hif", "psd",
];

#[tauri::command]
pub async fn pick_file(app: AppHandle) -> Result<Option<String>, AppError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Images", IMAGE_EXTENSIONS)
        .pick_file(move |f| {
            let _ = tx.send(f);
        });
    let picked = rx
        .await
        .map_err(|_| AppError::Internal("dialog dropped".into()))?;
    Ok(picked.map(|p| p.to_string()))
}

/// Native picker for a 3D look LUT (`.cube`).
#[tauri::command]
pub async fn pick_lut(app: AppHandle) -> Result<Option<String>, AppError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("3D LUT", &["cube"])
        .pick_file(move |f| {
            let _ = tx.send(f);
        });
    let picked = rx
        .await
        .map_err(|_| AppError::Internal("dialog dropped".into()))?;
    Ok(picked.map(|p| p.to_string()))
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>, AppError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |f| {
        let _ = tx.send(f);
    });
    let picked = rx
        .await
        .map_err(|_| AppError::Internal("dialog dropped".into()))?;
    Ok(picked.map(|p| p.to_string()))
}

#[tauri::command]
pub async fn read_file_meta(path: String) -> Result<FileMeta, AppError> {
    let p = crate::paths::validate_user_path(&path)?;
    let path = p.to_string_lossy().into_owned();
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());
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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseRoot {
    pub name: String,
    pub path: String,
}

#[tauri::command]
pub async fn browse_roots() -> Result<Vec<BrowseRoot>, AppError> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users".into());
    let mut roots = vec![
        BrowseRoot {
            name: "Desktop".into(),
            path: format!("{home}/Desktop"),
        },
        BrowseRoot {
            name: "Documents".into(),
            path: format!("{home}/Documents"),
        },
        BrowseRoot {
            name: "Pictures".into(),
            path: format!("{home}/Pictures"),
        },
        BrowseRoot {
            name: "Downloads".into(),
            path: format!("{home}/Downloads"),
        },
        BrowseRoot {
            name: "Home".into(),
            path: home.clone(),
        },
    ];
    roots.retain(|r| std::path::Path::new(&r.path).exists());

    if let Ok(volumes) = std::fs::read_dir("/Volumes") {
        for entry in volumes.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            roots.push(BrowseRoot {
                name,
                path: path.to_string_lossy().into_owned(),
            });
        }
    }
    Ok(roots)
}

#[tauri::command]
pub async fn list_dir(path: String) -> Result<Vec<DirEntry>, AppError> {
    let path = crate::paths::validate_existing_path(&path)?;
    let mut entries = Vec::new();
    let mut rd = tokio::fs::read_dir(&path).await?;
    while let Some(entry) = rd.next_entry().await? {
        let md = entry.metadata().await?;
        entries.push(DirEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: entry.path().to_string_lossy().into_owned(),
            is_dir: md.is_dir(),
            size: md.len(),
        });
    }
    entries.sort_by(|a, b| (b.is_dir, a.name.to_lowercase()).cmp(&(a.is_dir, b.name.to_lowercase())));
    Ok(entries)
}

#[tauri::command]
pub async fn open_image(
    engine: State<'_, EngineHandle>,
    path: String,
) -> Result<meratech_core::raw::ImageMeta, AppError> {
    let path = crate::paths::validate_existing_path(&path)?;
    engine
        .open_image(path)
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
pub async fn snapshot(
    engine: State<'_, EngineHandle>,
    name: String,
) -> Result<(), AppError> {
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
pub async fn virtual_copy(engine: State<'_, EngineHandle>) -> Result<String, AppError> {
    engine.virtual_copy().await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn switch_doc(
    engine: State<'_, EngineHandle>,
    docId: String,
) -> Result<meratech_core::ops::DocDelta, AppError> {
    engine.switch_doc(docId).await?.map_err(AppError::from)
}

/// Dev/test hook: MERATECH_OPEN=<path> auto-opens a file on launch.
/// Frontend polls this once at boot (deterministic, no event race).
#[tauri::command]
pub async fn autoopen_path() -> Result<Option<String>, AppError> {
    Ok(std::env::var("MERATECH_OPEN").ok().filter(|s| !s.is_empty()))
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

// ---- Phase 7: export + presets + perf ----

#[tauri::command]
pub async fn export_image(
    app: AppHandle,
    engine: State<'_, EngineHandle>,
    mut settings: meratech_core::export::ExportSettings,
) -> Result<String, AppError> {
    if settings.dest_dir.trim().is_empty() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.dialog().file().pick_folder(move |f| {
            let _ = tx.send(f);
        });
        let picked = rx
            .await
            .map_err(|_| AppError::Internal("dialog dropped".into()))?;
        settings.dest_dir = picked
            .map(|p| p.to_string())
            .ok_or_else(|| AppError::InvalidOp("export cancelled".into()))?;
    }
    settings.dest_dir = crate::paths::validate_user_path(&settings.dest_dir)?
        .to_string_lossy()
        .into_owned();
    engine.export_image(settings).await?.map_err(AppError::from)
}

/// Batch export: engine decodes + renders each path off the open image.
/// Returns the accepted queue length; progress arrives as
/// export-batch-progress / export-batch-done events.
#[tauri::command]
pub async fn export_batch(
    app: AppHandle,
    engine: State<'_, EngineHandle>,
    paths: Vec<String>,
    mut settings: meratech_core::export::ExportSettings,
) -> Result<u32, AppError> {
    if settings.dest_dir.trim().is_empty() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.dialog().file().pick_folder(move |f| {
            let _ = tx.send(f);
        });
        let picked = rx
            .await
            .map_err(|_| AppError::Internal("dialog dropped".into()))?;
        settings.dest_dir = picked
            .map(|p| p.to_string())
            .ok_or_else(|| AppError::InvalidOp("export cancelled".into()))?;
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
) -> Result<(), AppError> {
    engine
        .save_preset_to_disk(name, modules)
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

// ---- Phase 5: catalog ----

#[tauri::command]
pub async fn import_folder(
    engine: State<'_, EngineHandle>,
    path: String,
) -> Result<u64, AppError> {
    let path = crate::paths::validate_existing_path(&path)?;
    engine
        .import_folder(path)
        .await?
        .map_err(AppError::from)
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
    let mut validated = Vec::with_capacity(paths.len());
    for p in paths {
        validated.push(crate::paths::validate_existing_path(&p)?);
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
pub async fn create_album(
    engine: State<'_, EngineHandle>,
    name: String,
) -> Result<i64, AppError> {
    engine.create_album(name).await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn delete_album(
    engine: State<'_, EngineHandle>,
    id: i64,
) -> Result<(), AppError> {
    engine.delete_album(id).await?.map_err(AppError::from)
}

#[tauri::command]
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
        Some(p) if !p.trim().is_empty() => {
            Some(crate::paths::validate_existing_path(&p)?.to_string_lossy().into_owned())
        }
        _ => None,
    };
    engine.set_lut(path).await?.map_err(AppError::from)
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
pub async fn rebuild_index(engine: State<'_, EngineHandle>) -> Result<u64, AppError> {
    engine.rebuild_index().await?.map_err(AppError::from)
}

#[tauri::command]
pub async fn set_mask_overlay(
    engine: State<'_, EngineHandle>,
    id: Option<String>,
) -> Result<(), AppError> {
    Ok(engine.set_mask_overlay(id).await?)
}

/// Before/after: render the un-edited base while `on`.
#[tauri::command]
pub async fn set_preview_bypass(
    engine: State<'_, EngineHandle>,
    on: bool,
) -> Result<(), AppError> {
    Ok(engine.set_preview_bypass(on).await?)
}

/// Display look: 0 = Neutral, 1 = Camera, 2 = Filmic (AgX).
#[tauri::command]
pub async fn set_display_look(
    engine: State<'_, EngineHandle>,
    look: u32,
) -> Result<(), AppError> {
    Ok(engine.set_display_look(look).await?)
}

#[tauri::command]
pub async fn selftest_enabled() -> Result<bool, AppError> {
    Ok(std::env::var("MERATECH_SELFTEST").is_ok_and(|v| !v.is_empty()))
}

#[tauri::command]
pub async fn live_assistant_enabled() -> Result<bool, AppError> {
    Ok(std::env::var("MERATECH_LIVE_ASSISTANT").is_ok_and(|v| !v.is_empty()))
}

#[tauri::command]
pub async fn verify_slider_enabled() -> Result<bool, AppError> {
    Ok(std::env::var("MERATECH_VERIFY_SLIDER").is_ok_and(|v| !v.is_empty()))
}

/// DoD item 7: intentional error → typed AppError in the frontend.
#[tauri::command]
pub async fn fail_on_purpose() -> Result<(), AppError> {
    Err(AppError::Internal("intentional error probe".into()))
}

/// Webview self-report → engine log. Lets headless test runs verify the
/// frontend booted and the frame:// path worked, by grepping dev output.
#[tauri::command]
pub async fn report_frontend_status(status: String) -> Result<(), AppError> {
    tracing::info!(status = %status, "FRONTEND-REPORT");
    Ok(())
}

/// Reveal an exported file in Finder (macOS) or the system file manager.
#[tauri::command]
pub async fn reveal_in_finder(path: String) -> Result<(), AppError> {
    let p = crate::paths::validate_existing_path(&path)?;
    let path = p.to_string_lossy().into_owned();
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args(["/select,", &path])
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
    // Hand the mailto URL to the OS default mail handler.
    let spawn = |mut cmd: std::process::Command| {
        cmd.spawn()
            .map(|_| ())
            .map_err(|e| AppError::Internal(format!("open mail client: {e}")))
    };
    #[cfg(target_os = "macos")]
    {
        let mut c = std::process::Command::new("open");
        c.arg(&url);
        spawn(c)?;
    }
    #[cfg(target_os = "windows")]
    {
        // `start` is a cmd builtin; empty "" is the window-title arg.
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", "", &url]);
        spawn(c)?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(&url);
        spawn(c)?;
    }
    Ok(())
}
