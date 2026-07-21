//! Model registry — download-on-use, sha256, versioning (doc 05).
//! IPC-facing fields never include absolute filesystem paths (audit F5).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_MODEL_ID: &str = "nind-utnet-v2";

/// Public model descriptor for Tauri / UI. No absolute paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size_mb: f32,
    /// True only when the ONNX file is present (and hash-ok when configured).
    pub ready: bool,
    /// True when inference will use the classical stand-in (no ONNX yet).
    pub stand_in: bool,
    pub sha256: Option<String>,
    pub license: String,
}

/// Server-side record with the on-disk path (never serialized to the webview).
#[derive(Debug, Clone)]
pub struct ModelRecord {
    pub info: ModelInfo,
    pub path: PathBuf,
    /// Expected SHA-256 hex (lowercase). None = skip verify (stand-in / unset).
    pub expected_sha256: Option<&'static str>,
}

#[derive(Clone, Default)]
pub struct ModelRegistry {
    models_dir: Option<PathBuf>,
}

impl ModelRegistry {
    pub fn with_dir(dir: PathBuf) -> Self {
        Self {
            models_dir: Some(dir),
        }
    }

    fn dir(&self) -> PathBuf {
        self.models_dir.clone().unwrap_or_else(|| {
            // Test/selftest hook: point the default registry somewhere else.
            if let Some(d) = std::env::var_os("MERARAW_DENOISE_MODELS_DIR") {
                if !d.is_empty() {
                    return PathBuf::from(d);
                }
            }
            let mut p = dirs_fallback();
            p.push("models");
            p.push("denoise");
            p
        })
    }

    pub fn list(&self) -> Vec<ModelInfo> {
        self.records().into_iter().map(|r| r.info).collect()
    }

    pub fn get(&self, id: &str) -> Option<ModelRecord> {
        if !is_safe_model_id(id) {
            return None;
        }
        self.records().into_iter().find(|m| m.info.id == id)
    }

    fn records(&self) -> Vec<ModelRecord> {
        let dir = self.dir();
        let path = dir.join("nind-utnet-v2.onnx");
        let on_disk = path.is_file();
        // Pin before ready=true. None until the published weight digest is set.
        // Unpinned on-disk files are treated as not-ready unless explicitly
        // allowed (tests / local dig via MERARAW_DENOISE_ALLOW_UNPINNED=1 or
        // MERARAW_DENOISE_MODELS_DIR).
        //
        // `with_dir` counts as that explicit opt-in too: it is the programmatic
        // form of MERARAW_DENOISE_MODELS_DIR, and `dir()` already honours it.
        // Checking only the env vars here meant a caller-supplied directory
        // found the file but was never granted unpinned permission, so `ready`
        // stayed false. Production builds construct via `default()`, leaving
        // models_dir None, so this cannot loosen shipped integrity checks.
        let expected_sha256: Option<&'static str> = None;
        let allow_unpinned = self.models_dir.is_some()
            || std::env::var_os("MERARAW_DENOISE_ALLOW_UNPINNED").is_some_and(|v| v == "1")
            || std::env::var_os("MERARAW_DENOISE_MODELS_DIR").is_some_and(|v| !v.is_empty());
        let hash_ok = match (on_disk, expected_sha256) {
            (false, _) => false,
            (true, None) => allow_unpinned,
            (true, Some(expect)) => verify_sha256(&path, expect),
        };
        let ready = hash_ok;
        let stand_in = !ready;
        vec![ModelRecord {
            info: ModelInfo {
                id: DEFAULT_MODEL_ID.into(),
                name: if ready {
                    "MeraNoise v1 (nind-UNet)".into()
                } else {
                    "MeraNoise v1 (classical stand-in)".into()
                },
                size_mb: 48.0,
                ready,
                stand_in,
                sha256: expected_sha256.map(|s| s.to_string()),
                license: "download-on-use; see denoise/05-ai-denoising.md".into(),
            },
            path,
            expected_sha256,
        }]
    }
}

/// Only allowlisted model ids may enter cache keys / path joins.
pub fn is_safe_model_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        && !id.contains("..")
}

fn verify_sha256(path: &std::path::Path, expect_hex: &str) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    // blake3 is already a workspace dep; use it as integrity digest until
    // we pin published ONNX sha256 values. Field name stays sha256 in the
    // public API for doc compatibility; value is blake3 hex when set.
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let Ok(n) = f.read(&mut buf) else {
            return false;
        };
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    hasher.finalize().to_hex().as_str() == expect_hex
}

fn dirs_fallback() -> PathBuf {
    if let Some(h) = std::env::var_os("HOME") {
        let mut p = PathBuf::from(h);
        p.push("Library");
        p.push("Application Support");
        p.push("MeraRAW");
        return p;
    }
    std::env::temp_dir().join("meraraw")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal_ids() {
        assert!(!is_safe_model_id("../evil"));
        assert!(!is_safe_model_id("a/b"));
        assert!(is_safe_model_id(DEFAULT_MODEL_ID));
    }

    #[test]
    fn list_never_exposes_path_field() {
        let json = serde_json::to_value(ModelRegistry::default().list()).unwrap();
        let obj = &json[0];
        assert!(obj.get("path").is_none());
        assert!(obj.get("ready").is_some());
        assert!(obj.get("standIn").is_some() || obj.get("stand_in").is_some());
    }
}
