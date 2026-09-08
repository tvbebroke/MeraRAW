use super::Engine;
use super::*;

impl Engine {
    pub(super) fn do_apply_op(&mut self, op: Op, live: bool) -> Result<DocDelta, CoreError> {
        let retouch_op = matches!(
            op,
            Op::AddRetouchSpot { .. }
                | Op::RemoveRetouchSpot { .. }
                | Op::SetRetouchSource { .. }
                | Op::RefineRetouchSpot { .. }
                | Op::ResetAll
        );
        let c = self.current.as_mut().ok_or(CoreError::NoImage)?;
        let before = c.doc().clone();
        let new_id = ops::apply_op(c.doc_mut(), &op)?;
        let label = op.label();
        // Undo coalescing: a live drag records nothing per-tick — it just stashes
        // the pre-gesture state once. The committing (non-live) op folds the whole
        // gesture into a single undo entry from that stashed state.
        if live {
            if c.gesture_before.is_none() {
                c.gesture_before = Some(before);
            }
        } else {
            let base = c.gesture_before.take().unwrap_or(before);
            c.history.record(base, label.clone());
        }
        c.doc_dirty = true;
        let (new_mask_id, new_retouch_id) = match &op {
            Op::AddMask { .. } => (new_id, None),
            Op::AddRetouchSpot { .. } => (None, new_id),
            _ => (None, None),
        };
        let delta = c.delta(label, new_mask_id, new_retouch_id);
        match op.affected_module() {
            Some(m) => {
                if let Some(g) = &mut self.graph {
                    g.invalidate_from_module(&m);
                }
            }
            None => {
                if let Some(g) = &mut self.graph {
                    g.invalidate_all();
                }
            }
        }
        // Heal spots rewrite the working master (non-live commits only).
        if retouch_op && !live {
            self.rebuild_retouch();
            self.rerender_after_base_change();
        } else {
            self.schedule_render();
        }
        self.schedule_settle();
        Ok(delta)
    }

    pub(super) fn do_undo(&mut self, undo: bool) -> Result<DocDelta, CoreError> {
        let c = self.current.as_mut().ok_or(CoreError::NoImage)?;
        let cur_doc = c.doc().clone();
        let restored = if undo {
            c.history.undo(&cur_doc)
        } else {
            c.history.redo(&cur_doc)
        };
        let Some((doc, label)) = restored else {
            return Err(CoreError::InvalidOp(
                if undo {
                    "nothing to undo"
                } else {
                    "nothing to redo"
                }
                .into(),
            ));
        };
        *c.doc_mut() = doc;
        c.doc_dirty = true;
        let delta = c.delta(label, None, None);
        self.rebuild_retouch();
        self.full_redraw();
        self.schedule_settle();
        self.emit(EngineEvent::DocUpdated {
            delta: serde_json::to_value(&delta).unwrap_or_default(),
        });
        Ok(delta)
    }

    pub(super) fn flush_sidecar_now(&mut self) {
        let Some(c) = &self.current else {
            return;
        };
        if !c.doc_dirty {
            return;
        }
        let path = c.path.clone();
        let bundled = sidecar::bundle_copies(&c.docs);
        let has_edits =
            !bundled.modules.is_empty() || !bundled.masks.is_empty() || !bundled.copies.is_empty();
        match sidecar::write_sidecar(&path, &bundled) {
            Ok(p) => {
                if let Some(c) = &mut self.current {
                    c.doc_dirty = false;
                }
                tracing::debug!(path = %p.display(), "sidecar written");
                if let Ok(cat) = self.catalog_mut() {
                    let _ = cat.mark_has_edits(&path.to_string_lossy(), has_edits);
                }
            }
            Err(e) => tracing::error!(error = %e, "sidecar write failed"),
        }
    }

