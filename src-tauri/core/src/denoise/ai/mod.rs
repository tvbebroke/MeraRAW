//! AI denoise track — ONNX UNet (nind-style), tiled inference, disk cache.
//! Single-flight job manager: at most one Running job; per-job cancel token.
//! Results flow back over a notify callback fired from the worker thread
//! (progress + terminal states); the engine adapts it onto its actor channel.

mod cache;
mod models;
mod tiler;

pub use cache::{cache_key, CacheStore};
pub use models::{is_safe_model_id, ModelInfo, ModelRegistry, DEFAULT_MODEL_ID};
pub use tiler::{merge_tiles, split_tiles, tile_weight, Tile, TILE, TILE_OVERLAP};

use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

static NEXT_JOB: AtomicU64 = AtomicU64::new(1);

/// Fixed network input side: one compiled plan serves every tile; edge tiles
/// are replicate-padded up to this size and cropped back after inference.
pub const NET_TILE: usize = TILE + TILE_OVERLAP;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JobId(pub u64);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub job: JobId,
    pub pct: f32,
    pub tile: u32,
    pub tiles: u32,
    pub eta_secs: Option<f32>,
}

#[derive(Debug, Clone)]
pub enum JobState {
    Queued,
    Running { progress: JobProgress },
    Done { path: Option<PathBuf> },
    Cancelled,
    Failed { message: String },
}

impl JobState {
    fn is_running(&self) -> bool {
        matches!(self, JobState::Running { .. })
    }
}

/// What the worker denoises. `Decode` re-decodes the raw at full resolution on
/// the worker thread so the result can replace the working master; `Buffer`
/// is for tests and pre-decoded callers.
pub enum JobSource {
    Buffer {
        rgb: Arc<Vec<f32>>,
        width: usize,
        height: usize,
    },
    Decode {
        path: PathBuf,
        profile_path: Option<PathBuf>,
        demosaic: crate::raw::Demosaic,
    },
}

/// Pushed from the worker thread only (never from the caller's thread) —
/// safe to adapt onto a blocking channel send.
pub enum JobNotify {
    Progress(JobProgress),
    Done {
        job: JobId,
        rgb: Arc<Vec<f32>>,
        width: usize,
        height: usize,
    },
    Cancelled {
        job: JobId,
    },
    Failed {
        job: JobId,
        message: String,
    },
}

/// Outcome of `enqueue`.
#[derive(Debug)]
pub enum Enqueued {
    /// Worker spawned; completion arrives via the notify callback.
    Started(JobId),
    /// Cache hit — denoised base already on disk; no worker was spawned and
    /// the notify callback will NOT fire. Load the file synchronously.
    Cached(JobId, PathBuf),
}

type NotifyFn = Arc<dyn Fn(JobNotify) + Send + Sync>;

/// Process-wide AI denoise job manager (one active job for v1).
pub struct AiJobManager {
    /// Cancel flag for the currently Running job only.
    active_cancel: Arc<Mutex<Option<(JobId, Arc<AtomicBool>)>>>,
    state: Arc<Mutex<Option<(JobId, JobState)>>>,
    registry: ModelRegistry,
    cache: CacheStore,
    notify: Option<NotifyFn>,
}

