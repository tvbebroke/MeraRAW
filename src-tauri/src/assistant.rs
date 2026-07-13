//! Phase 6 — Claude as editing agent (the spirit door). Mostly wiring:
//! every tool is a thin skin over ops that already exist, schemas derived
//! from the param registry, every call through the same P2 guard-wall.
//! Loop: look → move → look-again → self-correct → explain (spec 6.6).
//!
//! Privacy (6.7): what leaves = small preview JPEG + edit doc + stats +
//! message. The full-res RAW never crosses the wire — there is no tool
//! that could send it.

use crate::error::AppError;
use crate::events;
use base64::Engine as _;
use meratech_core::engine::EngineHandle;
use meratech_core::ops::Op;
use meratech_core::registry;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MAX_LOOP_TURNS: usize = 8;
const PREVIEW_DIM: u32 = 576; // smaller = far fewer image tokens per turn
const MAX_TOKENS: u32 = 1024;

fn model() -> String {
    std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-sonnet-4-6".into())
}

fn api_key() -> Option<String> {
    std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.is_empty())
}

fn mock_mode() -> bool {
    std::env::var("MERATECH_ASSISTANT_MOCK").is_ok_and(|v| !v.is_empty())
}

pub fn available() -> bool {
    api_key().is_some() || mock_mode()
}