    /// Clone the active (or on-disk primary) edit into a new virtual copy.
    /// Never deletes or rewrites the original image bytes.
    pub(super) fn make_virtual_copy(&mut self, path: Option<String>) -> Result<String, CoreError> {
        let requested = path.filter(|p| !p.is_empty());
        let current_path = self
            .current
            .as_ref()
            .map(|c| c.path.to_string_lossy().into_owned());
        if requested.is_none() || requested.as_deref() == current_path.as_deref() {
            let c = self.current.as_mut().ok_or(CoreError::NoImage)?;
            let mut copy = c.doc().clone();
            copy.copies.clear();
            copy.doc_id = format!("vc-{}", c.docs.len());
            let id = copy.doc_id.clone();
            c.docs.push(copy);
            c.doc_dirty = true;
            self.flush_sidecar_now();
            self.emit(EngineEvent::CatalogChanged);
            return Ok(id);
        }
        let p = std::path::PathBuf::from(requested.unwrap());
        let id = sidecar::add_virtual_copy(&p)?;
        if let Ok(cat) = self.catalog_mut() {
            let _ = cat.mark_has_edits(&p.to_string_lossy(), true);
        }
        self.emit(EngineEvent::CatalogChanged);
        Ok(id)
    }

    /// Remove a non-master virtual copy from the open image. The RAW is untouched.
    pub(super) fn delete_virtual_copy(&mut self, doc_id: &str) -> Result<(), CoreError> {
        let switch_needed = {
            let c = self.current.as_mut().ok_or(CoreError::NoImage)?;
            if c.docs.len() <= 1 {
                return Err(CoreError::InvalidOp(
                    "cannot delete the only version".into(),
                ));
            }
            let idx = c
                .docs
                .iter()
                .position(|d| d.doc_id == doc_id)
                .ok_or_else(|| CoreError::InvalidOp(format!("no doc {doc_id}")))?;
            if idx == 0 {
                return Err(CoreError::InvalidOp("cannot delete the master".into()));
            }
            c.docs.remove(idx);
            if c.active_doc == idx {
                c.active_doc = 0;
            } else if c.active_doc > idx {
                c.active_doc -= 1;
            }
            c.doc_dirty = true;
            true
        };
        self.flush_sidecar_now();
        if switch_needed {
            self.full_redraw();
        }
        self.emit(EngineEvent::CatalogChanged);
        Ok(())
    }

    pub(super) fn save_preset_to_disk(
        &mut self,
        name: &str,
        modules: &[String],
        grade: Option<&crate::doc::ModuleParams>,
    ) -> Result<(), CoreError> {
        let name = validate_preset_name(name)?;
        let mut partial = crate::doc::PartialDoc {
            modules: Default::default(),
        };
        if let Some(src) = grade {
            for key in PRESET_SAVE_MODULES {
                if let Some(params) = src.get(*key) {
                    partial.modules.insert((*key).to_string(), params.clone());
                }
            }
        } else {
            let c = self.current.as_ref().ok_or(CoreError::NoImage)?;
            for m in modules {
                if !PRESET_SAVE_MODULES.contains(&m.as_str()) {
                    continue;
                }
                if let Some(params) = c.doc().modules.get(m) {
                    partial.modules.insert(m.clone(), params.clone());
                }
            }
        }
        let dir = presets_dir();
        std::fs::create_dir_all(&dir)?;
        let file = crate::doc::PresetFile {
            label: Some(name.to_string()),
            tags: Vec::new(),
            modules: partial.modules,
        };
        std::fs::write(
            dir.join(format!("{name}.json")),
            serde_json::to_string_pretty(&file).map_err(|e| CoreError::Io(e.to_string()))?,
        )?;
        Ok(())
    }
}
fn presets_dir() -> PathBuf {
    crate::catalog::data_dir().join("presets")
}

/// Shipped presets (repo `presets/bundled/` in dev; `MERATECH_BUNDLED_PRESETS` in prod).
pub fn bundled_presets_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("MERATECH_BUNDLED_PRESETS") {
        let p = PathBuf::from(p);
        if p.is_dir() {
            return Some(p);
        }
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets/bundled");
    if dev.is_dir() {
        return Some(dev);
    }
    None
}

/// Modules a named preset may persist. Crop / masks / retouch stay off this list.
const PRESET_SAVE_MODULES: &[&str] = &[
    "exposure",
    "white_balance",
    "calibration",
    "detail",
    "color_grade",
    "hsl",
    "tone_curve",
    "lut",
    "effects",
    "input",
    "highlights",
];

