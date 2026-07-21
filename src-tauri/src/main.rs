#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assistant;
mod commands;
mod error;
mod events;
mod license;
mod menu;
mod open_url;
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

/// A window or work-area rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WinRect {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

/// Leave a small margin so the frame isn't flush against the dock/menu bar.
const WINDOW_MARGIN: i32 = 8;

/// What to do with the main window once its target monitor is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FitAction {
    /// The saved size is usable — keep it, only nudge the origin on screen.
    Reposition(WinRect),
    /// No size worth restoring — fill the target display instead.
    Maximize,
}

/// Decide what to do with a window being restored onto `work`.
///
/// A saved size is only honoured when it could actually have come from the
/// user. Everything else opens maximized:
///
///  * No saved state (first run) — nothing to honour.
///  * Bigger than the display. Saved on a larger monitor, or a runaway value.
///    Real ones seen here: 8304x2040, then 4152x1020 — the size halves or
///    doubles each save/restore cycle as the window crosses between displays
///    with different scale factors. Shrinking such a value to fit "works" but
///    leaves the window sized from junk.
///  * Smaller than the app's own configured minimum (`min` here, physical px).
///    The OS enforces that minimum interactively, so the user cannot have
///    produced it by dragging — it is the same runaway sequence after enough
///    halvings (2076x700 against a 2048x1400 minimum). Without this check the
///    junk eventually shrinks into the "fits" range and gets treated as a
///    deliberate choice.
///
/// Anything left is a size the user plausibly chose: keep it, and only nudge
/// the origin so the whole frame is on screen.
///
/// All units are physical pixels, matching `outer_size` / `work_area`.
fn fit_into_work_area(
    work: WinRect,
    cur: WinRect,
    min: (u32, u32),
    has_saved_state: bool,
) -> FitAction {
    if !has_saved_state {
        return FitAction::Maximize;
    }

    let margin = WINDOW_MARGIN;
    let avail_w = work.w.saturating_sub(margin as u32 * 2).max(640);
    let avail_h = work.h.saturating_sub(margin as u32 * 2).max(480);

    if cur.w > avail_w || cur.h > avail_h {
        return FitAction::Maximize;
    }

    // Only trust a below-minimum size if the display genuinely can't fit the
    // minimum, in which case the OS legitimately shrank the window.
    if (cur.w < min.0 && min.0 <= avail_w) || (cur.h < min.1 && min.1 <= avail_h) {
        return FitAction::Maximize;
    }

    let min_x = work.x + margin;
    let min_y = work.y + margin;
    let max_x = work.x + work.w as i32 - cur.w as i32 - margin;
    let max_y = work.y + work.h as i32 - cur.h as i32 - margin;

    FitAction::Reposition(WinRect {
        x: cur.x.clamp(min_x, max_x.max(min_x)),
        y: cur.y.clamp(min_y, max_y.max(min_y)),
        w: cur.w,
        h: cur.h,
    })
}

/// Whether `tauri-plugin-window-state` has a persisted state file to restore
/// from. Absent means a genuine first run, so there is no user size to honour.
fn has_saved_window_state(win: &tauri::WebviewWindow) -> bool {
    win.app_handle()
        .path()
        .app_config_dir()
        .map(|dir| dir.join(tauri_plugin_window_state::DEFAULT_FILENAME).exists())
        .unwrap_or(false)
}