impl Default for AiJobManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AiJobManager {
    pub fn new() -> Self {
        Self {
            active_cancel: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(None)),
            registry: ModelRegistry::default(),
            cache: CacheStore::default(),
            notify: None,
        }
    }

    /// Install the worker→owner callback. Set once, before any enqueue.
    pub fn set_notify(&mut self, f: impl Fn(JobNotify) + Send + Sync + 'static) {
        self.notify = Some(Arc::new(f));
    }

    pub fn models(&self) -> Vec<ModelInfo> {
        self.registry.list()
    }

    pub fn enqueue(
        &self,
        content_hash: &str,
        model_id: &str,
        amount: f32,
        source: JobSource,
    ) -> Result<Enqueued, CoreError> {
        if !is_safe_model_id(model_id) {
            return Err(CoreError::Decode("invalid denoise model id".into()));
        }
        let model = self
            .registry
            .get(model_id)
            .ok_or_else(|| CoreError::Decode(format!("unknown denoise model: {model_id}")))?;
        // Allow classical stand-in when ONNX is missing; refuse unknown/broken ids.
        if !model.info.ready && !model.info.stand_in {
            return Err(CoreError::Decode(format!(
                "model '{}' not available",
                model.info.id
            )));
        }

        let key = cache_key(content_hash, model_id, amount)?;

        // Single-flight: refuse while a job is Running (audit F1).
        {
            let g = lock_mutex(&self.state)?;
            if let Some((_, st)) = g.as_ref() {
                if st.is_running() {
                    return Err(CoreError::Decode(
                        "denoise job already running; cancel or wait".into(),
                    ));
                }
            }
        }

        if let Some(path) = self.cache.lookup(&key)? {
            let id = JobId(NEXT_JOB.fetch_add(1, Ordering::Relaxed));
            *lock_mutex(&self.state)? = Some((
                id,
                JobState::Done {
                    path: Some(path.clone()),
                },
            ));
            return Ok(Enqueued::Cached(id, path));
        }

        let id = JobId(NEXT_JOB.fetch_add(1, Ordering::Relaxed));
        let cancel = Arc::new(AtomicBool::new(false));
        *lock_mutex(&self.active_cancel)? = Some((id, cancel.clone()));
        *lock_mutex(&self.state)? = Some((
            id,
            JobState::Running {
                progress: JobProgress {
                    job: id,
                    pct: 0.0,
                    tile: 0,
                    tiles: 0,
                    eta_secs: None,
                },
            },
        ));

        let state = self.state.clone();
        let active_cancel = self.active_cancel.clone();
        let cache = self.cache.clone();
        let model_path = model.path.clone();
        let use_stand_in = model.info.stand_in;
        let cache_key_owned = key;
        let notify = self.notify.clone();

        std::thread::Builder::new()
            .name("denoise-ai".into())
            .spawn(move || {
                let notify = move |n: JobNotify| {
                    if let Some(f) = &notify {
                        f(n);
                    }
                };
                let finish = |new_state: JobState| {
                    if let Ok(mut g) = lock_mutex(&state) {
                        if let Some((jid, _)) = g.as_ref() {
                            if *jid == id {
                                *g = Some((id, new_state));
                            }
                        }
                    }
                    if let Ok(mut ac) = lock_mutex(&active_cancel) {
                        if ac.as_ref().map(|(jid, _)| *jid == id).unwrap_or(false) {
                            *ac = None;
                        }
                    }
                };

                // 1) Obtain the source buffer (full-res decode for Decode jobs).
                let src = match source {
                    JobSource::Buffer { rgb, width, height } => Ok((rgb, width, height)),
                    JobSource::Decode {
                        path,
                        profile_path,
                        demosaic,
                    } => crate::raw::decoder_for(&path)
                        .decode_with_options(&path, profile_path.as_deref(), demosaic)
                        .map(|img| {
                            let (w, h) = (img.working.width, img.working.height);
                            (Arc::new(img.working.data), w, h)
                        }),
                };
                let (rgb, w, h) = match src {
                    Ok(v) => v,
                    Err(e) => {
                        finish(JobState::Failed {
                            message: e.to_string(),
                        });
                        notify(JobNotify::Failed {
                            job: id,
                            message: e.to_string(),
                        });
                        return;
                    }
                };
                if cancel.load(Ordering::SeqCst) {
                    finish(JobState::Cancelled);
                    notify(JobNotify::Cancelled { job: id });
                    return;
                }

                // 2) Denoise (tiled; per-tile progress).
                let started = std::time::Instant::now();
                let result = run_inference(
                    &model_path,
                    use_stand_in,
                    &rgb,
                    w,
                    h,
                    amount,
                    &cancel,
                    |tile, tiles, pct| {
                        let eta = if tile > 0 {
                            let per = started.elapsed().as_secs_f32() / tile as f32;
                            Some(per * (tiles - tile) as f32)
                        } else {
                            None
                        };
                        let progress = JobProgress {
                            job: id,
                            pct,
                            tile,
                            tiles,
                            eta_secs: eta,
                        };
                        if let Ok(mut g) = lock_mutex(&state) {
                            if let Some((jid, st)) = g.as_mut() {
                                if *jid == id {
                                    *st = JobState::Running {
                                        progress: progress.clone(),
                                    };
                                }
                            }
                        }
                        notify(JobNotify::Progress(progress));
                    },
                );

                // 3) Deliver. Cache-store failure is logged, not fatal — the
                // buffer still reaches the owner.
                match result {
                    Ok(buf) => {
                        let path = match cache.store(&cache_key_owned, &buf, w, h) {
                            Ok(p) => Some(p),
                            Err(e) => {
                                tracing::warn!(error = %e, "denoise cache store failed");
                                None
                            }
                        };
                        let rgb = Arc::new(buf);
                        finish(JobState::Done { path });
                        notify(JobNotify::Done {
                            job: id,
                            rgb,
                            width: w,
                            height: h,
                        });
                    }
                    Err(_) if cancel.load(Ordering::SeqCst) => {
                        finish(JobState::Cancelled);
                        notify(JobNotify::Cancelled { job: id });
                    }
                    Err(e) => {
                        finish(JobState::Failed {
                            message: e.to_string(),
                        });
                        notify(JobNotify::Failed {
                            job: id,
                            message: e.to_string(),
                        });
                    }
                }
            })
            .map_err(|e| CoreError::Decode(format!("spawn ai denoise: {e}")))?;

        Ok(Enqueued::Started(id))
    }

    pub fn cancel(&self, job: JobId) {
        if let Ok(g) = lock_mutex(&self.active_cancel) {
            if let Some((id, flag)) = g.as_ref() {
                if *id == job {
                    flag.store(true, Ordering::SeqCst);
                }
            }
        }
    }

    /// Cancel whatever job is currently running (image switch / reset).
    pub fn cancel_active(&self) {
        if let Ok(g) = lock_mutex(&self.active_cancel) {
            if let Some((_, flag)) = g.as_ref() {
                flag.store(true, Ordering::SeqCst);
            }
        }
    }

    pub fn poll(&self) -> Option<(JobId, JobState)> {
        lock_mutex(&self.state).ok().and_then(|g| g.clone())
    }
}

