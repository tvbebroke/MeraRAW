#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assistant;
mod commands;
mod error;
mod events;
mod license;
mod menu;
mod protocol;

// Reference RAW-pipeline scaffold (mirrors the documented stage layout). The
// live, GPU-accelerated pipeline runs in the `meratech-core` crate; each module
// below points at its real counterpart. See `pipeline.rs`.
mod color;
mod export;
mod gpu;
mod grading;
mod isp;
mod pipeline;
mod raw;
mod tone;

use tauri::Manager;
use tracing_subscriber::EnvFilter;

/// Load KEY=VALUE lines from a `.env` next to the project (dev cwd) or via
/// MERATECH_ENV_FILE. Real process env always wins; secrets never logged.
fn load_dotenv() {
    let candidates = [
        std::env::var("MERATECH_ENV_FILE").ok(),
        Some(".env".to_string()),
        Some("../.env".to_string()),
    ];
    for path in candidates.into_iter().flatten() {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let (k, v) = (k.trim(), v.trim().trim_matches('"'));
                if !k.is_empty() && !v.is_empty() && std::env::var_os(k).is_none() {
                    std::env::set_var(k, v);
                }
            }
        }
        tracing::debug!(path, "loaded .env");
        break;
    }
}

fn main() {
    // WebKitGTK's DMABUF / accelerated-compositing renderer aborts on many
    // Linux GPU stacks — cross-distro AppImages, Nvidia, and newer Mesa — which
    // shows up as a WebKitWebProcess SIGABRT before the window even draws. Force
    // the compatible path unless the user overrode it. This only affects the UI
    // webview; image processing runs on wgpu and is untouched.
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        if std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }

    load_dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,meratech_core=debug,meratech_editor=debug")),
        )
        .init();

    // Engine actor up before the window — owns GPU + heavy state.
    let profiles_dir = std::env::var("MERARAW_PROFILES_DIR")
        .map(std::path::PathBuf::from)
        .ok()
        .filter(|p| p.is_dir())
        .or_else(|| {
            let dev = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../meraraw-derivatives");
            dev.is_dir().then_some(dev)
        });
    if let Some(dir) = profiles_dir {
        meratech_core::profile::set_profiles_dir(dir);
        tracing::info!(dir = %meratech_core::profile::profiles_dir().display(), "camera profiles dir");
    } else {
        tracing::warn!("meraraw-derivatives not found; camera profiles disabled");
    }
    let (event_tx, mut event_rx) =
        tokio::sync::mpsc::unbounded_channel::<meratech_core::message::EngineEvent>();
    let engine = meratech_core::engine::spawn_with_events(Some(event_tx));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(engine)
        .register_asynchronous_uri_scheme_protocol("frame", protocol::handle_frame_request)
        .register_asynchronous_uri_scheme_protocol("thumb", protocol::handle_thumb_request)
        .menu(|app| menu::build_menu(app))
        .on_menu_event(|app, event| menu::handle_menu_event(app, event))
        .on_window_event(|window, event| {
            // flush unsaved sidecar edits before the window dies
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let engine = window
                    .app_handle()
                    .state::<meratech_core::engine::EngineHandle>();
                let _ = tauri::async_runtime::block_on(engine.flush_sidecar());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::ping_engine,
            commands::pick_file,
            commands::pick_folder,
            commands::read_file_meta,
            commands::list_dir,
            commands::browse_roots,
            commands::open_image,
            commands::request_frame,
            commands::get_metadata,
            commands::close_image,
            commands::apply_op,
            commands::undo,
            commands::redo,
            commands::get_doc,
            commands::get_history,
            commands::get_registry,
            commands::snapshot,
            commands::list_snapshots,
            commands::restore_snapshot,
            commands::virtual_copy,
            commands::switch_doc,
            commands::save_preset,
            commands::export_image,
            commands::list_presets,
            commands::list_preset_catalog,
            commands::apply_preset,
            commands::get_perf_stats,
            commands::get_stats,
            commands::wb_from_point,
            commands::set_mask_overlay,
            commands::set_preview_bypass,
            commands::set_display_look,
            commands::import_folder,
            commands::scan_import_folder,
            commands::import_selected,
            commands::get_asset_detail,
            commands::list_albums,
            commands::create_album,
            commands::delete_album,
            commands::add_to_album,
            commands::remove_from_album,
            commands::set_camera_profile,
            commands::set_lut,
            commands::set_demosaic,
            commands::pick_lut,
            commands::get_grid,
            commands::list_folders,
            commands::set_asset_meta,
            commands::rebuild_index,
            assistant::assistant_available,
            assistant::assistant_send,
            commands::report_problem,
            commands::autoopen_path,
            commands::selftest_enabled,
            commands::live_assistant_enabled,
            commands::verify_slider_enabled,
            commands::fail_on_purpose,
            commands::report_frontend_status,
            commands::reveal_in_finder,
            license::license_check_local,
            license::license_save_token,
            license::license_clear_token,
            license::license_verify_token_locally,
            license::license_sign_in_and_activate,
            license::license_supporter_status,
            license::license_start_checkout,
            license::open_external_url,
        ])
        .setup(move |app| {
            if std::env::var("MERATECH_BUNDLED_PRESETS").is_err() {
                if let Ok(res) = app.path().resolve(
                    "presets/bundled",
                    tauri::path::BaseDirectory::Resource,
                ) {
                    if res.is_dir() {
                        std::env::set_var("MERATECH_BUNDLED_PRESETS", &res);
                        tracing::info!(dir = %res.display(), "bundled presets dir");
                    }
                }
            }

            // The window-state plugin can restore a position on a monitor
            // that's since been disconnected, leaving the window invisible.
            // If the restored frame isn't on any current display, recenter.
            if let Some(win) = app.get_webview_window("main") {
                // always bring it back into a usable state
                let _ = win.unminimize();
                let _ = win.show();
                let on_screen = (|| {
                    let pos = win.outer_position().ok()?;
                    let monitors = win.available_monitors().ok()?;
                    Some(monitors.iter().any(|m| {
                        let mp = m.position();
                        let ms = m.size();
                        pos.x >= mp.x - 64
                            && pos.y >= mp.y - 64
                            && pos.x < mp.x + ms.width as i32 - 64
                            && pos.y < mp.y + ms.height as i32 - 64
                    }))
                })()
                .unwrap_or(false);
                if !on_screen {
                    tracing::info!("restored window off-screen; recentering");
                    let _ = win.center();
                }
                let _ = win.set_focus();
            }

            // pump engine events → webview
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(ev) = event_rx.recv().await {
                    events::forward_engine_event(&handle, ev);
                }
            });

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // first ping resolves after engine init (incl. GPU attempt)
                let engine = handle.state::<meratech_core::engine::EngineHandle>();
                match engine.ping().await {
                    Ok(s) => {
                        tracing::info!(gpu = s.gpu_ready, adapter = ?s.adapter, "ENGINE-READY");
                        events::emit_engine_ready(&handle, s.adapter, s.gpu_ready);
                    }
                    Err(e) => tracing::error!(error = %e, "engine never came up"),
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