/// Reject path separators / traversal — preset names are identifiers only.
fn validate_preset_name(name: &str) -> Result<&str, CoreError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 128 {
        return Err(CoreError::InvalidOp("invalid preset name".into()));
    }
    if name.contains("..")
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || std::path::Path::new(name).is_absolute()
    {
        return Err(CoreError::InvalidOp("invalid preset name".into()));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == ' ' || c == '.')
    {
        return Err(CoreError::InvalidOp("invalid preset name".into()));
    }
    Ok(name)
}

fn preset_json_paths(name: &str) -> Result<Vec<PathBuf>, CoreError> {
    let name = validate_preset_name(name)?;
    let mut out = vec![presets_dir().join(format!("{name}.json"))];
    if let Some(b) = bundled_presets_dir() {
        out.push(b.join(format!("{name}.json")));
    }
    Ok(out)
}

pub(super) fn list_presets() -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut scan = |dir: &PathBuf| {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("json") {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        names.push(stem.to_string());
                    }
                }
            }
        }
    };
    scan(&presets_dir());
    if let Some(b) = bundled_presets_dir() {
        scan(&b);
    }
    names.sort();
    names.dedup();
    names
}

pub(super) fn auto_preset_for(camera: &str, iso: Option<u32>) -> Option<String> {
    let cam = camera.trim().to_lowercase();
    let cam_key = format!("camera:{cam}");
    let iso_key = iso.map(|i| format!("iso:{i}"));
    let mut best: Option<(i32, String)> = None;
    for name in list_presets() {
        let Ok(file) = load_preset_file(&name) else {
            continue;
        };
        let tags: Vec<String> = file.tags.iter().map(|t| t.to_lowercase()).collect();
        if tags.is_empty() {
            continue;
        }
        let cam_tags: Vec<&str> = tags
            .iter()
            .filter(|t| !t.starts_with("iso:"))
            .map(|s| s.as_str())
            .collect();
        let iso_tags: Vec<&str> = tags
            .iter()
            .filter(|t| t.starts_with("iso:"))
            .map(|s| s.as_str())
            .collect();
        let cam_hit = !cam.is_empty()
            && cam_tags.iter().any(|t| {
                *t == cam
                    || *t == cam_key
                    || t.strip_prefix("camera:").is_some_and(|rest| rest == cam)
                    || cam.contains(t)
            });
        let iso_hit = iso_key
            .as_ref()
            .is_some_and(|k| iso_tags.iter().any(|t| *t == k));
        let ok = match (!cam_tags.is_empty(), !iso_tags.is_empty()) {
            (true, true) => cam_hit && iso_hit,
            (true, false) => cam_hit,
            (false, true) => iso_hit,
            (false, false) => false,
        };
        if !ok {
            continue;
        }
        let score = i32::from(cam_hit) * 2 + i32::from(iso_hit);
        if best.as_ref().is_none_or(|(s, _)| score > *s) {
            best = Some((score, name));
        }
    }
    best.map(|(_, n)| n)
}

pub(super) fn load_preset(name: &str) -> Result<crate::doc::PartialDoc, CoreError> {
    load_preset_file(name).map(|f| f.into_partial())
}

/// Drop every previous catalog preset, then apply `partial`. Looks (`lut`) stay
/// unless the incoming preset itself sets a LUT.
pub(super) fn replace_named_preset(
    doc: &mut crate::doc::EditDoc,
    name: &str,
    partial: crate::doc::PartialDoc,
) -> Result<(), CoreError> {
    let incoming_lut = partial.modules.contains_key("lut");
    for m in PRESET_SAVE_MODULES {
        if *m == "lut" && !incoming_lut {
            continue;
        }
        ops::apply_op(
            doc,
            &Op::ResetModule {
                module: (*m).to_string(),
            },
        )?;
    }
    ops::apply_op(doc, &Op::ApplyPreset { preset: partial })?;
    doc.meta.preset_id = Some(name.to_string());
    Ok(())
}

