//! AI denoise track — ONNX UNet (nind-style), tiled inference, disk cache.
//! Single-flight job manager: at most one Running job; per-job cancel token.

mod cache;
mod models;
mod tiler;

pub use cache::{cache_key, CacheStore};
pub use models::{is_safe_model_id, ModelInfo, ModelRegistry, DEFAULT_MODEL_ID};
pub use tiler::{merge_tiles, split_tiles, Tile, TILE, TILE_OVERLAP};

use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

static NEXT_JOB: AtomicU64 = AtomicU64::new(1);

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
    Done { path: PathBuf },
    Cancelled,
    Failed { message: String },
}

impl JobState {
    fn is_running(&self) -> bool {
        matches!(self, JobState::Running { .. })
    }
}

/// Process-wide AI denoise job manager (one active job for v1).
pub struct AiJobManager {
    /// Cancel flag for the currently Running job only.
    active_cancel: Arc<Mutex<Option<(JobId, Arc<AtomicBool>)>>>,
    state: Arc<Mutex<Option<(JobId, JobState)>>>,
    registry: ModelRegistry,
    cache: CacheStore,
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
        }
    }

    pub fn models(&self) -> Vec<ModelInfo> {
        self.registry.list()
    }

    pub fn enqueue(
        &self,
        content_hash: &str,
        model_id: &str,
        amount: f32,
        rgb: Arc<Vec<f32>>,
        w: usize,
        h: usize,
    ) -> Result<JobId, CoreError> {
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
            *lock_mutex(&self.state)? = Some((id, JobState::Done { path }));
            return Ok(id);
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

        std::thread::Builder::new()
            .name("denoise-ai".into())
            .spawn(move || {
                let result = run_inference(
                    &model_path,
                    use_stand_in,
                    &rgb,
                    w,
                    h,
                    amount,
                    &cancel,
                    |tile, tiles, pct| {
                        if let Ok(mut g) = lock_mutex(&state) {
                            if let Some((jid, st)) = g.as_mut() {
                                if *jid == id {
                                    *st = JobState::Running {
                                        progress: JobProgress {
                                            job: id,
                                            pct,
                                            tile,
                                            tiles,
                                            eta_secs: None,
                                        },
                                    };
                                }
                            }
                        }
                    },
                );
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
                match result {
                    Ok(buf) => match cache.store(&cache_key_owned, &buf, w, h) {
                        Ok(path) => finish(JobState::Done { path }),
                        Err(e) => finish(JobState::Failed {
                            message: e.to_string(),
                        }),
                    },
                    Err(_) if cancel.load(Ordering::SeqCst) => finish(JobState::Cancelled),
                    Err(e) => finish(JobState::Failed {
                        message: e.to_string(),
                    }),
                }
            })
            .map_err(|e| CoreError::Decode(format!("spawn ai denoise: {e}")))?;

        Ok(id)
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
    mut on_progress: impl FnMut(u32, u32, f32),
) -> Result<Vec<f32>, CoreError> {
    if !use_stand_in && !model_path.is_file() {
        return Err(CoreError::Decode(format!(
            "model file missing: {}",
            model_path.display()
        )));
    }
    // Stand-in: classical chain. Real ONNX path reserved for tract-onnx session.
    let _ = model_path;
    let tiles = split_tiles(w, h);
    let n = tiles.len() as u32;
    let mut out = rgb.to_vec();
    let profile = super::profile::NoiseProfile::from_iso(6400);
    let params = super::cpu::ChainParams::from_sliders(
        &profile,
        1.0,
        70.0,
        60.0,
        50.0,
        0.0,
        &[1.0; 6],
        &[1.0; 6],
        false,
        1,
        5,
        30.0,
        false,
        1.0,
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
    let a = (amount / 100.0).clamp(0.0, 1.0);
    for i in 0..out.len() {
        out[i] = rgb[i] * (1.0 - a) + out[i] * a;
    }
    Ok(out)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_flight_rejects_second_enqueue() {
        let mgr = AiJobManager::new();
        let rgb = Arc::new(vec![0.2f32; 32 * 32 * 3]);
        let h = "a".repeat(64); // safe hex-like
        // Force a long-running job by using a larger buffer — still single-flight
        // is tested by marking Running without waiting for completion.
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
            .enqueue(&h, DEFAULT_MODEL_ID, 50.0, rgb, 32, 32)
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
}
