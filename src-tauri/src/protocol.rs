//! frame:// custom URI protocol — the GPU-texture→canvas transport seam
//! (contract C5). Webview fetches frame://localhost/current?v={version};
//! we return raw RGBA bytes + dimension headers. P0 serves the engine's
//! test frame; Phase 1/2 swap in real render output.

use meratech_core::engine::EngineHandle;
use tauri::{http, Manager, Runtime, UriSchemeContext, UriSchemeResponder};

const TEST_FRAME_W: u32 = 960;
const TEST_FRAME_H: u32 = 600;

/// thumb://localhost/<asset_id>?tier=t|p → preview JPEG bytes (contract C5
/// sibling for the P5 grid).
pub fn handle_thumb_request<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    let uri = request.uri().clone();
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
            Some(bytes) => http::Response::builder()
                .status(200)
                .header("Content-Type", "image/jpeg")
                .header("Cache-Control", "max-age=60")
                .header("Access-Control-Allow-Origin", "*")
                .body(bytes),
            None => http::Response::builder()
                .status(404)
                .header("Access-Control-Allow-Origin", "*")
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
                    match meratech_core::image::rgba8_to_jpeg(
                        &frame.rgba,
                        frame.width,
                        frame.height,
                        88,
                    ) {
                        Ok(bytes) => http::Response::builder()
                            .status(200)
                            .header("Content-Type", "image/jpeg")
                            .header("X-Frame-Width", frame.width.to_string())
                            .header("X-Frame-Height", frame.height.to_string())
                            .header("X-Frame-Version", frame.version.to_string())
                            .header("Cache-Control", "no-store")
                            .header("Access-Control-Allow-Origin", "*")
                            .body(bytes),
                        Err(e) => {
                            tracing::error!(error = %e, "frame jpeg encode failed");
                            http::Response::builder()
                                .status(500)
                                .header("Access-Control-Allow-Origin", "*")
                                .body(e.to_string().into_bytes())
                        }
                    }
                } else {
                    http::Response::builder()
                        .status(200)
                        .header("Content-Type", "application/octet-stream")
                        .header("X-Frame-Width", frame.width.to_string())
                        .header("X-Frame-Height", frame.height.to_string())
                        .header("X-Frame-Version", frame.version.to_string())
                        .header("Cache-Control", "no-store")
                        .header("Access-Control-Allow-Origin", "*")
                        .header(
                            "Access-Control-Expose-Headers",
                            "X-Frame-Width, X-Frame-Height, X-Frame-Version",
                        )
                        .body(frame.rgba)
                };
                if let Ok(resp) = resp {
                    responder.respond(resp);
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "frame render failed");
                let resp = http::Response::builder()
                    .status(500)
                    .header("Access-Control-Allow-Origin", "*")
                    .body(e.to_string().into_bytes())
                    .expect("error response build");
                responder.respond(resp);
            }
        }
    });
}