impl Engine {
    pub(super) fn apply_named_preset(&mut self, name: &str) -> Result<DocDelta, CoreError> {
        let name = validate_preset_name(name)?.to_string();
        let partial = load_preset(&name)?;
        let c = self.current.as_mut().ok_or(CoreError::NoImage)?;
        let before = c.doc().clone();
        replace_named_preset(c.doc_mut(), &name, partial)?;
        c.doc_mut().touch();
        let label = format!("preset {name}");
        c.history.record(before, label.clone());
        c.doc_dirty = true;
        let delta = c.delta(label, None, None);
        if let Some(g) = &mut self.graph {
            g.invalidate_all();
        }
        self.schedule_render();
        self.schedule_settle();
        Ok(delta)
    }
}

const MAX_PRESET_BYTES: u64 = 1024 * 1024;

pub(super) fn load_preset_file(name: &str) -> Result<crate::doc::PresetFile, CoreError> {
    for path in preset_json_paths(name)? {
        if path.exists() {
            let meta = std::fs::metadata(&path)?;
            if meta.len() > MAX_PRESET_BYTES {
                return Err(CoreError::Io("preset file too large".into()));
            }
            let text = std::fs::read_to_string(&path)?;
            let file: crate::doc::PresetFile = serde_json::from_str(&text)
                .map_err(|e| CoreError::Io(format!("preset parse: {e}")))?;
            return Ok(file);
        }
    }
    Err(CoreError::Io(format!("preset not found: {name}")))
}

fn humanize_preset_id(id: &str) -> String {
    id.replace('_', " ")
}

pub(super) fn list_preset_catalog() -> Vec<crate::doc::PresetCatalogEntry> {
    let mut entries: Vec<crate::doc::PresetCatalogEntry> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut scan = |dir: &PathBuf| {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) != Some("json") {
                    continue;
                }
                let Some(id) = p.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                if !seen.insert(id.to_string()) {
                    continue;
                }
                let label;
                let tags;
                match std::fs::read_to_string(&p)
                    .ok()
                    .and_then(|t| serde_json::from_str::<crate::doc::PresetFile>(&t).ok())
                {
                    Some(file) => {
                        label = file.label.unwrap_or_else(|| humanize_preset_id(id));
                        tags = file.tags;
                    }
                    None => {
                        label = humanize_preset_id(id);
                        tags = Vec::new();
                    }
                }
                entries.push(crate::doc::PresetCatalogEntry {
                    id: id.to_string(),
                    label,
                    tags,
                });
            }
        }
    };

    scan(&presets_dir());
    if let Some(b) = bundled_presets_dir() {
        scan(&b);
    }

    entries.sort_by(|a, b| a.label.cmp(&b.label));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_name_rejects_traversal() {
        assert!(validate_preset_name("../x").is_err());
        assert!(validate_preset_name("ok-name").is_ok());
        assert!(validate_preset_name("My Look").is_ok());
        assert!(validate_preset_name("a/b").is_err());
        assert!(validate_preset_name("").is_err());
    }

    #[test]
    fn named_preset_replaces_previous_instead_of_stacking() {
        use crate::doc::{EditDoc, ParamValue, PartialDoc};
        use std::collections::BTreeMap;

        let mut grain = BTreeMap::new();
        grain.insert("grain_amount".into(), ParamValue::F32(35.0));
        let mut a = BTreeMap::new();
        a.insert("effects".into(), grain);

        let mut contrast = BTreeMap::new();
        contrast.insert("contrast".into(), ParamValue::F32(20.0));
        let mut b = BTreeMap::new();
        b.insert("tone_curve".into(), contrast);

        let mut doc = EditDoc::new("/x.ARW");
        replace_named_preset(
            &mut doc,
            "vintage",
            PartialDoc { modules: a },
        )
        .unwrap();
        assert_eq!(
            doc.get("effects", "grain_amount"),
            Some(&ParamValue::F32(35.0))
        );

        replace_named_preset(
            &mut doc,
            "punch",
            PartialDoc { modules: b },
        )
        .unwrap();
        assert_eq!(doc.meta.preset_id.as_deref(), Some("punch"));
        assert_eq!(
            doc.get("tone_curve", "contrast"),
            Some(&ParamValue::F32(20.0))
        );
        assert!(
            doc.get("effects", "grain_amount").is_none(),
            "previous preset grain must not stack: {:?}",
            doc.get("effects", "grain_amount")
        );
    }
}