/// The `main` window's configured default and minimum sizes (tauri.conf.json,
/// logical px) converted to physical px for a monitor with `scale_factor`.
/// Falls back to the values checked into tauri.conf.json if config is absent.
fn configured_window_sizes(
    win: &tauri::WebviewWindow,
    scale_factor: f64,
) -> ((u32, u32), (u32, u32)) {
    let cfg = win
        .app_handle()
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .map(|w| {
            (
                w.width,
                w.height,
                w.min_width.unwrap_or(1024.0),
                w.min_height.unwrap_or(700.0),
            )
        })
        .unwrap_or((1280.0, 800.0, 1024.0, 700.0));

    let px = |v: f64| (v * scale_factor).round().max(1.0) as u32;
    ((px(cfg.0), px(cfg.1)), (px(cfg.2), px(cfg.3)))
}

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
    let cur = WinRect {
        x: pos.x,
        y: pos.y,
        w: size.width,
        h: size.height,
    };
    let work = WinRect {
        x: wa.position.x,
        y: wa.position.y,
        w: wa.size.width,
        h: wa.size.height,
    };

    let (default_size, min_size) = configured_window_sizes(win, monitor.scale_factor());

    match fit_into_work_area(work, cur, min_size, has_saved_window_state(win)) {
        FitAction::Reposition(fitted) => {
            let size_changed = fitted.w != size.width || fitted.h != size.height;
            let pos_changed = fitted.x != pos.x || fitted.y != pos.y;

            if size_changed || pos_changed {
                tracing::info!(
                    old_w = size.width,
                    old_h = size.height,
                    old_x = pos.x,
                    old_y = pos.y,
                    new_w = fitted.w,
                    new_h = fitted.h,
                    new_x = fitted.x,
                    new_y = fitted.y,
                    monitor = ?monitor.name(),
                    "nudging main window back into the monitor work area"
                );
            }

            if size_changed {
                let _ = win.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                    fitted.w, fitted.h,
                )));
            }
            if pos_changed {
                let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                    fitted.x, fitted.y,
                )));
            }
        }
        FitAction::Maximize if win.is_maximized().unwrap_or(false) => {
            // Already zoomed — the plugin restored a maximized window, whose
            // outer_size legitimately exceeds the margin-inset work area.
            // Re-parking it would un-maximize and visibly flicker every launch.
            tracing::debug!("main window restored already maximized");
        }
        FitAction::Maximize => {
            tracing::info!(
                old_w = size.width,
                old_h = size.height,
                monitor = ?monitor.name(),
                "no usable saved window size — opening maximized"
            );

            // macOS zooms to whichever screen the window currently sits on, so
            // park it on the target monitor at a modest size first. Without
            // this, a window restored at junk coordinates maximizes onto the
            // wrong display.
            let w = default_size.0.min(work.w);
            let h = default_size.1.min(work.h);
            let _ = win.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(w, h)));
            let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                work.x + (work.w as i32 - w as i32) / 2,
                work.y + (work.h as i32 - h as i32) / 2,
            )));

            // Native zoom rather than set_size(work_area): it keeps the menu
            // bar and traffic lights, records a real maximized state for the
            // window-state plugin, and leaves the green button behaving
            // normally. Deliberately not set_fullscreen — that would put
            // MeraRAW in its own Space.
            let _ = win.maximize();
        }
    }

    let _ = win.set_focus();
}

/// Load KEY=VALUE lines from a `.env` next to the project (dev cwd) or via
/// MERATECH_ENV_FILE. Real process env always wins; secrets never logged.
/// Debug builds only — packaged releases must not pick up a cwd `.env`.
#[cfg(debug_assertions)]
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

#[cfg(not(debug_assertions))]
fn load_dotenv() {}

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
        // Never restore/save decorations — the Svelte TitleBar draws its own
        // traffic lights (`decorations: false` in tauri.conf.json). Persisting
        // `decorated: true` from an older session re-enables the native title
        // bar and shows a second set of controls ("seeing double").
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::all()
                        - tauri_plugin_window_state::StateFlags::DECORATIONS,
                )
                .build(),
        )
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
            commands::denoise_estimate_profile,
            commands::denoise_models_list,
            commands::denoise_ai_start,
            commands::denoise_ai_cancel,
            commands::denoise_ai_reset,
            commands::get_stats,
            commands::wb_from_point,
            commands::auto_level,
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
            commands::reveal_in_finder,
            license::license_check_local,
            license::license_clear_token,
            license::license_verify_token_locally,
            license::license_sign_in_and_activate,
            license::license_supporter_status,
            license::license_start_checkout,
            license::open_external_url,
            // Dig / selftest probes — ACL-gated to allow-dev-probes (debug capability).
            commands::autoopen_path,
            commands::selftest_enabled,
            commands::live_assistant_enabled,
            commands::verify_slider_enabled,
            commands::report_frontend_status,
            #[cfg(debug_assertions)]
            commands::fail_on_purpose,
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
            // Also pin decorations off so a stale `.window-state.json` cannot
            // resurrect the native title bar beside the custom traffic lights.
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_decorations(false);
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

#[cfg(test)]
mod window_geometry_tests {
    use super::*;

    /// Built-in Liquid Retina XDR: 3456x2234 physical, 2x, menu bar removed.
    fn builtin_work_area() -> WinRect {
        WinRect { x: 0, y: 50, w: 3456, h: 2184 }
    }

    /// External C27F390: 1920x1080, 1x, placed to the right of the built-in.
    fn external_work_area() -> WinRect {
        WinRect { x: 3456, y: 25, w: 1920, h: 1055 }
    }

