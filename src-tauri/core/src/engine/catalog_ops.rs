use super::Engine;
use super::*;

impl Engine {
    pub(super) fn catalog_mut(&mut self) -> Result<&mut crate::catalog::Catalog, CoreError> {
        if self.catalog.is_none() {
            self.catalog = Some(crate::catalog::Catalog::open_default()?);
        }
        Ok(self.catalog.as_mut().unwrap())
    }

    pub(super) fn start_import(
        &mut self,
        root: PathBuf,
        only_paths: Option<Vec<PathBuf>>,
    ) -> Result<u64, CoreError> {
        if !root.exists() {
            return Err(CoreError::Io(format!("folder not found: {}", root.display())));
        }
        if !root.is_dir() {
            return Err(CoreError::Io(format!("not a folder: {}", root.display())));
        }

        if let Some(st) = &self.import_state {
            if st.done < st.total {
                // Queue another root instead of failing the UI call.
                tracing::info!(queued = %root.display(), "import queued behind active import");
                if let Some(st) = &mut self.import_state {
                    st.queued_roots.push(root);
                }
                return Ok(0);
            }
            tracing::warn!("clearing stale import state");
            self.import_state = None;
        }

        let cat = self.catalog_mut()?;
        let selected_only = only_paths.is_some();
        let files = only_paths.unwrap_or_else(|| crate::catalog::scan_folder(&root));
        let root_canon = root.canonicalize().unwrap_or_else(|_| root.clone());
        let todo: Vec<PathBuf> = files
            .into_iter()
            .filter(|p| {
                if !p.exists() {
                    return false;
                }
                // Selected-only imports must stay under the chosen root.
                if selected_only {
                    let Ok(pc) = p.canonicalize() else {
                        return false;
                    };
                    return pc.starts_with(&root_canon);
                }
                let m = std::fs::metadata(p)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                !cat.is_current(&p.to_string_lossy(), m)
            })
            .collect();
        cat.remember_folder(crate::catalog::trim_path_root(&root.to_string_lossy()))?;
        let total = todo.len() as u64;
        let import_id = self.generation.wrapping_add(1000) + total;
        self.import_state = Some(ImportState {
            id: import_id,
            total,
            done: 0,
            queued_roots: Vec::new(),
        });
        if total == 0 {
            self.import_state = None;
            self.emit(EngineEvent::ImportDone { total: 0 });
            return Ok(0);
        }
        // 4 parallel workers chew chunks; results post back to the actor
        let chunks: Vec<Vec<PathBuf>> = {
            let n = 4.min(todo.len());
            let mut cs: Vec<Vec<PathBuf>> = (0..n).map(|_| Vec::new()).collect();
            for (i, p) in todo.into_iter().enumerate() {
                cs[i % n].push(p);
            }
            cs
        };
        let pending = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(chunks.len()));
        for chunk in chunks {
            let tx = self.self_tx.clone();
            let pending = pending.clone();
            std::thread::Builder::new()
                .name("import-worker".into())
                .spawn(move || {
                    for p in chunk {
                        let result = crate::catalog::import_one(&p);
                        let _ = tx.blocking_send(EngineMsg::ImportFileDone {
                            import_id,
                            file: Box::new(result),
                        });
                    }
                    if pending.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                        let _ = tx.blocking_send(EngineMsg::ImportFinished { import_id });
                    }
                })
                .expect("spawn import worker");
        }
        Ok(total)
    }

    pub(super) fn import_file_done(
        &mut self,
        import_id: u64,
        file: Result<crate::catalog::ImportedFile, CoreError>,
    ) {
        let Some(st) = &mut self.import_state else { return };
        if st.id != import_id {
            return; // stale import
        }
        st.done += 1;
        let (done, total) = (st.done, st.total);
        match file {
            Ok(f) => {
                if let Ok(cat) = self.catalog_mut() {
                    match cat.upsert_asset(
                        &f.path,
                        &f.folder,
                        &f.partial_hash,
                        f.size,
                        f.modified_ms,
                        &f.meta,
                        f.sidecar_meta.as_ref(),
                        f.has_edits,
                        f.phash.as_deref(),
                        f.blur_score,
                    ) {
                        Ok(id) => {
                            if let Some(bytes) = &f.thumb_jpeg {
                                let _ = std::fs::write(cat.thumb_path(id), bytes);
                                let _ = cat.mark_thumb(id);
                            }
                            if let Some(bytes) = &f.preview_jpeg {
                                let _ = std::fs::write(cat.preview_path(id), bytes);
                            }
                        }
                        Err(e) => tracing::error!(error = %e, path = %f.path, "upsert failed"),
                    }
                }
            }
            Err(e) => tracing::warn!(error = %e, "import file failed"),
        }
        self.emit(EngineEvent::ImportProgress { done, total });
        if done % 8 == 0 {
            self.emit(EngineEvent::CatalogChanged); // stream the grid in
        }
    }

    /// DB write + sidecar mirror (+ live doc update if the image is open).
    pub(super) fn set_asset_meta(
        &mut self,
        ids: &[i64],
        patch: &crate::catalog::MetaPatch,
    ) -> Result<(), CoreError> {
        let touched = self.catalog_mut()?.set_meta(ids, patch)?;
        for (_, path) in &touched {
            // mirror to sidecar (sidecar = portable truth)
            let p = std::path::Path::new(path);
            let mut doc = match sidecar::load_sidecar(p) {
                Ok(Some(d)) => d,
                _ => EditDoc::new(path),
            };
            apply_meta_patch(&mut doc, patch);
            if let Err(e) = sidecar::write_sidecar(p, &doc) {
                tracing::warn!(error = %e, path, "sidecar mirror failed");
            }
            // live doc update if this image is open
            if let Some(cur) = &mut self.current {
                if cur.path.to_string_lossy() == *path {
                    apply_meta_patch(cur.doc_mut(), patch);
                    cur.doc_dirty = true;
                }
            }
        }
        self.emit(EngineEvent::CatalogChanged);
        Ok(())
    }

    /// Wipe + rescan the manifest roots (R8 recovery path). Roots run
    /// sequentially; later roots are queued onto the import state.
    pub(super) fn rebuild_index(&mut self) -> Result<u64, CoreError> {
        let dir = crate::catalog::data_dir();
        let roots = crate::catalog::Catalog::folders_from_manifest(&dir);
        self.catalog_mut()?.wipe_assets()?;
        let mut iter = roots.into_iter();
        let Some(first) = iter.next() else {
            return Ok(0);
        };
        let queued = self.start_import(PathBuf::from(first), None)?;
        if let Some(st) = &mut self.import_state {
            st.queued_roots = iter.map(PathBuf::from).collect();
        }
        Ok(queued)
    }

    /// Kick off segmentation inference for any segmented mask whose source
    /// changed (cache by source hash — never re-infer on param edits).
    pub(super) fn ensure_segmentations(&mut self) {
        let Some(cur) = &mut self.current else { return };
        let Some(small) = cur.small_cpu.clone() else { return };
        let generation = self.generation;
        // collect work first (avoid holding doc borrow while mutating)
        let jobs: Vec<(String, String, u64, serde_json::Value)> = cur.docs[cur.active_doc]
            .masks
            .iter()
            .filter(|m| {
                m.source.get("type").and_then(|t| t.as_str()) == Some("segmented")
            })
            .map(|m| {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                m.kind.hash(&mut h);
                m.source.to_string().hash(&mut h);
                (m.id.clone(), m.kind.clone(), h.finish(), m.source.clone())
            })
            .collect();
        for (id, kind, hash, source) in jobs {
            let cached = cur.masks_gpu.get(&id).map(|(h, _)| *h) == Some(hash);
            if cached || cur.pending_segments.contains(&id) {
                continue;
            }
            cur.pending_segments.insert(id.clone());
            let tx = self.self_tx.clone();
            let img = small.clone();
            std::thread::Builder::new()
                .name("segment-worker".into())
                .spawn(move || {
                    use crate::segment::{Segmenter, TractSegmenter};
                    let seg = TractSegmenter;
                    let started = Instant::now();
                    // segmentation runs on a further-downscaled copy
                    let input = img.downscale_to(768);
                    let result = match kind.as_str() {
                        "subject" | "background" => seg.subject(&input),
                        "sky" => seg.sky(&input),
                        "object" => {
                            let p = source
                                .get("hint")
                                .and_then(|h| h.get("point"))
                                .and_then(|p| p.as_array())
                                .and_then(|a| {
                                    Some((
                                        a.first()?.as_f64()? as f32,
                                        a.get(1)?.as_f64()? as f32,
                                    ))
                                })
                                .unwrap_or((0.5, 0.5));
                            seg.object(&input, p)
                        }
                        other => Err(CoreError::InvalidOp(format!(
                            "kind {other} is not segmented"
                        ))),
                    };
                    tracing::info!(
                        kind = %kind,
                        ms = started.elapsed().as_millis() as u64,
                        ok = result.is_ok(),
                        "segmentation finished"
                    );
                    let _ = tx.blocking_send(EngineMsg::SegmentDone {
                        generation,
                        mask_id: id,
                        source_hash: hash,
                        result,
                    });
                })
                .expect("spawn segment worker");
        }
    }
}

fn apply_meta_patch(doc: &mut EditDoc, patch: &crate::catalog::MetaPatch) {
    if let Some(r) = patch.rating {
        doc.meta.rating = r.min(5);
    }
    if let Some(f) = &patch.flag {
        doc.meta.flag = f.clone();
    }
    if let Some(l) = &patch.label {
        doc.meta.label = l.clone();
    }
    if let Some(kw) = &patch.add_keyword {
        if !doc.meta.keywords.contains(kw) {
            doc.meta.keywords.push(kw.clone());
        }
    }
    if let Some(kw) = &patch.remove_keyword {
        doc.meta.keywords.retain(|k| k != kw);
    }
    doc.touch();
}
