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

pub const GRADE_MODULES: &[&str] = &[
    "exposure",
    "white_balance",
    "color_grade",
    "hsl",
    "tone_curve",
    "lut",
    "effects",
    "input",
    "highlights",
];

/// Copy look modules onto `dest` without touching crop / masks / retouch / calibration / detail.
pub fn merge_grade(dest: &mut EditDoc, src: &crate::doc::ModuleParams, lut_file: Option<String>) {
    for name in GRADE_MODULES {
        dest.modules.remove(*name);
        if let Some(m) = src.get(*name) {
            dest.modules.insert((*name).to_string(), m.clone());
        }
    }
    dest.meta.lut_file = crate::path_safety::sanitize_lut_path(lut_file.filter(|s| !s.is_empty()));
    dest.touch();
}

pub fn apply_grade_to_path(
    path: &Path,
    src: &crate::doc::ModuleParams,
    lut_file: Option<String>,
) -> Result<(), CoreError> {
    let mut doc = load_sidecar(path)?.unwrap_or_else(|| EditDoc::new(&path.to_string_lossy()));
    merge_grade(&mut doc, src, lut_file);
    write_sidecar(path, &doc)?;
    Ok(())
}

/// Clone the primary doc into a new virtual copy and persist the sidecar.
/// Does not touch the original image file.
pub fn add_virtual_copy(path: &Path) -> Result<String, CoreError> {
    let mut docs = match load_sidecar(path)? {
        Some(d) => split_copies(d),
        None => vec![EditDoc::new(&path.to_string_lossy())],
    };
    if docs.is_empty() {
        docs.push(EditDoc::new(&path.to_string_lossy()));
    }
    let mut copy = docs[0].clone();
    copy.copies.clear();
    copy.doc_id = format!("vc-{}", docs.len());
    let id = copy.doc_id.clone();
    docs.push(copy);
    write_sidecar(path, &bundle_copies(&docs))?;
    Ok(id)
}

/// Primary + virtual copies in one sidecar (`copies` on the primary).
pub fn split_copies(mut doc: EditDoc) -> Vec<EditDoc> {
    let extra = std::mem::take(&mut doc.copies);
    let mut docs = vec![doc];
    for v in extra {
        if let Ok(mut c) = EditDoc::from_json(v) {
            c.copies.clear();
            docs.push(c);
        }
    }
    docs
}

pub fn bundle_copies(docs: &[EditDoc]) -> EditDoc {
    let mut primary = docs.first().cloned().unwrap_or_else(|| EditDoc::new(""));
    primary.copies = docs
        .iter()
        .skip(1)
        .map(|d| {
            let mut c = d.clone();
            c.copies.clear();
            c.to_json()
        })
        .collect();
    primary
}
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

/// Load one virtual copy (or the primary when `doc_id` is empty).
pub fn load_edits_doc(source: &Path, doc_id: Option<&str>) -> Result<Option<EditDoc>, CoreError> {
    let Some(bundled) = load_edits(source)? else {
        if doc_id.map(str::trim).is_some_and(|s| !s.is_empty()) {
            return Err(CoreError::InvalidOp(format!(
                "no sidecar for virtual copy {}",
                doc_id.unwrap_or("")
            )));
        }
        return Ok(None);
    };
    let mut docs = split_copies(bundled);
    if docs.is_empty() {
        return Ok(None);
    }
    let want = doc_id.map(str::trim).filter(|s| !s.is_empty());
    let Some(id) = want else {
        return Ok(Some(docs.remove(0)));
    };
    docs.into_iter()
        .find(|d| d.doc_id == id)
        .map(Some)
        .ok_or_else(|| CoreError::InvalidOp(format!("no virtual copy {id}")))
}

