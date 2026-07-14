#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assistant;
mod commands;
mod error;
mod events;
mod license;
mod menu;
mod paths;
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
use tauri::window::Monitor;
use tracing_subscriber::EnvFilter;

/// Keep the main window fully usable on the current display.
///
/// `tauri-plugin-window-state` can restore a size/position from a larger or
/// disconnected monitor. A top-left-on-screen check is not enough — the right
/// ~300px Develop rail can sit past the physical edge while the window still
/// looks "launched". Clamp size + origin into the chosen monitor's work area.
fn ensure_main_window_visible(win: &tauri::WebviewWindow) {
    let _ = win.unminimize();
    let _ = win.show();

    let Ok(pos) = win.outer_position() else {
        return;
    };
    let Ok(size) = win.outer_size() else {
        return;
    };
    let Ok(monitors) = win.available_monitors() else {
        return;
    };
    if monitors.is_empty() {
        return;
    }

    let center_x = pos.x.saturating_add(size.width as i32 / 2);
    let center_y = pos.y.saturating_add(size.height as i32 / 2);

    let monitor: Monitor = monitors
        .iter()
        .find(|m| {
            let wa = m.work_area();
            center_x >= wa.position.x
                && center_y >= wa.position.y
                && center_x < wa.position.x + wa.size.width as i32
                && center_y < wa.position.y + wa.size.height as i32
        })
        .cloned()
        .or_else(|| {
            monitors.iter().find(|m| {
                let mp = m.position();
                let ms = m.size();
                pos.x >= mp.x - 64
                    && pos.y >= mp.y - 64
                    && pos.x < mp.x + ms.width as i32
                    && pos.y < mp.y + ms.height as i32
            }).cloned()
        })
        .or_else(|| win.primary_monitor().ok().flatten())
        .unwrap_or_else(|| monitors[0].clone());

    let wa = monitor.work_area();
    // Leave a tiny margin so the frame isn't flush against the dock/menu bar.
    let margin = 8i32;
    let max_w = wa.size.width.saturating_sub(margin as u32 * 2).max(640);
    let max_h = wa.size.height.saturating_sub(margin as u32 * 2).max(480);

    let new_w = size.width.min(max_w);
    let new_h = size.height.min(max_h);

    let min_x = wa.position.x + margin;
    let min_y = wa.position.y + margin;
    let max_x = wa.position.x + wa.size.width as i32 - new_w as i32 - margin;
    let max_y = wa.position.y + wa.size.height as i32 - new_h as i32 - margin;

    let new_x = pos.x.clamp(min_x, max_x.max(min_x));
    let new_y = pos.y.clamp(min_y, max_y.max(min_y));

    let size_changed = new_w != size.width || new_h != size.height;
    let pos_changed = new_x != pos.x || new_y != pos.y;

    if size_changed || pos_changed {
        tracing::info!(
            old_w = size.width,
            old_h = size.height,
            old_x = pos.x,
            old_y = pos.y,
            new_w,
            new_h,
            new_x,
            new_y,
            monitor = ?monitor.name(),
            "clamping main window into monitor work area"
        );
    }

    if size_changed {
        let _ = win.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(new_w, new_h)));
    }
    if pos_changed {
        let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            new_x, new_y,
        )));
    }

    let _ = win.set_focus();
}

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
        // Some Wayland stacks still crash WebKit unless forced to X11 GDK backend.
        if std::env::var_os("GDK_BACKEND").is_none()
            && std::env::var_os("WAYLAND_DISPLAY").is_some()
        {
            std::env::set_var("GDK_BACKEND", "x11");
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Ensure child processes (sidecar workers) resolve relative to the app.
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let _ = std::env::set_current_dir(dir);
            }
        }
    }

    load_dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,meratech_core=debug,meratech_editor=debug")),
        )
        .init();

    // Selftest runs are hermetic: a fresh per-process data dir keeps the
    // import/thumb steps deterministic and the user's real catalog untouched.
    // An explicit MERATECH_DATA_DIR still wins.
    if std::env::var("MERATECH_SELFTEST").is_ok_and(|v| !v.is_empty())
        && std::env::var("MERATECH_DATA_DIR").map_or(true, |v| v.is_empty())
    {
        let dir = std::env::temp_dir().join(format!("meratech-selftest-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("MERATECH_DATA_DIR", &dir);
        tracing::info!(dir = %dir.display(), "selftest: hermetic data dir");
    }

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
            commands::export_batch,
            commands::cancel_export_batch,
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
            #[cfg(debug_assertions)]
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

            // Window-state restore can leave the frame oversized / hanging off
            // the right edge (common when moving from a large display to a
            // 13" MacBook). Clamp into the current monitor work area.
            if let Some(win) = app.get_webview_window("main") {
                ensure_main_window_visible(&win);
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
                        let adapter = s.adapter.clone();
                        let gpu = s.gpu_ready;
                        events::emit_engine_ready(&handle, adapter.clone(), gpu);
                        // Re-broadcast shortly after so late webview listeners catch it.
                        let handle2 = handle.clone();
                        tauri::async_runtime::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            events::emit_engine_ready(&handle2, adapter.clone(), gpu);
                            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                            events::emit_engine_ready(&handle2, adapter, gpu);
                        });
                    }
                    Err(e) => tracing::error!(error = %e, "engine never came up"),
                }
            });
            Ok(())
        })
        .on_page_load(|window, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                let app = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    let engine = app.state::<meratech_core::engine::EngineHandle>();
                    if let Ok(s) = engine.ping().await {
                        if s.alive {
                            events::emit_engine_ready(&app, s.adapter, s.gpu_ready);
                        }
                    }
                });
            }
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("MeraRAW failed to start: {e}");
            eprintln!(
                "On Linux install webkit2gtk4.1; on Windows install the WebView2 runtime."
            );
            std::process::exit(1);
        });
}
