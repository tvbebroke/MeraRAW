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
    Color {
        h: f32,
        s: f32,
        l: f32,
    },
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
    /// Blend onto the image under this mask: normal | multiply | screen.
    #[serde(default = "default_blend", skip_serializing_if = "is_normal_blend")]
    pub blend: String,
}

fn default_opacity() -> f32 {
    100.0
}

fn default_true() -> bool {
    true
}

fn default_blend() -> String {
    "normal".into()
}

fn is_normal_blend(s: &str) -> bool {
    s.is_empty() || s == "normal"
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
    /// Path to a loaded 3D look LUT (`.cube`); the parsed cube is cached in the
    /// engine. None = no LUT. Persisted in the sidecar so a look survives reload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lut_file: Option<String>,
    /// Bundled look id when the active cube is generated (`bundled:<id>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_id: Option<String>,
    /// Demosaic algorithm name (e.g. "rcd", "amaze", "rawler"). None = engine
    /// default. Changing it re-decodes the RAW; persisted in the sidecar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub demosaic: Option<String>,
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
    /// Object-removal / heal spots (phase 10). Pixels live in the working
    /// master; the sidecar stores brush strokes so fills can be rebuilt.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retouch: Vec<crate::retouch::RetouchSpot>,
    /// Sibling virtual copies stored next to the primary in one sidecar.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<serde_json::Value>,
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
            retouch: Vec::new(),
            copies: Vec::new(),
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

    pub fn retouch(&self, id: &str) -> Option<&crate::retouch::RetouchSpot> {
        self.retouch.iter().find(|s| s.id == id)
    }

    pub fn retouch_mut(&mut self, id: &str) -> Option<&mut crate::retouch::RetouchSpot> {
        self.retouch.iter_mut().find(|s| s.id == id)
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

/// On-disk preset file — modules plus optional catalog metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub modules: ModuleParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetCatalogEntry {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl PresetFile {
    pub fn into_partial(self) -> PartialDoc {
        PartialDoc {
            modules: self.modules,
        }
    }
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
        assert!(
            ts.ends_with('Z') && ts.contains('T') && ts.starts_with("20"),
            "{ts}"
        );
    }

    /// Pins `#[serde(untagged)]` discrimination order: Bool before F32.
    /// Reordering ParamValue silently changes wire behavior.
    #[test]
    fn param_value_untagged_order_is_bool_then_f32() {
        assert_eq!(
            serde_json::from_str::<ParamValue>("true").unwrap(),
            ParamValue::Bool(true)
        );
        assert_eq!(
            serde_json::from_str::<ParamValue>("false").unwrap(),
            ParamValue::Bool(false)
        );
        assert_eq!(
            serde_json::from_str::<ParamValue>("1.0").unwrap(),
            ParamValue::F32(1.0)
        );
        assert_eq!(
            serde_json::from_str::<ParamValue>("0").unwrap(),
            ParamValue::F32(0.0)
        );
        assert_eq!(
            serde_json::from_str::<ParamValue>("\"rcd\"").unwrap(),
            ParamValue::Enum("rcd".into())
        );
        assert_eq!(
            serde_json::from_str::<ParamValue>("[]").unwrap(),
            ParamValue::Curve(vec![])
        );
        assert_eq!(
            serde_json::from_str::<ParamValue>("[[0.0,0.0],[1.0,1.0]]").unwrap(),
            ParamValue::Curve(vec![[0.0, 0.0], [1.0, 1.0]])
        );
        assert_eq!(
            serde_json::from_str::<ParamValue>(r#"{"h":12.0,"s":0.5,"l":0.4}"#).unwrap(),
            ParamValue::Color {
                h: 12.0,
                s: 0.5,
                l: 0.4
            }
        );
        // A JSON number must never be eaten by Bool.
        assert!(matches!(
            serde_json::from_str::<ParamValue>("1").unwrap(),
            ParamValue::F32(_)
        ));
    }

    #[test]
    fn full_doc_fixture_round_trips() {
        let mut doc = EditDoc {
            schema_version: SCHEMA_VERSION,
            doc_id: "doc-full-fixture".into(),
            source_ref: SourceRef {
                path: "/fixtures/full.ARW".into(),
                content_hash: Some("abc123".into()),
                orientation: 1,
            },
            modules: Default::default(),
            masks: vec![Mask {
                id: "m-radial".into(),
                kind: "radial".into(),
                name: None,
                enabled: true,
                opacity: 80.0,
                invert: false,
                feather: 12.0,
                source: serde_json::json!({
                    "type": "radial",
                    "center": [0.5, 0.5],
                    "radii": [0.3, 0.2],
                    "rotation": 0
                }),
                blend: "normal".into(),
                modules: {
                    let mut m = ModuleParams::new();
                    m.entry("exposure".into())
                        .or_default()
                        .insert("stops".into(), ParamValue::F32(1.25));
                    m
                },
            }],
            copies: vec![],
            retouch: vec![crate::retouch::RetouchSpot {
                id: "r-spot".into(),
                enabled: true,
                feather: 25.0,
                source: serde_json::json!({
                    "type": "brush",
                    "strokes": [{"radius": 0.03, "mode": "add", "points": [[0.4, 0.4], [0.41, 0.42]]}]
                }),
            }],
            meta: DocMeta {
                created_at: Some("2026-01-01T00:00:00Z".into()),
                modified_at: Some("2026-01-02T00:00:00Z".into()),
                rating: 3,
                flag: "pick".into(),
                label: Some("red".into()),
                profile_file: Some("Adobe Standard.dcp".into()),
                lut_file: Some("/looks/film.cube".into()),
                look_id: None,
                demosaic: Some("rcd".into()),
                keywords: vec!["studio".into()],
            },
            unknown: Default::default(),
        };
        doc.set("exposure", "stops", ParamValue::F32(0.35));
        doc.set("detail", "hot_pixels", ParamValue::Bool(true));
        doc.set(
            "tone_curve",
            "points",
            ParamValue::Curve(vec![[0.0, 0.0], [1.0, 1.0]]),
        );
        doc.set("white_balance", "temp", ParamValue::Enum("as-shot".into()));
        doc.set(
            "color_grade",
            "shadows",
            ParamValue::Color {
                h: 30.0,
                s: 0.2,
                l: 0.4,
            },
        );
        // set() touches modified_at — freeze so the committed fixture is stable.
        doc.meta.created_at = Some("2026-01-01T00:00:00Z".into());
        doc.meta.modified_at = Some("2026-01-02T00:00:00Z".into());

        let back = EditDoc::from_json(doc.to_json()).unwrap();
        assert_eq!(back.doc_id, doc.doc_id);
        assert_eq!(back.schema_version, SCHEMA_VERSION);
        assert_eq!(back.masks.len(), 1);
        assert_eq!(back.retouch.len(), 1);
        assert_eq!(
            back.get("detail", "hot_pixels"),
            Some(&ParamValue::Bool(true))
        );
        assert!(matches!(
            back.get("color_grade", "shadows"),
            Some(ParamValue::Color { .. })
        ));

        let mut json = serde_json::to_string_pretty(&doc).unwrap();
        json.push('\n');
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join("doc-full.json");
        let update = std::env::var("UPDATE_FIXTURES").ok().as_deref() == Some("1");
        if update {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &json).unwrap();
        }
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            committed, json,
            "doc-full.json is stale. Run: UPDATE_FIXTURES=1 cargo test -p meratech-core --lib"
        );
    }
}