fn lock_mutex<T>(m: &Mutex<T>) -> Result<MutexGuard<'_, T>, CoreError> {
    Ok(m.lock().unwrap_or_else(|p| p.into_inner()))
}

fn run_inference(
    model_path: &std::path::Path,
    use_stand_in: bool,
    rgb: &[f32],
    w: usize,
    h: usize,
    amount: f32,
    cancel: &AtomicBool,
    on_progress: impl FnMut(u32, u32, f32),
) -> Result<Vec<f32>, CoreError> {
    if use_stand_in {
        run_stand_in(rgb, w, h, amount, cancel, on_progress)
    } else {
        run_onnx(model_path, rgb, w, h, amount, cancel, on_progress)
    }
}

/// Classical chain per tile — used when no ONNX weight file is installed.
fn run_stand_in(
    rgb: &[f32],
    w: usize,
    h: usize,
    amount: f32,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u32, u32, f32),
) -> Result<Vec<f32>, CoreError> {
    let tiles = split_tiles(w, h);
    let n = tiles.len() as u32;
    let mut out = rgb.to_vec();
    let profile = super::profile::NoiseProfile::from_iso(6400);
    let params = super::cpu::ChainParams::from_sliders(
        &profile, 1.0, 70.0, 60.0, 50.0, 0.0, &[1.0; 6], &[1.0; 6], false, 1, 5, 30.0, false, 1.0,
    );
    for (i, tile) in tiles.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            return Err(CoreError::Decode("cancelled".into()));
        }
        let patch = extract_tile(rgb, w, h, tile);
        let den = super::cpu::denoise_rgb(&patch, tile.pw, tile.ph, &params);
        blit_tile(&mut out, w, h, tile, &den);
        on_progress(i as u32 + 1, n, (i as f32 + 1.0) / n as f32 * 100.0);
    }
    blend_amount(&mut out, rgb, amount);
    Ok(out)
}

