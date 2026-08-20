//! Sidecar persistence — contract D1. `<name>.mrt.json` next to the
//! original. Canonical for edits; the P5 DB is only a fast index.
//! Atomic write (temp + rename), debounced by the engine.

use crate::doc::EditDoc;
use crate::error::CoreError;
use std::path::{Path, PathBuf};

/// Hard caps against DoS via huge adjacent sidecars / XMP.
const MAX_MRT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_XMP_BYTES: u64 = 2 * 1024 * 1024;

pub fn sidecar_path(source: &Path) -> PathBuf {
    source.with_extension("mrt.json")
}

fn read_to_string_capped(path: &Path, max_bytes: u64) -> Result<String, CoreError> {
    let meta = std::fs::metadata(path).map_err(|e| CoreError::Io(e.to_string()))?;
    if meta.len() > max_bytes {
        return Err(CoreError::Io(format!(
            "file too large ({} > {max_bytes} bytes): {}",
            meta.len(),
            path.display()
        )));
    }
    std::fs::read_to_string(path).map_err(|e| CoreError::Io(e.to_string()))
}

/// Write the edit sidecar next to `source` (authoritative path — never trust
/// `doc.source_ref.path` for filesystem writes).
pub fn write_sidecar(source: &Path, doc: &EditDoc) -> Result<PathBuf, CoreError> {
    let path = sidecar_path(source);
    let json =
        serde_json::to_string_pretty(&doc.to_json()).map_err(|e| CoreError::Io(e.to_string()))?;
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
    let text = read_to_string_capped(&path, MAX_MRT_BYTES)?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| CoreError::Io(format!("sidecar parse: {e}")))?;
    let mut doc = EditDoc::from_json(value).map_err(CoreError::Io)?;
    // Bind to the opened image — ignore any attacker-controlled source_ref.path.
    doc.source_ref.path = source.to_string_lossy().into_owned();
    // Sidecar lut_file never went through IPC path validation — drop unsafe paths.
    let before = doc.meta.lut_file.clone();
    doc.meta.lut_file = crate::path_safety::sanitize_lut_path(doc.meta.lut_file.take());
    if before.is_some() && doc.meta.lut_file.is_none() {
        tracing::warn!("sidecar lut_file rejected by path safety; cleared");
    }
    Ok(Some(doc))
}

/// Adobe Lightroom / Camera Raw `.xmp` sidecar next to the RAW (same stem).
/// Used when no `.mrt.json` exists. Maps common `crs:*` develop sliders only.
pub fn load_from_xmp(source: &Path) -> Result<Option<EditDoc>, CoreError> {
    let path = source.with_extension("xmp");
    if !path.exists() {
        return Ok(None);
    }
    let text = read_to_string_capped(&path, MAX_XMP_BYTES)?;
    if !text.contains("crs:") {
        return Ok(None);
    }

    use crate::doc::ParamValue;
    let mut doc = EditDoc::new(source.to_string_lossy().as_ref());
    let mut any = false;

    let mut map = |module: &str, param: &str, xmp: &str| {
        if let Some(v) = parse_xmp_f32(&text, xmp) {
            doc.set(module, param, ParamValue::F32(v));
            any = true;
        }
    };

    map("exposure", "stops", "crs:Exposure2012");
    map("white_balance", "temp", "crs:Temperature");
    map("white_balance", "tint", "crs:Tint");
    map("tone_curve", "contrast", "crs:Contrast2012");
    map("tone_curve", "shadows", "crs:Shadows2012");
    map("tone_curve", "highlights", "crs:Highlights2012");
    map("detail", "sharpen_amount", "crs:Sharpness");

    Ok(if any { Some(doc) } else { None })
}

/// Load edits: `.mrt.json` wins; else try Adobe `.xmp` sidecar.
pub fn load_edits(source: &Path) -> Result<Option<EditDoc>, CoreError> {
    if let Some(doc) = load_sidecar(source)? {
        return Ok(Some(doc));
    }
    load_from_xmp(source)
}

fn parse_xmp_f32(text: &str, attr: &str) -> Option<f32> {
    let needle = format!("{attr}=\"");
    let start = text.find(&needle)? + needle.len();
    let rest = &text[start..];
    let end = rest.find('"')?;
    let raw = rest[..end].trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("false") {
        return None;
    }
    raw.parse().ok()
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
        let written = write_sidecar(&src, &doc).unwrap();
        assert_eq!(written, dir.join("IMG_0001.mrt.json"));

        let loaded = load_sidecar(&src).unwrap().expect("sidecar exists");
        assert_eq!(
            loaded.get("exposure", "stops"),
            Some(&ParamValue::F32(0.75))
        );
        assert_eq!(loaded.doc_id, doc.doc_id);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_rebinds_source_ref_path() {
        let dir = std::env::temp_dir().join("meratech-sidecar-rebind");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("real.ARW");
        std::fs::write(&src, b"fake").unwrap();
        let evil = serde_json::json!({
            "schema_version": 1,
            "doc_id": "x",
            "source_ref": { "path": "/tmp/evil-redirect.ARW" },
            "modules": { "exposure": { "stops": 1.0 } }
        });
        std::fs::write(dir.join("real.mrt.json"), evil.to_string()).unwrap();
        let loaded = load_sidecar(&src).unwrap().expect("sidecar");
        assert_eq!(loaded.source_ref.path, src.to_string_lossy());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_sidecar_is_none() {
        let p = std::env::temp_dir().join("no-such-file-12345.ARW");
        assert!(load_sidecar(&p).unwrap().is_none());
    }

    #[test]
    fn old_schema_doc_with_missing_fields_upgrades() {
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

    #[test]
    fn adobe_xmp_maps_common_sliders() {
        let dir = std::env::temp_dir().join("meratech-xmp-test");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("DSC0001.ARW");
        std::fs::write(&src, b"fake").unwrap();
        std::fs::write(
            dir.join("DSC0001.xmp"),
            r#"<x:xmpmeta xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/">
  <rdf:Description crs:Exposure2012="+0.75" crs:Temperature="5800" crs:Tint="+12"
    crs:Contrast2012="+15" crs:Shadows2012="-20" crs:Highlights2012="-35"/>
</x:xmpmeta>"#,
        )
        .unwrap();

        let doc = load_from_xmp(&src).unwrap().expect("xmp parsed");
        assert_eq!(doc.get("exposure", "stops"), Some(&ParamValue::F32(0.75)));
        assert_eq!(
            doc.get("white_balance", "temp"),
            Some(&ParamValue::F32(5800.0))
        );
        assert_eq!(
            doc.get("white_balance", "tint"),
            Some(&ParamValue::F32(12.0))
        );
        assert_eq!(
            doc.get("tone_curve", "contrast"),
            Some(&ParamValue::F32(15.0))
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mrt_json_wins_over_xmp() {
        let dir = std::env::temp_dir().join("meratech-edits-priority");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("IMG.ARW");
        std::fs::write(&src, b"fake").unwrap();
        std::fs::write(
            dir.join("IMG.xmp"),
            r#"<rdf:Description crs:Exposure2012="+2.0"/>"#,
        )
        .unwrap();
        let mut doc = EditDoc::new(src.to_str().unwrap());
        doc.set("exposure", "stops", ParamValue::F32(0.5));
        write_sidecar(&src, &doc).unwrap();

        let loaded = load_edits(&src).unwrap().expect("edits");
        assert_eq!(loaded.get("exposure", "stops"), Some(&ParamValue::F32(0.5)));

        std::fs::remove_dir_all(&dir).ok();
    }
}
