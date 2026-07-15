//! frame:// custom URI protocol — the GPU-texture→canvas transport seam
//! (contract C5). Webview fetches frame://localhost/current?v={version};
//! we return raw RGBA bytes + dimension headers. P0 serves the engine's
//! test frame; Phase 1/2 swap in real render output.

use meratech_core::engine::EngineHandle;
use std::sync::{Mutex, OnceLock};
use tauri::{http, Manager, Runtime, UriSchemeContext, UriSchemeResponder};

const TEST_FRAME_W: u32 = 960;
const TEST_FRAME_H: u32 = 600;

/// Last JPEG-encoded frame, keyed by version — so the same render isn't
/// re-encoded on a repeat fetch (startup probe, remount, etc.).
fn jpeg_cache() -> &'static Mutex<Option<(u64, Vec<u8>)>> {
    static C: OnceLock<Mutex<Option<(u64, Vec<u8>)>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

/// Return the cached JPEG for `version`, or encode + cache it now.
fn frame_jpeg(frame: &meratech_core::message::Frame) -> Result<Vec<u8>, String> {
    if let Some((v, bytes)) = jpeg_cache().lock().unwrap().as_ref() {
        if *v == frame.version {
            return Ok(bytes.clone());
        }
    }
    let bytes = meratech_core::image::rgba8_to_jpeg(&frame.rgba, frame.width, frame.height, 88)
        .map_err(|e| e.to_string())?;
    *jpeg_cache().lock().unwrap() = Some((frame.version, bytes.clone()));
    Ok(bytes)
}

/// Webview pages load from `http://127.0.0.1:1420` (dev) / `tauri://` (prod).
/// `fetch(frame://…)` is cross-origin and needs ACAO to read the body —
/// reflect only known app origins (never `*`).
fn allowed_origin(origin_header: Option<&str>) -> &'static str {
    match origin_header.unwrap_or("") {
        "http://127.0.0.1:1420" => "http://127.0.0.1:1420",
        "http://localhost:1420" => "http://localhost:1420",
        "tauri://localhost" => "tauri://localhost",
        "https://tauri.localhost" => "https://tauri.localhost",
        "http://tauri.localhost" => "http://tauri.localhost",
        // Fallback for builds that omit Origin on custom-scheme fetches
        _ => "tauri://localhost",
    }
}

fn request_origin(request: &http::Request<Vec<u8>>) -> &'static str {
    let hdr = request
        .headers()
        .get(http::header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    allowed_origin(hdr)
}

fn cors_headers(
    builder: http::response::Builder,
    origin: &'static str,
) -> http::response::Builder {
    builder
        .header("Access-Control-Allow-Origin", origin)
        .header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS")
        .header("Access-Control-Allow-Headers", "Content-Type")
        .header("Vary", "Origin")
        .header(
            "Access-Control-Expose-Headers",
            "X-Frame-Width, X-Frame-Height, X-Frame-Version",
        )
}

/// thumb://localhost/<asset_id>?tier=t|p → preview JPEG bytes (contract C5
/// sibling for the P5 grid).
pub fn handle_thumb_request<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    let uri = request.uri().clone();
    let origin = request_origin(&request);
    tauri::async_runtime::spawn(async move {
        let id: i64 = uri
            .path()
            .trim_start_matches('/')
            .parse()
            .unwrap_or(-1);
        let tier = uri
            .query()
            .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("tier=")))
            .unwrap_or("t")
            .to_string();
        let engine = app.state::<EngineHandle>();
        let file = match engine.get_preview_file(id, tier).await {
            Ok(Some(p)) => tokio::fs::read(p).await.ok(),
            _ => None,
        };
        let resp = match file {
            Some(bytes) => cors_headers(http::Response::builder(), origin)
                .status(200)
                .header("Content-Type", "image/jpeg")
                .header("Cache-Control", "max-age=60")
                .body(bytes),
            None => cors_headers(http::Response::builder(), origin)
                .status(404)
                .body(Vec::new()),
        };
        if let Ok(resp) = resp {
            responder.respond(resp);
        }
    });
}

pub fn handle_frame_request<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let origin = request_origin(&request);
    // Preflight for cross-origin fetch from the Vite/dev or tauri origin.
    if request.method() == http::Method::OPTIONS {
        let resp = cors_headers(http::Response::builder(), origin)
            .status(204)
            .body(Vec::new())
            .expect("options response build");
        responder.respond(resp);
        return;
    }

    let app = ctx.app_handle().clone();
    let uri = request.uri().to_string();
    tauri::async_runtime::spawn(async move {
        tracing::debug!(uri = %uri, "frame request");
        let engine = app.state::<EngineHandle>();
        // latest real frame (image/preview); fall back to the test pattern
        let frame = match engine.get_frame().await {
            Ok(Some(f)) => Ok(f),
            Ok(None) => engine.test_frame(TEST_FRAME_W, TEST_FRAME_H).await,
            Err(e) => Err(e),
        };

        match frame {
            Ok(frame) => {
                let resp = if uri.contains("fmt=jpeg") {
                    match frame_jpeg(&frame) {
                        Ok(bytes) => cors_headers(http::Response::builder(), origin)
                            .status(200)
                            .header("Content-Type", "image/jpeg")
                            .header("X-Frame-Width", frame.width.to_string())
                            .header("X-Frame-Height", frame.height.to_string())
                            .header("X-Frame-Version", frame.version.to_string())
                            .header("Cache-Control", "no-store")
                            .body(bytes),
                        Err(e) => {
                            tracing::error!(error = %e, "frame jpeg encode failed");
                            cors_headers(http::Response::builder(), origin)
                                .status(500)
                                .body(Vec::new())
                        }
                    }
                } else {
                    cors_headers(http::Response::builder(), origin)
                        .status(200)
                        .header("Content-Type", "application/octet-stream")
                        .header("X-Frame-Width", frame.width.to_string())
                        .header("X-Frame-Height", frame.height.to_string())
                        .header("X-Frame-Version", frame.version.to_string())
                        .header("Cache-Control", "no-store")
                        .body(frame.rgba)
                };
                if let Ok(resp) = resp {
                    responder.respond(resp);
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "frame render failed");
                let resp = cors_headers(http::Response::builder(), origin)
                    .status(500)
                    .body(Vec::new())
                    .expect("error response build");
                responder.respond(resp);
            }
        }
    });
}