/// Real ONNX path: fixed-size padded tiles through one tract plan, feather
/// merged. Model I/O is display-encoded (denoisers train on encoded images);
/// the working buffer is scene-linear, so encode/decode wraps inference.
fn run_onnx(
    model_path: &std::path::Path,
    rgb: &[f32],
    w: usize,
    h: usize,
    amount: f32,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u32, u32, f32),
) -> Result<Vec<f32>, CoreError> {
    use tract_onnx::prelude::*;
    if !model_path.is_file() {
        return Err(CoreError::Decode(format!(
            "model file missing: {}",
            model_path.display()
        )));
    }
    let plan = tract_onnx::onnx()
        .model_for_path(model_path)
        .and_then(|m| {
            m.with_input_fact(
                0,
                InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 3, NET_TILE, NET_TILE)),
            )
        })
        .and_then(|m| m.into_optimized())
        .and_then(|m| m.into_runnable())
        .map_err(|e| CoreError::Decode(format!("denoise model load: {e}")))?;

    let enc = |v: f32| v.max(0.0).powf(1.0 / 2.2);
    let dec = |v: f32| v.max(0.0).powf(2.2);

    let tiles = split_tiles(w, h);
    let n = tiles.len() as u32;
    // Incremental feather merge (holding every tile result would cost
    // hundreds of MB at full res).
    let mut acc = vec![0f32; w * h * 3];
    let mut wsum = vec![0f32; w * h];
    let mut input = vec![0f32; 3 * NET_TILE * NET_TILE];
    for (i, t) in tiles.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            return Err(CoreError::Decode("cancelled".into()));
        }
        let patch = extract_tile(rgb, w, h, t);
        // Interleaved linear patch → planar encoded NCHW, replicate-padded.
        for y in 0..NET_TILE {
            let sy = y.min(t.ph - 1);
            for x in 0..NET_TILE {
                let sx = x.min(t.pw - 1);
                let si = (sy * t.pw + sx) * 3;
                for c in 0..3 {
                    input[c * NET_TILE * NET_TILE + y * NET_TILE + x] = enc(patch[si + c]);
                }
            }
        }
        let tensor = Tensor::from_shape(&[1, 3, NET_TILE, NET_TILE], &input)
            .map_err(|e| CoreError::Decode(format!("denoise tensor: {e}")))?;
        let out = plan
            .run(tvec!(tensor.into()))
            .map_err(|e| CoreError::Decode(format!("denoise inference: {e}")))?;
        let view = out[0].view();
        let flat = view
            .as_slice::<f32>()
            .map_err(|e| CoreError::Decode(format!("denoise output: {e}")))?;
        if flat.len() < 3 * NET_TILE * NET_TILE {
            return Err(CoreError::Decode(format!(
                "denoise output shape mismatch: {}",
                flat.len()
            )));
        }
        for y in 0..t.ph {
            let dy = t.y + y as i32;
            if dy < 0 || dy >= h as i32 {
                continue;
            }
            for x in 0..t.pw {
                let dx = t.x + x as i32;
                if dx < 0 || dx >= w as i32 {
                    continue;
                }
                let wt = tile_weight(t, x, y);
                let di = dy as usize * w + dx as usize;
                for c in 0..3 {
                    let v = dec(flat[c * NET_TILE * NET_TILE + y * NET_TILE + x]);
                    acc[di * 3 + c] += v * wt;
                }
                wsum[di] += wt;
            }
        }
        on_progress(i as u32 + 1, n, (i as f32 + 1.0) / n as f32 * 100.0);
    }
    for i in 0..w * h {
        let s = wsum[i].max(1e-6);
        for c in 0..3 {
            acc[i * 3 + c] /= s;
        }
    }
    blend_amount(&mut acc, rgb, amount);
    Ok(acc)
}

fn blend_amount(out: &mut [f32], original: &[f32], amount: f32) {
    let a = (amount / 100.0).clamp(0.0, 1.0);
    for i in 0..out.len() {
        out[i] = original[i] * (1.0 - a) + out[i] * a;
    }
}

fn extract_tile(rgb: &[f32], w: usize, h: usize, t: &Tile) -> Vec<f32> {
    let mut v = vec![0f32; t.pw * t.ph * 3];
    for y in 0..t.ph {
        for x in 0..t.pw {
            let sx = (t.x + x as i32).clamp(0, w as i32 - 1) as usize;
            let sy = (t.y + y as i32).clamp(0, h as i32 - 1) as usize;
            let si = (sy * w + sx) * 3;
            let di = (y * t.pw + x) * 3;
            v[di..di + 3].copy_from_slice(&rgb[si..si + 3]);
        }
    }
    v
}

fn blit_tile(dst: &mut [f32], w: usize, h: usize, t: &Tile, src: &[f32]) {
    for y in 0..t.ph {
        for x in 0..t.pw {
            let dx = t.x + x as i32;
            let dy = t.y + y as i32;
            if dx < 0 || dy < 0 || dx >= w as i32 || dy >= h as i32 {
                continue;
            }
            if x < t.halo && dx != 0 {
                continue;
            }
            if y < t.halo && dy != 0 {
                continue;
            }
            if x >= t.pw - t.halo && dx != w as i32 - 1 {
                continue;
            }
            if y >= t.ph - t.halo && dy != h as i32 - 1 {
                continue;
            }
            let si = (y * t.pw + x) * 3;
            let di = (dy as usize * w + dx as usize) * 3;
            dst[di..di + 3].copy_from_slice(&src[si..si + 3]);
        }
    }
}