/// Compact registry digest grouped by module (contract A2 — every P3 param
/// is auto-addressable). Grouping keeps the per-turn tool schema small.
fn registry_digest() -> String {
    use std::collections::BTreeMap;
    let mut by_mod: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for spec in registry::all_specs() {
        if !matches!(spec.ty, registry::ParamType::F32) {
            continue;
        }
        let (module, param) = spec.path.split_once('.').unwrap_or(("", spec.path));
        by_mod
            .entry(module)
            .or_default()
            .push(format!("{param}[{:.0}..{:.0}]", spec.min, spec.max));
    }
    by_mod
        .into_iter()
        .map(|(m, ps)| format!("{m}: {}", ps.join(",")))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn tool_defs() -> Value {
    json!([
      {
        "name": "set_params",
        "description": format!(
          "Set develop params. sets=[{{path,value}}]. Paths: {}. Mask-scoped: \
           'mask.<id>.<module>.<param>'. Clamped to range. Batch related changes in one call.",
          registry_digest()),
        "input_schema": { "type": "object", "properties": {
          "sets": { "type": "array", "items": { "type": "object",
            "properties": { "path": {"type": "string"}, "value": {"type": "number"} },
            "required": ["path", "value"] } } }, "required": ["sets"] }
      },
      {
        "name": "set_tone_curve",
        "description": "Tone curve points [[x,y],...], x increasing in [0,1]. Empty=identity. Film-like, hue-preserving.",
        "input_schema": { "type": "object", "properties": {
          "points": { "type": "array", "items": { "type": "array", "items": {"type": "number"} } } },
          "required": ["points"] }
      },
      {
        "name": "wb_from_point",
        "description": "WB eyedropper: neutralize the cast at point (x,y in 0..1) that should be neutral gray. Best tool for casts.",
        "input_schema": { "type": "object", "properties": {
          "x": {"type": "number"}, "y": {"type": "number"} }, "required": ["x", "y"] }
      },
      {
        "name": "create_mask",
        "description": "Local model cuts a mask. kind: subject|background|sky|object(needs point)|radial|linear. Returns id; edit inside via mask.<id>.* paths. You conduct, model cuts pixels.",
        "input_schema": { "type": "object", "properties": {
          "kind": {"type": "string"},
          "point": { "type": "array", "items": {"type": "number"} } }, "required": ["kind"] }
      },
      {
        "name": "refine_mask",
        "description": "Adjust mask: opacity 0-100, feather 0-100, invert.",
        "input_schema": { "type": "object", "properties": {
          "id": {"type": "string"}, "opacity": {"type": "number"},
          "feather": {"type": "number"}, "invert": {"type": "boolean"} }, "required": ["id"] }
      },
      { "name": "remove_mask", "description": "Remove a mask by id.",
        "input_schema": { "type": "object", "properties": { "id": {"type": "string"} }, "required": ["id"] } },
      { "name": "get_stats", "description": "Clip % (highlights/shadows) + coarse luma histogram.",
        "input_schema": { "type": "object", "properties": {} } },
      {
        "name": "sample_color",
        "description": "Color at point (x,y): scene-linear + display sRGB. Measure skin/neutrals, don't guess.",
        "input_schema": { "type": "object", "properties": {
          "x": {"type": "number"}, "y": {"type": "number"} }, "required": ["x", "y"] }
      },
      { "name": "render_preview",
        "description": "LOOK AGAIN: fresh preview with your edits. Call after a batch to judge + self-correct.",
        "input_schema": { "type": "object", "properties": {} } },
      { "name": "undo", "description": "Undo the last edit step.",
        "input_schema": { "type": "object", "properties": {} } },
      { "name": "reset_module", "description": "Reset one module to default.",
        "input_schema": { "type": "object", "properties": { "module": {"type": "string"} }, "required": ["module"] } },
      { "name": "reset_all", "description": "Reset to neutral base (use before auto-render).",
        "input_schema": { "type": "object", "properties": {} } },
      { "name": "snapshot", "description": "Freeze current state under a name.",
        "input_schema": { "type": "object", "properties": { "name": {"type": "string"} }, "required": ["name"] } }
    ])
}

const SYSTEM_PROMPT: &str = r#"You are the editing assistant in MeraRAW, a scene-referred RAW editor. You edit only via the validated ops the sliders use — every move is undoable and visible. You conduct; local models cut pixel masks; you never touch pixels beyond the small preview shown.

Color brain (order matters): pipeline is linear Rec.2020 — exposure/WB early, tone_curve/color_grade late. Recover clipped highlights (tone_curve.highlights negative) before raising exposure. Casts: wb_from_point on a neutral; calibration.green_hue/shadow_tint for canopy/foliage. Skin = HSL orange band; film-like contrast is hue-safe; verify skin with sample_color after big color moves. Subject separation: create_mask(subject) + tiny exposure lift, create_mask(background) + slight darken/desaturate.

Style — SUBTLE and ALWAYS REVIEW: modest moves (±0.3EV class); mask grade sat 5-25, never a strong tint; a background separates by a small exposure drop + slight desaturation, NOT a heavy color cast. Do a small first batch, then IMMEDIATELY render_preview and look: anything overdone? background an unnatural color? skin drifting? Pull back (smaller values or reset_module) before adding more — at least one review+correct pass before finishing. When happy, STOP and reply in plain photographer words: what you did, why, all on the undo stack. Be brief. Offer, don't dictate."#;

/// One executed tool call → (json result, optional image for tool_result).
async fn execute_tool(
    app: &AppHandle,
    engine: &EngineHandle,
    name: &str,
    input: &Value,
) -> (Value, Option<Vec<u8>>) {
    let progress = |label: String| {
        let _ = app.emit(crate::events::ASSISTANT_PROGRESS, json!({"kind": "tool", "label": label}));
    };
    let op_result = |r: Result<Result<meratech_core::ops::DocDelta, meratech_core::error::CoreError>, meratech_core::engine::EngineError>| -> Value {
        match r {
            Ok(Ok(delta)) => json!({"ok": true, "doc": delta.doc, "newMaskId": delta.new_mask_id}),
            Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}),
            Err(e) => json!({"ok": false, "error": e.to_string()}),
        }
    };
    match name {
        "set_params" => {
            let sets = input["sets"].as_array().cloned().unwrap_or_default();
            progress(format!("adjusting {} parameter(s)", sets.len()));
            let mut results = Vec::new();
            for s in sets {
                let path = s["path"].as_str().unwrap_or("").to_string();
                let r = engine
                    .apply_op(Op::SetParam {
                        path: path.clone(),
                        value: s["value"].clone(),
                    })
                    .await;
                match r {
                    Ok(Ok(_)) => results.push(json!({"path": path, "ok": true})),
                    Ok(Err(e)) => results.push(json!({"path": path, "ok": false, "error": e.to_string()})),
                    Err(e) => results.push(json!({"path": path, "ok": false, "error": e.to_string()})),
                }
            }
            // return the resulting doc once (compact)
            let doc = engine.get_doc().await.ok().flatten();
            (json!({"results": results, "doc": doc}), None)
        }
        "set_tone_curve" => {
            progress("shaping tone curve".into());
            let r = engine
                .apply_op(Op::SetParam {
                    path: "tone_curve.points".into(),
                    value: input["points"].clone(),
                })
                .await;
            (op_result(r), None)
        }
        "wb_from_point" => {
            progress("white-balance eyedropper".into());
            let (x, y) = (
                input["x"].as_f64().unwrap_or(0.5) as f32,
                input["y"].as_f64().unwrap_or(0.5) as f32,
            );
            let r = engine.wb_from_point(x, y).await;
            (op_result(r), None)
        }
        "create_mask" => {
            let kind = input["kind"].as_str().unwrap_or("subject").to_string();
            progress(format!("creating {kind} mask (local model)"));
            let source = match kind.as_str() {
                "radial" => json!({"type":"radial","center":[0.5,0.5],"radii":[0.3,0.25],"rotation":0}),
                "linear" => json!({"type":"linear","start":[0.5,0.0],"end":[0.5,0.6]}),
                "object" => json!({"type":"segmented","model":"object_v1","hint":{"point": input["point"]}}),
                _ => json!({"type":"segmented","model":"subject_v1","hint":null}),
            };
            let r = engine.apply_op(Op::AddMask { kind, source }).await;
            (op_result(r), None)
        }
        "refine_mask" => {
            progress("refining mask".into());
            let r = engine
                .apply_op(Op::RefineMask {
                    id: input["id"].as_str().unwrap_or("").into(),
                    opacity: input["opacity"].as_f64().map(|v| v as f32),
                    feather: input["feather"].as_f64().map(|v| v as f32),
                    invert: input["invert"].as_bool(),
                })
                .await;
            (op_result(r), None)
        }
        "remove_mask" => {
            progress("removing mask".into());
            let r = engine
                .apply_op(Op::RemoveMask {
                    id: input["id"].as_str().unwrap_or("").into(),
                })
                .await;
            (op_result(r), None)
        }
        "get_stats" => {
            let stats = engine.get_stats().await.ok().flatten();
            (
                stats
                    .map(|s| {
                        // coarse 8-bin luma summary keeps the result tiny
                        let mut bins8 = [0u32; 8];
                        for (i, v) in s.luma.iter().enumerate() {
                            bins8[i * 8 / s.luma.len().max(1)] += v;
                        }
                        json!({
                            "clipHighPct": (s.clip_high_pct * 10.0).round() / 10.0,
                            "clipLowPct": (s.clip_low_pct * 10.0).round() / 10.0,
                            "lumaHist8": bins8
                        })
                    })
                    .unwrap_or(json!({"error": "no stats"})),
                None,
            )
        }
        "sample_color" => {
            let (x, y) = (
                input["x"].as_f64().unwrap_or(0.5) as f32,
                input["y"].as_f64().unwrap_or(0.5) as f32,
            );
            match engine.sample_color(x, y).await {
                Ok(Ok(c)) => (serde_json::to_value(&c).unwrap_or_default(), None),
                Ok(Err(e)) => (json!({"error": e.to_string()}), None),
                Err(e) => (json!({"error": e.to_string()}), None),
            }
        }
        "render_preview" => {
            progress("looking at the result".into());
            match engine.render_preview_jpeg(PREVIEW_DIM).await {
                Ok(Ok(jpeg)) => (json!({"ok": true}), Some(jpeg)),
                Ok(Err(e)) => (json!({"error": e.to_string()}), None),
                Err(e) => (json!({"error": e.to_string()}), None),
            }
        }
        "undo" => {
            progress("undoing last step".into());
            (op_result(engine.undo().await.map(|r| r)), None)
        }
        "reset_module" => {
            progress("resetting module".into());
            let r = engine
                .apply_op(Op::ResetModule {
                    module: input["module"].as_str().unwrap_or("").into(),
                })
                .await;
            (op_result(r), None)
        }
        "reset_all" => {
            progress("resetting to base".into());
            (op_result(engine.apply_op(Op::ResetAll).await), None)
        }
        "snapshot" => {
            let name = input["name"].as_str().unwrap_or("assistant").to_string();
            let r = engine.snapshot(name).await;
            (
                match r {
                    Ok(Ok(())) => json!({"ok": true}),
                    Ok(Err(e)) => json!({"ok": false, "error": e.to_string()}),
                    Err(e) => json!({"ok": false, "error": e.to_string()}),
                },
                None,
            )
        }
        other => (json!({"error": format!("unknown tool {other}")}), None),
    }
}

