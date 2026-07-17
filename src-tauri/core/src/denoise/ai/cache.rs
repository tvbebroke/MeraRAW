//! Denoised-base disk cache. Keyed by (content_hash, model, amount_bucket).
//! Keys are allowlisted; loads are size-bounded; files live under the app
//! support cache dir (not world-shared temp).

use crate::error::CoreError;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const AMOUNT_STEP: f32 = 5.0;
/// Full-res denoise bases are cached now — sized for 100 MP-class sensors.
const MAX_DIM: usize = 16384;
/// Hard cap on a single cache blob (100 MP × RGB f32 ≈ 1.2 GiB).
const MAX_FILE_BYTES: usize = 2 * 1024 * 1024 * 1024;
/// Soft cap on total cache directory size before LRU prune.
const MAX_CACHE_DIR_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Clone, Default)]
pub struct CacheStore {
    root: Arc<Mutex<Option<PathBuf>>>,
}

impl CacheStore {
    pub fn with_root(root: PathBuf) -> Self {
        Self {
            root: Arc::new(Mutex::new(Some(root))),
        }
    }

    fn dir(&self) -> Result<PathBuf, CoreError> {
        let mut g = lock_mutex(&self.root)?;
        if g.is_none() {
            let mut p = app_cache_root();
            p.push("denoise");
            std::fs::create_dir_all(&p)
                .map_err(|e| CoreError::Decode(format!("denoise cache mkdir: {e}")))?;
            *g = Some(p);
        }
        Ok(g.clone().expect("cache root set above"))
    }

    pub fn lookup(&self, key: &str) -> Result<Option<PathBuf>, CoreError> {
        validate_cache_key_component(key)?;
        let p = self.dir()?.join(format!("{key}.bin"));
        // Refuse path escape even if key somehow slipped validation.
        let dir = self.dir()?;
        if !p.starts_with(&dir) {
            return Err(CoreError::Decode("denoise cache path escape".into()));
        }
        Ok(if p.is_file() { Some(p) } else { None })
    }