/// Minimal hand-encoded ONNX (protobuf) fixture — a single-Identity-node
/// graph exercises the whole tract path (load, fixed-shape plan, NCHW
/// staging, feather merge) without shipping real weights. Test-support only
/// (also used by integration tests, hence not `#[cfg(test)]`).
#[doc(hidden)]
pub mod fixtures {
    use super::NET_TILE;

    fn pb_varint(mut v: u64, out: &mut Vec<u8>) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                break;
            }
            out.push(b | 0x80);
        }
    }

    fn pb_field_varint(field: u32, v: u64, out: &mut Vec<u8>) {
        pb_varint(u64::from(field) << 3, out);
        pb_varint(v, out);
    }

    fn pb_field_bytes(field: u32, bytes: &[u8], out: &mut Vec<u8>) {
        pb_varint((u64::from(field) << 3) | 2, out);
        pb_varint(bytes.len() as u64, out);
        out.extend_from_slice(bytes);
    }

    fn onnx_value_info(name: &str, dims: &[u64]) -> Vec<u8> {
        let mut shape = Vec::new();
        for d in dims {
            let mut dim = Vec::new();
            pb_field_varint(1, *d, &mut dim); // Dimension.dim_value
            pb_field_bytes(1, &dim, &mut shape); // TensorShapeProto.dim
        }
        let mut tensor = Vec::new();
        pb_field_varint(1, 1, &mut tensor); // Tensor.elem_type = FLOAT
        pb_field_bytes(2, &shape, &mut tensor); // Tensor.shape
        let mut ty = Vec::new();
        pb_field_bytes(1, &tensor, &mut ty); // TypeProto.tensor_type
        let mut vi = Vec::new();
        pb_field_bytes(1, name.as_bytes(), &mut vi); // ValueInfoProto.name
        pb_field_bytes(2, &ty, &mut vi); // ValueInfoProto.type
        vi
    }

    pub fn identity_onnx_bytes() -> Vec<u8> {
        let dims = [1u64, 3, NET_TILE as u64, NET_TILE as u64];
        let mut node = Vec::new();
        pb_field_bytes(1, b"x", &mut node); // NodeProto.input
        pb_field_bytes(2, b"y", &mut node); // NodeProto.output
        pb_field_bytes(4, b"Identity", &mut node); // NodeProto.op_type
        let mut graph = Vec::new();
        pb_field_bytes(1, &node, &mut graph); // GraphProto.node
        pb_field_bytes(2, b"g", &mut graph); // GraphProto.name
        pb_field_bytes(11, &onnx_value_info("x", &dims), &mut graph);
        pb_field_bytes(12, &onnx_value_info("y", &dims), &mut graph);
        let mut opset = Vec::new();
        pb_field_varint(2, 13, &mut opset); // OperatorSetIdProto.version
        let mut model = Vec::new();
        pb_field_varint(1, 8, &mut model); // ModelProto.ir_version
        pb_field_bytes(7, &graph, &mut model); // ModelProto.graph
        pb_field_bytes(8, &opset, &mut model); // ModelProto.opset_import
        model
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn single_flight_rejects_second_enqueue() {
        let mgr = AiJobManager::new();
        let rgb = Arc::new(vec![0.2f32; 32 * 32 * 3]);
        let h = "a".repeat(64); // safe hex-like
        {
            let id = JobId(99);
            let cancel = Arc::new(AtomicBool::new(false));
            *lock_mutex(&mgr.active_cancel).unwrap() = Some((id, cancel));
            *lock_mutex(&mgr.state).unwrap() = Some((
                id,
                JobState::Running {
                    progress: JobProgress {
                        job: id,
                        pct: 0.0,
                        tile: 0,
                        tiles: 1,
                        eta_secs: None,
                    },
                },
            ));
        }
        let err = mgr
            .enqueue(
                &h,
                DEFAULT_MODEL_ID,
                50.0,
                JobSource::Buffer {
                    rgb,
                    width: 32,
                    height: 32,
                },
            )
            .unwrap_err();
        assert!(err.to_string().contains("already running"));
    }

    #[test]
    fn cancel_only_affects_matching_job() {
        let mgr = AiJobManager::new();
        let id = JobId(7);
        let flag = Arc::new(AtomicBool::new(false));
        *lock_mutex(&mgr.active_cancel).unwrap() = Some((id, flag.clone()));
        mgr.cancel(JobId(8));
        assert!(!flag.load(Ordering::SeqCst));
        mgr.cancel(JobId(7));
        assert!(flag.load(Ordering::SeqCst));
    }

    /// End-to-end worker flow with the notify callback: progress arrives in
    /// order and the terminal Done carries a buffer of the right size.
    #[test]
    fn notify_delivers_progress_then_done() {
        let (tx, rx) = mpsc::channel::<JobNotify>();
        let mut cache_root = std::env::temp_dir();
        cache_root.push(format!("meraraw-ai-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cache_root);
        let mut mgr = AiJobManager {
            active_cancel: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(None)),
            registry: ModelRegistry::default(),
            cache: CacheStore::with_root(cache_root),
            notify: None,
        };
        mgr.set_notify(move |n| {
            let _ = tx.send(n);
        });
        let (w, h) = (64usize, 48usize);
        let rgb = Arc::new(vec![0.25f32; w * h * 3]);
        let hash = format!(
            "t{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let id = match mgr
            .enqueue(
                &hash,
                DEFAULT_MODEL_ID,
                100.0,
                JobSource::Buffer {
                    rgb,
                    width: w,
                    height: h,
                },
            )
            .unwrap()
        {
            Enqueued::Started(id) => id,
            Enqueued::Cached(..) => panic!("unexpected cache hit"),
        };
        let mut saw_progress = false;
        loop {
            match rx.recv_timeout(Duration::from_secs(60)).expect("notify") {
                JobNotify::Progress(p) => {
                    assert_eq!(p.job, id);
                    assert!(p.pct > 0.0 && p.pct <= 100.0);
                    saw_progress = true;
                }
                JobNotify::Done {
                    job,
                    rgb,
                    width,
                    height,
                } => {
                    assert_eq!(job, id);
                    assert_eq!((width, height), (w, h));
                    assert_eq!(rgb.len(), w * h * 3);
                    assert!(rgb.iter().all(|v| v.is_finite()));
                    break;
                }
                JobNotify::Cancelled { .. } | JobNotify::Failed { .. } => {
                    panic!("job should succeed")
                }
            }
        }
        assert!(saw_progress, "at least one progress notification");
        assert!(matches!(
            mgr.poll(),
            Some((jid, JobState::Done { .. })) if jid == id
        ));
    }

    /// Real ONNX path end-to-end: with an identity model installed, the
    /// denoised base equals the input (enc/dec round-trip + feather merge
    /// are the only transforms, both identity-preserving).
    #[test]
    fn onnx_identity_model_round_trips() {
        let mut root = std::env::temp_dir();
        root.push(format!("meraraw-onnx-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("nind-utnet-v2.onnx"),
            fixtures::identity_onnx_bytes(),
        )
        .unwrap();

        let registry = ModelRegistry::with_dir(root.clone());
        let ready = &registry.list()[0];
        assert!(
            ready.ready && !ready.stand_in,
            "fixture model should be ready"
        );

        let (tx, rx) = mpsc::channel::<JobNotify>();
        let mut mgr = AiJobManager {
            active_cancel: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(None)),
            registry,
            cache: CacheStore::with_root(root.join("cache")),
            notify: None,
        };
        mgr.set_notify(move |n| {
            let _ = tx.send(n);
        });

        let (w, h) = (100usize, 80usize);
        let src: Vec<f32> = (0..w * h * 3).map(|i| (i % 97) as f32 / 96.0).collect();
        let rgb = Arc::new(src.clone());
        match mgr
            .enqueue(
                "cafe0123",
                DEFAULT_MODEL_ID,
                100.0,
                JobSource::Buffer {
                    rgb,
                    width: w,
                    height: h,
                },
            )
            .unwrap()
        {
            Enqueued::Started(_) => {}
            Enqueued::Cached(..) => panic!("unexpected cache hit"),
        }
        loop {
            match rx.recv_timeout(Duration::from_secs(120)).expect("notify") {
                JobNotify::Progress(_) => {}
                JobNotify::Done { rgb, .. } => {
                    let max_err = rgb
                        .iter()
                        .zip(&src)
                        .map(|(a, b)| (a - b).abs())
                        .fold(0f32, f32::max);
                    assert!(max_err < 1e-3, "identity model must round-trip: {max_err}");
                    break;
                }
                JobNotify::Cancelled { .. } => panic!("cancelled"),
                JobNotify::Failed { message, .. } => panic!("onnx path failed: {message}"),
            }
        }
    }
}
