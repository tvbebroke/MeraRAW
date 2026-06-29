//! EditDoc — contract A1, the spine. State document (current param values),
//! canonical in the engine, mirrored to the frontend. Small: instructions,
//! not pixels. Full spec: editdoc-schema-spec.md.
//!
//! Pipeline order is NOT stored here (separate fixed vector in registry.rs)
//! so it can't be corrupted by an op.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: u32 = 1;

/// One param value. Canonical encodings per schema spec §2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    Bool(bool),
    F32(f32),
    Enum(String),
    /// Curve: ordered control points, x strictly increasing, x/y ∈ [0,1].
    Curve(Vec<[f32; 2]>),
    /// Color as {h,s,l} (hue 0..360, s/l normalized).
    Color { h: f32, s: f32, l: f32 },
}

impl ParamValue {
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            ParamValue::F32(v) => Some(*v),
            _ => None,
        }
    }
}

/// module id → (param id → value). Only non-default entries present.
pub type ModuleParams = BTreeMap<String, BTreeMap<String, ParamValue>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceRef {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(default)]
    pub orientation: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mask {
    pub id: String,
    pub kind: String, // subject|sky|background|object|radial|linear|brush
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default)]
    pub invert: bool,
    #[serde(default)]
    pub feather: f32,
    pub source: serde_json::Value, // schema §4.1 variants; P4 interprets
    /// SAME shape as global modules — scoped overrides (schema §4).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub modules: ModuleParams,
}

fn default_opacity() -> f32 {
    100.0
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
    #[serde(default)]
    pub rating: u8,
    #[serde(default, skip_serializing_if = "is_default_flag")]
    pub flag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_file: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
}

fn is_default_flag(f: &str) -> bool {
    f.is_empty() || f == "none"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditDoc {
    pub schema_version: u32,
    pub doc_id: String,
    pub source_ref: SourceRef,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub modules: ModuleParams,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub masks: Vec<Mask>,
    #[serde(default)]
    pub meta: DocMeta,
    /// Forward-compat: unknown future fields round-trip unharmed (§6).
    #[serde(flatten)]
    pub unknown: BTreeMap<String, serde_json::Value>,
}

impl EditDoc {
    pub fn new(source_path: &str) -> Self {
        let now = chrono_now();
        Self {
            schema_version: SCHEMA_VERSION,
            doc_id: new_id(),
            source_ref: SourceRef {
                path: source_path.to_string(),
                content_hash: None,
                orientation: 0,
            },
            modules: BTreeMap::new(),
            masks: Vec::new(),
            meta: DocMeta {
                created_at: Some(now.clone()),
                modified_at: Some(now),
                flag: "none".into(),
                ..Default::default()
            },
            unknown: BTreeMap::new(),
        }
    }

    pub fn get(&self, module: &str, param: &str) -> Option<&ParamValue> {
        self.modules.get(module).and_then(|m| m.get(param))
    }

    pub fn set(&mut self, module: &str, param: &str, value: ParamValue) {
        self.modules
            .entry(module.to_string())
            .or_default()
            .insert(param.to_string(), value);
        self.touch();
    }

    /// Remove a param (→ falls back to registry default). Prunes empty maps.
    pub fn unset(&mut self, module: &str, param: &str) {
        if let Some(m) = self.modules.get_mut(module) {
            m.remove(param);
            if m.is_empty() {
                self.modules.remove(module);
            }
        }
        self.touch();
    }

    pub fn mask(&self, id: &str) -> Option<&Mask> {
        self.masks.iter().find(|m| m.id == id)
    }

    pub fn mask_mut(&mut self, id: &str) -> Option<&mut Mask> {
        self.masks.iter_mut().find(|m| m.id == id)
    }

    pub fn touch(&mut self) {
        self.meta.modified_at = Some(chrono_now());
    }

    /// Serialize for sidecar/mirror: floats at 6 sig digits happens via f32
    /// natural printing; defaults already omitted by construction.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("doc serialize")
    }

    pub fn from_json(v: serde_json::Value) -> Result<Self, String> {
        let doc: EditDoc = serde_json::from_value(v).map_err(|e| e.to_string())?;
        if doc.schema_version > SCHEMA_VERSION {
            // newer doc: we still loaded what we know; unknown fields are
            // preserved by the flatten map. Warn, never destroy.
            tracing::warn!(
                version = doc.schema_version,
                "doc from a newer app; unknown fields preserved"
            );
        }
        Ok(migrate(doc))
    }
}

/// Migration registry (§6). v1 is current — chain grows here.
fn migrate(doc: EditDoc) -> EditDoc {
    // match doc.schema_version { 1 => doc, ... }
    doc
}

/// Preset = PartialDoc (contract D4): subset of module params.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialDoc {
    #[serde(default)]
    pub modules: ModuleParams,
}

fn new_id() -> String {
    // time + counter pseudo-uuid; avoids a uuid dep
    use std::sync::atomic::{AtomicU64, Ordering};
    static C: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);
    format!("{:x}-{:x}", t, C.fetch_add(1, Ordering::Relaxed))
}

pub fn new_mask_id() -> String {
    format!("m-{}", new_id())
}

fn chrono_now() -> String {
    // ISO-8601 UTC without a chrono dep
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs();
    let days = secs / 86400;
    let (y, m, day) = civil_from_days(days as i64);
    let tod = secs % 86400;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        day,
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

/// Howard Hinnant's civil_from_days.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_round_trips_exactly() {
        let mut doc = EditDoc::new("/x/y.ARW");
        doc.set("exposure", "stops", ParamValue::F32(0.35));
        doc.set(
            "tone_curve",
            "points",
            ParamValue::Curve(vec![[0.0, 0.0], [0.5, 0.6], [1.0, 1.0]]),
        );
        let json = doc.to_json();
        let back = EditDoc::from_json(json).unwrap();
        assert_eq!(back.get("exposure", "stops"), Some(&ParamValue::F32(0.35)));
        assert_eq!(back.doc_id, doc.doc_id);
        assert_eq!(back.modules, doc.modules);
    }

    #[test]
    fn unknown_fields_preserved_round_trip() {
        let v: serde_json::Value = serde_json::json!({
            "schema_version": 1,
            "doc_id": "abc",
            "source_ref": { "path": "/x.ARW" },
            "modules": {},
            "future_field": { "x": 1 }
        });
        let doc = EditDoc::from_json(v).unwrap();
        let out = doc.to_json();
        assert_eq!(out["future_field"]["x"], 1, "future field must round-trip");
    }

    #[test]
    fn empty_doc_serializes_small() {
        let doc = EditDoc::new("/x.ARW");
        let s = serde_json::to_string(&doc.to_json()).unwrap();
        assert!(s.len() < 400, "empty doc should be tiny, got {}", s.len());
        assert!(!s.contains("modules"), "empty modules omitted");
    }

    #[test]
    fn unset_prunes_to_default_state() {
        let mut doc = EditDoc::new("/x.ARW");
        doc.set("exposure", "stops", ParamValue::F32(1.0));
        doc.unset("exposure", "stops");
        assert!(doc.modules.is_empty());
    }

    #[test]
    fn timestamps_look_iso() {
        let doc = EditDoc::new("/x.ARW");
        let ts = doc.meta.created_at.unwrap();
        assert!(ts.ends_with('Z') && ts.contains('T') && ts.starts_with("20"), "{ts}");
    }
}