    /// minWidth 1024 / minHeight 700 logical, at 2x and 1x.
    const MIN_2X: (u32, u32) = (2048, 1400);
    const MIN_1X: (u32, u32) = (1024, 700);

    #[test]
    fn first_run_opens_maximized() {
        let cur = WinRect { x: 0, y: 0, w: 2560, h: 1600 };
        assert_eq!(
            fit_into_work_area(builtin_work_area(), cur, MIN_2X, false),
            FitAction::Maximize,
            "with no saved state the size is not the user's choice"
        );
    }

    #[test]
    fn runaway_saved_size_maximizes_instead_of_shrinking_to_junk() {
        // The first real state found on disk: 8304x2040.
        let cur = WinRect { x: 286, y: 194, w: 8304, h: 2040 };
        assert_eq!(
            fit_into_work_area(builtin_work_area(), cur, MIN_2X, true),
            FitAction::Maximize
        );
    }

    #[test]
    fn halved_runaway_that_fits_is_still_rejected() {
        // 2076x700 — the runaway after two halvings. It fits the display, so
        // the fits-check alone accepted it and the app opened squat instead of
        // maximized. Its height is half the 1400px minimum, which is the tell.
        let cur = WinRect { x: 2164, y: 115, w: 2076, h: 700 };
        assert_eq!(
            fit_into_work_area(builtin_work_area(), cur, MIN_2X, true),
            FitAction::Maximize,
            "a size below the app's own minimum cannot have come from the user"
        );
    }

    #[test]
    fn oversize_on_the_1x_external_also_maximizes() {
        let cur = WinRect { x: 3500, y: 100, w: 3440, h: 2040 };
        assert_eq!(
            fit_into_work_area(external_work_area(), cur, MIN_1X, true),
            FitAction::Maximize
        );
    }

    #[test]
    fn a_saved_size_that_fits_is_left_alone() {
        // The whole point of "remember my size" — this must never be resized.
        let cur = WinRect { x: 100, y: 100, w: 3000, h: 1900 };
        assert_eq!(
            fit_into_work_area(builtin_work_area(), cur, MIN_2X, true),
            FitAction::Reposition(WinRect { x: 100, y: 100, w: 3000, h: 1900 })
        );
    }

    #[test]
    fn a_size_at_the_minimum_is_left_alone() {
        // Deliberately small-but-legal windows must not be "helpfully" maximized.
        let cur = WinRect { x: 200, y: 200, w: 2048, h: 1400 };
        assert_eq!(
            fit_into_work_area(builtin_work_area(), cur, MIN_2X, true),
            FitAction::Reposition(WinRect { x: 200, y: 200, w: 2048, h: 1400 })
        );
    }

    #[test]
    fn below_minimum_is_kept_when_the_display_cannot_fit_the_minimum() {
        // Tiny display: the OS legitimately shrank the window below minHeight,
        // so that is not evidence of corruption.
        let work = WinRect { x: 0, y: 0, w: 1200, h: 800 };
        let cur = WinRect { x: 10, y: 10, w: 1100, h: 600 };
        assert_eq!(
            fit_into_work_area(work, cur, MIN_2X, true),
            FitAction::Reposition(WinRect { x: 10, y: 10, w: 1100, h: 600 })
        );
    }

    #[test]
    fn offscreen_but_fitting_window_is_pulled_back_without_resizing() {
        // Hanging off the right edge — the original bug this function existed for.
        let cur = WinRect { x: 3300, y: 194, w: 2560, h: 1600 };
        let got = fit_into_work_area(builtin_work_area(), cur, MIN_2X, true);
        let FitAction::Reposition(r) = got else {
            panic!("a fitting size must be repositioned, not maximized: {got:?}");
        };
        assert_eq!((r.w, r.h), (2560, 1600), "position fix must not resize");
        assert_eq!(r.x, 3456 - 2560 - WINDOW_MARGIN);
        let wa = builtin_work_area();
        assert!(r.x >= wa.x && r.y >= wa.y);
        assert!(r.x + r.w as i32 <= wa.x + wa.w as i32);
        assert!(r.y + r.h as i32 <= wa.y + wa.h as i32);
    }

    #[test]
    fn tiny_work_area_does_not_underflow() {
        let work = WinRect { x: 0, y: 0, w: 300, h: 200 };
        let cur = WinRect { x: 0, y: 0, w: 8304, h: 2040 };
        assert_eq!(fit_into_work_area(work, cur, MIN_2X, true), FitAction::Maximize);
    }
}
