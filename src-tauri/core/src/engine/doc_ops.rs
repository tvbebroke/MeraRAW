use super::Engine;
use super::*;

impl Engine {
    pub(super) fn do_apply_op(&mut self, op: Op, live: bool) -> Result<DocDelta, CoreError> {
        let c = self.current.as_mut().ok_or(CoreError::NoImage)?;
        let before = c.doc().clone();
        let new_mask_id = ops::apply_op(c.doc_mut(), &op)?;
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
        let delta = c.delta(label, new_mask_id);
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
        self.schedule_render();
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
                if undo { "nothing to undo" } else { "nothing to redo" }.into(),
            ));
        };
        *c.doc_mut() = doc;
        c.doc_dirty = true;
        let delta = c.delta(label, None);
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
                match sidecar::write_sidecar(&c.docs[0]) {
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
            .map(|ch| if ch.is_alphanumeric() || ch == '-' || ch == '_' { ch } else { '_' })
            .collect();
        std::fs::write(
            dir.join(format!("{safe}.json")),
            serde_json::to_string_pretty(&partial).map_err(|e| CoreError::Io(e.to_string()))?,
        )?;
        Ok(())
    }
}
fn presets_dir() -> PathBuf {
    crate::catalog::data_dir().join("presets")
}

pub(super) fn list_presets() -> Vec<String> {
    std::fs::read_dir(presets_dir())
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let p = e.path();
                    (p.extension()? == "json")
                        .then(|| p.file_stem().map(|s| s.to_string_lossy().into_owned()))?
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn load_preset(name: &str) -> Result<crate::doc::PartialDoc, CoreError> {
    let text = std::fs::read_to_string(presets_dir().join(format!("{name}.json")))?;
    serde_json::from_str(&text).map_err(|e| CoreError::Io(format!("preset parse: {e}")))
}