/// Assemble the eyes (spec 6.3): doc + preview + stats + metadata + masks.
async fn context_bundle(engine: &EngineHandle) -> Result<(String, Vec<u8>), AppError> {
    let doc = engine.get_doc().await?.unwrap_or(json!({}));
    let meta = engine.get_metadata().await?;
    let stats = engine.get_stats().await?;
    let preview = engine.render_preview_jpeg(PREVIEW_DIM).await??;
    let meta_txt = meta
        .map(|m| {
            format!(
                "{} {} | ISO {:?} | {:?} | f/{:?} | {}x{} | as-shot ~{:.0}K",
                m.camera_make,
                m.camera_model,
                m.iso,
                m.shutter,
                m.aperture,
                m.width,
                m.height,
                m.estimated_cct.unwrap_or(0.0)
            )
        })
        .unwrap_or_else(|| "no image".into());
    let stats_txt = stats
        .map(|s| {
            format!(
                "highlights clipped {:.1}%, shadows crushed {:.1}%",
                s.clip_high_pct, s.clip_low_pct
            )
        })
        .unwrap_or_default();
    let text = format!(
        "CURRENT EDIT DOC (params not listed are at defaults):\n{doc}\n\n\
         METADATA: {meta_txt}\nSTATS: {stats_txt}\n\
         AVAILABLE MASK MODELS: subject, background (inverse), sky, object-by-point — all local/on-device.\n\
         The attached image is the photo AS IT LOOKS RIGHT NOW with the current edits applied."
    );
    Ok((text, preview))
}