/// Extra file-stem suffix so a virtual copy does not overwrite the master export.
pub fn virtual_copy_stem_suffix(doc_id: Option<&str>, primary_id: Option<&str>) -> Option<String> {
    let id = doc_id.map(str::trim).filter(|s| !s.is_empty())?;
    if primary_id.is_some_and(|p| p == id) {
        return None;
    }
    if primary_id.is_none() {
        return None;
    }
    Some(id.to_string())
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

    #[test]
    fn merge_grade_leaves_crop_masks_and_detail() {
        let mut dest = EditDoc::new("/dest.ARW");
        dest.set("crop", "left", ParamValue::F32(0.12));
        dest.set("detail", "sharpen_amount", ParamValue::F32(40.0));
        dest.set("exposure", "stops", ParamValue::F32(-1.0));
        dest.masks.push(crate::doc::Mask {
            id: "keep".into(),
            kind: "radial".into(),
            name: None,
            enabled: true,
            opacity: 80.0,
            invert: false,
            feather: 10.0,
            source: serde_json::json!({"type": "radial"}),
            blend: "normal".into(),
            modules: Default::default(),
        });
        let mut src = crate::doc::ModuleParams::new();
        src.entry("exposure".into())
            .or_default()
            .insert("stops".into(), ParamValue::F32(1.25));
        merge_grade(&mut dest, &src, None);
        assert_eq!(dest.get("crop", "left"), Some(&ParamValue::F32(0.12)));
        assert_eq!(
            dest.get("detail", "sharpen_amount"),
            Some(&ParamValue::F32(40.0))
        );
        assert_eq!(dest.masks.len(), 1);
        assert_eq!(dest.get("exposure", "stops"), Some(&ParamValue::F32(1.25)));
    }

    #[test]
    fn merge_grade_drops_unsafe_lut_path() {
        let mut dest = EditDoc::new("/dest.ARW");
        dest.meta.lut_file = Some("/tmp/ok.cube".into());
        merge_grade(
            &mut dest,
            &crate::doc::ModuleParams::new(),
            Some("/etc/passwd".into()),
        );
        assert!(dest.meta.lut_file.is_none());
    }

    #[test]
    fn virtual_copies_round_trip_in_sidecar() {
        let mut primary = EditDoc::new("/a.ARW");
        primary.set("exposure", "stops", ParamValue::F32(0.2));
        let mut copy = EditDoc::new("/a.ARW");
        copy.doc_id = "vc-1".into();
        copy.set("exposure", "stops", ParamValue::F32(1.5));
        let bundled = bundle_copies(&[primary, copy]);
        assert_eq!(bundled.copies.len(), 1);
        let split = split_copies(bundled);
        assert_eq!(split.len(), 2);
        assert_eq!(split[1].doc_id, "vc-1");
        assert_eq!(
            split[1].get("exposure", "stops"),
            Some(&ParamValue::F32(1.5))
        );
        assert!(split[0].copies.is_empty());
        assert_eq!(
            virtual_copy_stem_suffix(Some("vc-1"), Some(&split[0].doc_id)).as_deref(),
            Some("vc-1")
        );
        assert_eq!(
            virtual_copy_stem_suffix(Some(&split[0].doc_id), Some(&split[0].doc_id)),
            None
        );
    }

    #[test]
    fn load_edits_doc_picks_virtual_copy() {
        let dir = std::env::temp_dir().join(format!(
            "meratech-vc-load-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("a.ARW");
        std::fs::write(&src, b"fake").unwrap();
        let mut primary = EditDoc::new(src.to_str().unwrap());
        primary.set("exposure", "stops", ParamValue::F32(0.2));
        let mut copy = EditDoc::new(src.to_str().unwrap());
        copy.doc_id = "vc-1".into();
        copy.set("exposure", "stops", ParamValue::F32(1.5));
        write_sidecar(&src, &bundle_copies(&[primary, copy])).unwrap();
        let loaded = load_edits_doc(&src, Some("vc-1"))
            .unwrap()
            .expect("copy");
        assert_eq!(loaded.doc_id, "vc-1");
        assert_eq!(
            loaded.get("exposure", "stops"),
            Some(&ParamValue::F32(1.5))
        );
        let master = load_edits_doc(&src, None).unwrap().expect("primary");
        assert_eq!(
            master.get("exposure", "stops"),
            Some(&ParamValue::F32(0.2))
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn add_virtual_copy_persists_without_touching_source() {
        let dir = std::env::temp_dir().join(format!(
            "meratech-vc-add-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let img = dir.join("IMG.ARW");
        std::fs::write(&img, b"raw").unwrap();
        let mut primary = EditDoc::new(img.to_str().unwrap());
        primary.set("exposure", "stops", ParamValue::F32(0.4));
        write_sidecar(&img, &primary).unwrap();
        let id = add_virtual_copy(&img).unwrap();
        assert_eq!(id, "vc-1");
        assert_eq!(std::fs::read(&img).unwrap(), b"raw");
        let loaded = load_sidecar(&img).unwrap().unwrap();
        let split = split_copies(loaded);
        assert_eq!(split.len(), 2);
        assert_eq!(split[1].doc_id, "vc-1");
        std::fs::remove_dir_all(&dir).ok();
    }
}
