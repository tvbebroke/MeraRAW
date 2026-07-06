//! Native menu. File→Open/Open Folder fully functional in P0; the rest
//! wired-but-stub (disabled items enable as phases land).

use crate::events;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_dialog::DialogExt;

pub fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let app_menu = SubmenuBuilder::new(app, "MeraRAW")
        .about(None)
        .separator()
        .item(&MenuItem::with_id(
            app,
            "settings",
            "Settings…",
            true,
            Some("Cmd+,"),
        )?)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let file = SubmenuBuilder::new(app, "File")
        .item(&MenuItem::with_id(
            app,
            "open-file",
            "Open…",
            true,
            Some("CmdOrCtrl+O"),
        )?)
        .item(&MenuItem::with_id(
            app,
            "open-folder",
            "Open Folder…",
            true,
            Some("CmdOrCtrl+Shift+O"),
        )?)
        .item(&MenuItem::with_id(app, "import", "Add Photos…", true, None::<&str>)?)
        .item(&MenuItem::with_id(app, "export", "Export…", true, None::<&str>)?)
        .separator()
        .item(&PredefinedMenuItem::close_window(app, None)?)
        .build()?;

    let edit = SubmenuBuilder::new(app, "Edit")
        .item(&MenuItem::with_id(app, "undo", "Undo", false, Some("CmdOrCtrl+Z"))?)
        .item(&MenuItem::with_id(
            app,
            "redo",
            "Redo",
            false,
            Some("CmdOrCtrl+Shift+Z"),
        )?)
        .separator()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let view = SubmenuBuilder::new(app, "View")
        .item(&MenuItem::with_id(app, "zoom-in", "Zoom In", false, Some("CmdOrCtrl+="))?)
        .item(&MenuItem::with_id(app, "zoom-out", "Zoom Out", false, Some("CmdOrCtrl+-"))?)
        .item(&MenuItem::with_id(app, "zoom-fit", "Fit", false, Some("CmdOrCtrl+0"))?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            "toggle-devtools",
            "Toggle Devtools",
            cfg!(debug_assertions),
            Some("CmdOrCtrl+Alt+I"),
        )?)
        .build()?;

    let help = SubmenuBuilder::new(app, "Help")
        .item(&MenuItem::with_id(app, "docs", "Documentation", true, None::<&str>)?)
        .build()?;

    Menu::with_items(app, &[&app_menu, &file, &edit, &view, &help])
}

pub fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "open-file" => {
            let app = app.clone();
            app.clone()
                .dialog()
                .file()
                .add_filter("Images", crate::commands::IMAGE_EXTENSIONS)
                .pick_file(move |f| {
                    if let Some(path) = f {
                        let path = path.to_string();
                        tracing::info!(path = %path, "menu: open file");
                        events::emit_path_event(&app, events::FILE_OPENED, &path);
                    }
                });
        }
        "open-folder" => {
            let app = app.clone();
            app.clone().dialog().file().pick_folder(move |f| {
                if let Some(path) = f {
                    let path = path.to_string();
                    tracing::info!(path = %path, "menu: open folder");
                    events::emit_path_event(&app, events::FOLDER_OPENED, &path);
                }
            });
        }
        "import" => {
            if let Err(e) = app.emit(events::IMPORT_REQUESTED, ()) {
                tracing::error!(error = %e, "emit import-requested failed");
            }
        }
        "export" => {
            if let Err(e) = app.emit(events::EXPORT_REQUESTED, ()) {
                tracing::error!(error = %e, "emit export-requested failed");
            }
        }
        "settings" => {
            if let Err(e) = app.emit(events::SETTINGS_REQUESTED, ()) {
                tracing::error!(error = %e, "emit settings-requested failed");
            }
        }
        "toggle-devtools" => {
            #[cfg(debug_assertions)]
            if let Some(w) = app.get_webview_window("main") {
                if w.is_devtools_open() {
                    w.close_devtools();
                } else {
                    w.open_devtools();
                }
            }
        }
        "docs" => {
            tracing::info!("menu: docs (stub)");
        }
        other => {
            tracing::debug!(id = other, "menu: unhandled (stub)");
        }
    }
}