#[derive(serde::Deserialize)]
struct ApiResponse {
    content: Vec<Value>,
    stop_reason: Option<String>,
}

/// Replace every image block EXCEPT the last with a short text stub. Keeps
/// the conversation coherent while bounding input tokens (6.7).
fn strip_stale_images(messages: &mut [Value]) {
    // index of the last image block across all messages
    let mut last: Option<(usize, usize)> = None;
    for (mi, m) in messages.iter().enumerate() {
        if let Some(arr) = m["content"].as_array() {
            for (bi, b) in arr.iter().enumerate() {
                if b["type"] == "image" {
                    last = Some((mi, bi));
                }
            }
        }
    }
    for (mi, m) in messages.iter_mut().enumerate() {
        if let Some(arr) = m["content"].as_array_mut() {
            for (bi, b) in arr.iter_mut().enumerate() {
                if b["type"] == "image" && last != Some((mi, bi)) {
                    *b = json!({"type": "text", "text": "[earlier preview omitted to save tokens]"});
                }
            }
        }
    }
}

/// The agentic loop (spec 6.6). Returns the final plain-words explanation.
pub async fn run(
    app: AppHandle,
    engine: EngineHandle,
    message: String,
    mode: String,
) -> Result<String, AppError> {
    if mock_mode() {
        return run_mock(&app, &engine).await;
    }
    let key = api_key().ok_or_else(|| {
        AppError::Internal("no ANTHROPIC_API_KEY set (or MERATECH_ASSISTANT_MOCK)".into())
    })?;

    let (ctx_text, preview) = context_bundle(&engine).await?;
    let user_text = match mode.as_str() {
        "explain" => format!(
            "{ctx_text}\n\nUSER ASKS FOR DIAGNOSIS ONLY — look, measure, explain what's wrong and what you'd do, but make NO edits (no write tools) until told to.\n\nUser: {message}"
        ),
        "auto" => format!(
            "{ctx_text}\n\nAUTO-RENDER: reset to base first, read what this photo IS (subject, light, ISO), then apply a fitting starting edit — not a generic stretch. Explain briefly.\n"
        ),
        _ => format!("{ctx_text}\n\nUser: {message}"),
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&preview);
    tracing::info!(
        preview_kb = preview.len() / 1024,
        "assistant turn starts (only preview+doc+stats leave the device)"
    );

    let mut messages = vec![json!({
        "role": "user",
        "content": [
            {"type": "image", "source": {"type": "base64", "media_type": "image/jpeg", "data": b64}},
            {"type": "text", "text": user_text}
        ]
    })];

    let client = reqwest::Client::new();
    for turn in 0..MAX_LOOP_TURNS {
        // cost discipline (6.7): only the FRESHEST image stays in history;
        // older image blocks are replaced with a text stub so input tokens
        // don't balloon across look-again turns.
        strip_stale_images(&mut messages);
        let body = json!({
            "model": model(),
            "max_tokens": MAX_TOKENS,
            "system": SYSTEM_PROMPT,
            "tools": tool_defs(),
            "messages": messages,
        });
        // Backoff on 429 (rate-limit): up to 3 waits with escalating delay.
        // On a low token-per-minute tier this lets the loop finish instead
        // of erroring out mid-edit.
        const MAX_RATE_RETRIES: usize = 3;
        let mut parsed: Option<ApiResponse> = None;
        for attempt in 0..=MAX_RATE_RETRIES {
            let resp = client
                .post(API_URL)
                .header("x-api-key", &key)
                .header("anthropic-version", "2023-06-01")
                .json(&body)
                .send()
                .await
                .map_err(|e| AppError::Internal(format!("api: {e}")))?;
            let status = resp.status();
            if status.as_u16() == 429 && attempt < MAX_RATE_RETRIES {
                let wait = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(15 + attempt as u64 * 15)
                    .clamp(5, 60);
                tracing::warn!(wait, attempt, "rate limited; backing off");
                let _ = app.emit(
                    crate::events::ASSISTANT_PROGRESS,
                    json!({"kind": "tool", "label": format!("rate limited — waiting {wait}s")}),
                );
                tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                continue;
            }
            if status.as_u16() == 429 {
                return Err(AppError::Internal(
                    "Rate limited: your plan allows 30k input tokens/min. Wait a minute and retry, or add credits to raise the tier.".into(),
                ));
            }
            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                return Err(AppError::Internal(format!("api {status}: {text}")));
            }
            parsed = Some(
                resp.json()
                    .await
                    .map_err(|e| AppError::Internal(format!("api parse: {e}")))?,
            );
            break;
        }
        let Some(parsed) = parsed else {
            return Err(AppError::Internal("rate limited; try again shortly".into()));
        };

        let mut tool_results = Vec::new();
        let mut final_text = String::new();
        for block in &parsed.content {
            match block["type"].as_str() {
                Some("text") => {
                    if let Some(t) = block["text"].as_str() {
                        final_text.push_str(t);
                        let _ = app.emit(
                            crate::events::ASSISTANT_PROGRESS,
                            json!({"kind": "text", "label": t}),
                        );
                    }
                }
                Some("tool_use") => {
                    let name = block["name"].as_str().unwrap_or("");
                    let id = block["id"].as_str().unwrap_or("");
                    let (result, image) =
                        execute_tool(&app, &engine, name, &block["input"]).await;
                    let content = if let Some(jpeg) = image {
                        let b = base64::engine::general_purpose::STANDARD.encode(jpeg);
                        json!([
                            {"type": "image", "source": {"type": "base64", "media_type": "image/jpeg", "data": b}},
                            {"type": "text", "text": "Fresh preview with your edits applied."}
                        ])
                    } else {
                        json!(result.to_string())
                    };
                    tool_results.push(json!({
                        "type": "tool_result",
                        "tool_use_id": id,
                        "content": content,
                    }));
                }
                _ => {}
            }
        }

        if parsed.stop_reason.as_deref() == Some("tool_use") && !tool_results.is_empty() {
            messages.push(json!({"role": "assistant", "content": parsed.content}));
            messages.push(json!({"role": "user", "content": tool_results}));
            tracing::debug!(turn, "assistant loop continues");
            continue;
        }
        return Ok(final_text);
    }
    Ok("(stopped: edit loop reached its iteration cap — every applied move is on your undo stack)".into())
}

