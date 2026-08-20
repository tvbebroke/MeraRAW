//! Headless engine tests — no Tauri, no window. DoD items 5 + 8.

use meratech_core::engine;

#[tokio::test]
async fn ping_round_trips_through_actor() {
    let handle = engine::spawn();
    let status = handle.ping().await.expect("ping reply");
    assert!(status.alive, "engine must report alive");
}

#[tokio::test]
async fn test_frame_has_correct_dimensions_and_size() {
    let handle = engine::spawn();
    let frame = handle.test_frame(320, 200).await.expect("frame reply");
    assert_eq!(frame.width, 320);
    assert_eq!(frame.height, 200);
    assert_eq!(frame.rgba.len(), 320 * 200 * 4);
    // fully opaque
    assert!(frame.rgba.chunks_exact(4).all(|px| px[3] == 255));
}

#[tokio::test]
async fn info_returns_without_hanging() {
    let handle = engine::spawn();
    let info = handle.info().await.expect("info reply");
    // gpu may legitimately be absent on CI; just assert the call works
    let _ = info.gpu_adapter;
}

/// Full AI-denoise loop against a real raw: open → denoise_ai_start (worker
/// re-decodes full-res, runs the identity ONNX fixture through tract) →
/// DenoiseProgress events → DenoiseDone → working master swapped (ImageReady
/// re-emitted). Slow (~1 min full-res in a dev build), so opt-in.
///
///   MERARAW_DENOISE_MODELS_DIR=<tmp> cargo test -p meratech-core \
///     --test engine_test denoise_events -- --ignored --nocapture
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs ~/Desktop/test-claude-raw/DSC07078.ARW; ~1 min full-res job"]
async fn denoise_events_flow_end_to_end() {
    use meratech_core::message::EngineEvent;
    use std::path::PathBuf;
    use std::time::Duration;

    let raw =
        PathBuf::from(std::env::var("HOME").unwrap()).join("Desktop/test-claude-raw/DSC07078.ARW");
    if !raw.is_file() {
        eprintln!("fixture raw missing — skipping");
        return;
    }

    // Identity ONNX model in a temp registry dir (env hook read by the
    // engine's default ModelRegistry).
    let models = std::env::temp_dir().join(format!("meraraw-e2e-models-{}", std::process::id()));
    std::fs::create_dir_all(&models).unwrap();
    std::fs::write(
        models.join("nind-utnet-v2.onnx"),
        meratech_core::denoise::ai::fixtures::identity_onnx_bytes(),
    )
    .unwrap();
    std::env::set_var("MERARAW_DENOISE_MODELS_DIR", &models);

    let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel::<EngineEvent>();
    let handle = engine::spawn_with_events(Some(ev_tx));

    handle
        .open_image(raw, None)
        .await
        .expect("open reply")
        .expect("open ok");

    // Wait for the initial full decode (ImageReady) before starting the job.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        let ev = tokio::time::timeout_at(deadline, ev_rx.recv())
            .await
            .expect("initial decode timed out")
            .expect("event channel open");
        if matches!(ev, EngineEvent::ImageReady { .. }) {
            break;
        }
    }

    let job = handle
        .denoise_ai_start()
        .await
        .expect("start reply")
        .expect("start ok");

    let mut progress = 0u32;
    let mut image_ready_after = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(600);
    loop {
        let ev = tokio::time::timeout_at(deadline, ev_rx.recv())
            .await
            .expect("denoise job timed out")
            .expect("event channel open");
        match ev {
            EngineEvent::DenoiseProgress { job: j, pct, .. } => {
                assert_eq!(j, job);
                assert!(pct > 0.0 && pct <= 100.0);
                progress += 1;
            }
            EngineEvent::ImageReady { .. } => image_ready_after = true,
            EngineEvent::DenoiseDone { job: j } => {
                assert_eq!(j, job);
                break;
            }
            EngineEvent::DenoiseError { message, .. } => panic!("denoise failed: {message}"),
            _ => {}
        }
    }
    assert!(progress > 0, "progress events must arrive before done");
    assert!(
        image_ready_after,
        "base swap must re-render (ImageReady) before DenoiseDone"
    );
}