    pub fn store(&self, key: &str, rgb: &[f32], w: usize, h: usize) -> Result<PathBuf, CoreError> {
        validate_cache_key_component(key)?;
        let expect = checked_pixel_count(w, h)?;
        if rgb.len() != expect {
            return Err(CoreError::Decode(format!(
                "denoise cache store size mismatch: {} vs {expect}",
                rgb.len()
            )));
        }
        let dir = self.dir()?;
        std::fs::create_dir_all(&dir)
            .map_err(|e| CoreError::Decode(format!("denoise cache mkdir: {e}")))?;
        let p = dir.join(format!("{key}.bin"));
        if !p.starts_with(&dir) {
            return Err(CoreError::Decode("denoise cache path escape".into()));
        }
        let mut bytes = Vec::with_capacity(16 + rgb.len() * 4);
        bytes.extend_from_slice(&(w as u32).to_le_bytes());
        bytes.extend_from_slice(&(h as u32).to_le_bytes());
        bytes.extend_from_slice(&(rgb.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        for v in rgb {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        if bytes.len() > MAX_FILE_BYTES {
            return Err(CoreError::Decode("denoise cache blob too large".into()));
        }
        std::fs::write(&p, &bytes)
            .map_err(|e| CoreError::Decode(format!("denoise cache write: {e}")))?;
        prune_cache_dir(&dir);
        Ok(p)
    }

    pub fn load(path: &Path) -> Result<(usize, usize, Vec<f32>), CoreError> {
        let meta = std::fs::metadata(path)
            .map_err(|e| CoreError::Decode(format!("denoise cache stat: {e}")))?;
        if meta.len() as usize > MAX_FILE_BYTES {
            return Err(CoreError::Decode("denoise cache file too large".into()));
        }
        let bytes =
            std::fs::read(path).map_err(|e| CoreError::Decode(format!("denoise cache read: {e}")))?;
        if bytes.len() < 16 {
            return Err(CoreError::Decode("denoise cache truncated".into()));
        }
        let w = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let h = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let n = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let expect = checked_pixel_count(w, h)?;
        if n != expect {
            return Err(CoreError::Decode(format!(
                "denoise cache length mismatch: n={n} expect={expect}"
            )));
        }
        if bytes.len() != 16 + n * 4 {
            return Err(CoreError::Decode("denoise cache size mismatch".into()));
        }
        let mut rgb = Vec::with_capacity(n);
        for i in 0..n {
            let o = 16 + i * 4;
            rgb.push(f32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()));
        }
        Ok((w, h, rgb))
    }
}

/// Build a filesystem-safe cache key. Both IDs must already be allowlisted.
pub fn cache_key(content_hash: &str, model_id: &str, amount: f32) -> Result<String, CoreError> {
    validate_cache_key_component(content_hash)?;
    validate_cache_key_component(model_id)?;
    let bucket = ((amount / AMOUNT_STEP).round() * AMOUNT_STEP) as i32;
    let key = format!("{content_hash}_{model_id}_{bucket}");
    validate_cache_key_component(&key)?;
    Ok(key)
}

fn validate_cache_key_component(s: &str) -> Result<(), CoreError> {
    if s.is_empty() || s.len() > 200 {
        return Err(CoreError::Decode("denoise cache key length".into()));
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(CoreError::Decode(
            "denoise cache key has illegal characters".into(),
        ));
    }
    if s.contains("..") {
        return Err(CoreError::Decode("denoise cache key path traversal".into()));
    }
    Ok(())
}

fn checked_pixel_count(w: usize, h: usize) -> Result<usize, CoreError> {
    if w == 0 || h == 0 || w > MAX_DIM || h > MAX_DIM {
        return Err(CoreError::Decode(format!(
            "denoise cache dims out of range: {w}x{h}"
        )));
    }
    w.checked_mul(h)
        .and_then(|p| p.checked_mul(3))
        .ok_or_else(|| CoreError::Decode("denoise cache dim overflow".into()))
}

fn app_cache_root() -> PathBuf {
    let mut p = if let Some(h) = std::env::var_os("HOME") {
        let mut p = PathBuf::from(h);
        p.push("Library");
        p.push("Application Support");
        p.push("MeraRAW");
        p
    } else {
        std::env::temp_dir().join("meraraw")
    };
    p.push("cache");
    p
}

/// Drop oldest `.bin` files until the directory is under the soft cap.
fn prune_cache_dir(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
    let mut total = 0u64;
    for ent in rd.flatten() {
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) != Some("bin") {
            continue;
        }
        let Ok(meta) = ent.metadata() else { continue };
        let len = meta.len();
        let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        total = total.saturating_add(len);
        files.push((path, len, modified));
    }
    if total <= MAX_CACHE_DIR_BYTES {
        return;
    }
    files.sort_by_key(|(_, _, m)| *m);
    for (path, len, _) in files {
        if total <= MAX_CACHE_DIR_BYTES {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(len);
        }
    }
}

fn lock_mutex<T>(m: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, CoreError> {
    Ok(m.lock().unwrap_or_else(|p| p.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal_keys() {
        assert!(cache_key("abc", "../evil", 50.0).is_err());
        assert!(cache_key("abc/def", "model", 50.0).is_err());
        assert!(validate_cache_key_component("..").is_err());
    }

    #[test]
    fn load_rejects_mismatched_length() {
        let dir = tempfile_dir();
        let p = dir.join("bad.bin");
        // w=2 h=2 but n claims 1_000_000_000
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&1_000_000_000u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        std::fs::write(&p, bytes).unwrap();
        assert!(CacheStore::load(&p).is_err());
    }

    #[test]
    fn store_round_trip() {
        let store = CacheStore::with_root(tempfile_dir());
        let rgb = vec![0.1f32, 0.2, 0.3, 0.4, 0.5, 0.6];
        let key = cache_key("deadbeef", "nind-utnet-v2", 50.0).unwrap();
        let path = store.store(&key, &rgb, 2, 1).unwrap();
        let (w, h, out) = CacheStore::load(&path).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(out, rgb);
    }

    fn tempfile_dir() -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("meraraw-denoise-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
