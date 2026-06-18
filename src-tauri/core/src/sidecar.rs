//! Sidecar persistence — contract D1. `<name>.mrt.json` next to the
//! original. Canonical for edits; the P5 DB is only a fast index.
//! Atomic write (temp + rename), debounced by the engine.

use crate::doc::EditDoc;
use crate::error::CoreError;
use std::path::{Path, PathBuf};

pub fn sidecar_path(source: &Path) -> PathBuf {
    source.with_extension("mrt.json")
}

pub fn write_sidecar(doc: &EditDoc) -> Result<PathBuf, CoreError> {
    let source = PathBuf::from(&doc.source_ref.path);
    let path = sidecar_path(&source);
    let json = serde_json::to_string_pretty(&doc.to_json())
        .map_err(|e| CoreError::Io(e.to_string()))?;
    let tmp = path.with_extension("mrt.json.tmp");
    std::fs::write(&tmp, json.as_bytes())?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

pub fn load_sidecar(source: &Path) -> Result<Option<EditDoc>, CoreError> {
    let path = sidecar_path(source);
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| CoreError::Io(format!("sidecar parse: {e}")))?;
    let doc = EditDoc::from_json(value).map_err(CoreError::Io)?;
    Ok(Some(doc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::ParamValue;

    #[test]
    fn sidecar_round_trip_exact() {
        let dir = std::env::temp_dir().join("meratech-sidecar-test");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("IMG_0001.ARW");
        std::fs::write(&src, b"fake").unwrap();

        let mut doc = EditDoc::new(src.to_str().unwrap());
        doc.set("exposure", "stops", ParamValue::F32(0.75));
        let written = write_sidecar(&doc).unwrap();
        assert_eq!(written, dir.join("IMG_0001.mrt.json"));

        let loaded = load_sidecar(&src).unwrap().expect("sidecar exists");
        assert_eq!(loaded.get("exposure", "stops"), Some(&ParamValue::F32(0.75)));
        assert_eq!(loaded.doc_id, doc.doc_id);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_sidecar_is_none() {
        let p = std::env::temp_dir().join("no-such-file-12345.ARW");
        assert!(load_sidecar(&p).unwrap().is_none());
    }

    #[test]
    fn old_schema_doc_with_missing_fields_upgrades() {
        // minimal v1 doc lacking meta/masks → defaults fill
        let v = serde_json::json!({
            "schema_version": 1,
            "doc_id": "old",
            "source_ref": { "path": "/x.ARW" },
            "modules": { "exposure": { "stops": 0.2 } }
        });
        let doc = EditDoc::from_json(v).unwrap();
        assert_eq!(doc.get("exposure", "stops"), Some(&ParamValue::F32(0.2)));
        assert_eq!(doc.meta.rating, 0);
        assert!(doc.masks.is_empty());
    }
}
