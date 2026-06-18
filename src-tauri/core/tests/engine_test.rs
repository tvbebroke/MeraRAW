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
