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
        if let Some(c) = &mut self.current {
            // primary doc only; virtual copies are in-memory until P5
            if c.doc_dirty && c.active_doc == 0 {
                match sidecar::write_sidecar(&c.path, &c.docs[0]) {
                    Ok(p) => {
                        c.doc_dirty = false;
                        tracing::debug!(path = %p.display(), "sidecar written");
                    }
                    Err(e) => tracing::error!(error = %e, "sidecar write failed"),
                }
            }
        }
    }
    pub(super) fn save_preset_to_disk(
        &mut self,
        name: &str,
        modules: &[String],
    ) -> Result<(), CoreError> {
        let c = self.current.as_ref().ok_or(CoreError::NoImage)?;
        let mut partial = crate::doc::PartialDoc {
            modules: Default::default(),
        };
        for m in modules {
            if let Some(params) = c.doc().modules.get(m) {
                partial.modules.insert(m.clone(), params.clone());
            }
        }
        let dir = presets_dir();
        std::fs::create_dir_all(&dir)?;
        let safe: String = name
            .chars()
            .map(|ch| {
                if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        let file = crate::doc::PresetFile {
            label: Some(name.to_string()),
            tags: Vec::new(),
            modules: partial.modules,
        };
        std::fs::write(
            dir.join(format!("{safe}.json")),
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

pub(super) fn load_preset(name: &str) -> Result<crate::doc::PartialDoc, CoreError> {
    load_preset_file(name).map(|f| f.into_partial())
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