/// Mock executor: a canned tool sequence through the REAL execute_tool path
/// (wiring + guard-wall + preview verified without the API — test §9).
async fn run_mock(app: &AppHandle, engine: &EngineHandle) -> Result<String, AppError> {
    let script: Vec<(&str, Value)> = vec![
        ("get_stats", json!({})),
        (
            "set_params",
            json!({"sets": [
                {"path": "exposure.stops", "value": 0.4},
                {"path": "tone_curve.contrast", "value": 20},
                {"path": "color_grade.highlights_hue", "value": 50},
                {"path": "color_grade.highlights_sat", "value": 12},
            ]}),
        ),
        ("render_preview", json!({})),
        // the "look again, pull back" move
        ("set_params", json!({"sets": [{"path": "color_grade.highlights_sat", "value": 8}]})),
        // guard-wall probe: extreme value must clamp, junk path must error
        ("set_params", json!({"sets": [
            {"path": "exposure.stops", "value": 400},
            {"path": "hax.pwn", "value": 1}
        ]})),
        ("sample_color", json!({"x": 0.5, "y": 0.4})),
    ];
    let mut transcript = Vec::new();
    for (name, input) in script {
        let (result, image) = execute_tool(app, engine, name, &input).await;
        transcript.push(json!({"tool": name, "result": result, "had_image": image.is_some()}));
    }
    Ok(format!(
        "MOCK RUN COMPLETE. Warmed the exposure a touch, added gentle film contrast and a hint of golden highlights, then looked at the result and pulled the highlight saturation back. Transcript: {}",
        serde_json::to_string_pretty(&transcript).unwrap_or_default()
    ))
}

#[tauri::command]
pub async fn assistant_available() -> Result<bool, AppError> {
    Ok(available())
}

#[tauri::command]
pub async fn assistant_send(
    app: AppHandle,
    engine: tauri::State<'_, EngineHandle>,
    message: String,
    mode: Option<String>,
) -> Result<String, AppError> {
    let text = run(
        app.clone(),
        engine.inner().clone(),
        message,
        mode.unwrap_or_else(|| "edit".into()),
    )
    .await?;
    let _ = app.emit(crate::events::ASSISTANT_PROGRESS, json!({"kind": "done", "label": ""}));
    events::forward_engine_event(
        &app,
        meratech_core::message::EngineEvent::CatalogChanged,
    );
    Ok(text)
}