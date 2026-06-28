use super::Engine;
use super::*;

impl Engine {
    pub(super) fn open_image(
        &mut self,
        path: PathBuf,
        reply: oneshot::Sender<Result<ImageMeta, CoreError>>,
    ) {
        self.flush_sidecar_now();
        self.generation += 1;
        let generation = self.generation;

        if !self.decoder.probe(&path) {
            let _ = reply.send(Err(CoreError::Decode(format!(
                "unsupported file: {}",
                path.display()
            ))));
            return;
        }
        let mut meta = match self.decoder.metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                let _ = reply.send(Err(e));
                return;
            }
        };

        let index = crate::profile::ProfileIndex::embedded();
        let available = crate::profile::resolve_profiles(&meta, &index);
        let camera_key = crate::profile::matched_camera_key(&meta, &index);
        meta.available_profiles = available
            .iter()
            .map(|p| {
                crate::profile::profile_display_name(
                    &p.file,
                    camera_key.as_deref().unwrap_or(""),
                )
            })
            .collect();
        let profile_path = match crate::profile::autoload_profile(&meta, &index) {
            Ok(Some(ap)) => {
                meta.camera_profile = Some(ap.name.clone());
                Some(crate::profile::profile_path(&ap.file))
            }
            _ => None,
        };

        let dcp_profile = profile_path.as_ref().and_then(|p| {
            DcpProfile::load(p)
                .ok()
                .filter(|d| d.matches_camera(&meta.camera_make, &meta.camera_model))
                .map(std::sync::Arc::new)
        });

        // sidecar = canonical for edits; else Adobe XMP; else fresh doc
        let doc = match sidecar::load_edits(&path) {
            Ok(Some(d)) => {
                tracing::info!("edit sidecar loaded");
                d
            }
            Ok(None) => EditDoc::new(&path.to_string_lossy()),
            Err(e) => {
                tracing::warn!(error = %e, "sidecar unreadable; starting fresh");
                EditDoc::new(&path.to_string_lossy())
            }
        };

        self.current = Some(CurrentImage {
            path: path.clone(),
            meta: meta.clone(),
            dcp_profile,
            working: None,
            small_cpu: None,
            docs: vec![doc],
            active_doc: 0,
            history: History::default(),
            snapshots: Vec::new(),
            gesture_before: None,
            doc_dirty: false,
            masks_gpu: Default::default(),
            pending_segments: Default::default(),
        });
        self.latest_frame = None;
        if let Some(g) = &mut self.graph {
            g.invalidate_all();
        }
        let _ = reply.send(Ok(meta));

        // fast path: embedded preview (camera JPEG — replaced when decode finishes)
        {
            let tx = self.self_tx.clone();
            let path = path.clone();
            std::thread::Builder::new()
                .name("preview-worker".into())
                .spawn(move || {
                    let dec = RawlerDecoder::default();
                    match dec.embedded_preview(&path, 2560) {
                        Ok(Some((rgba, width, height))) => {
                            let _ = tx.blocking_send(EngineMsg::PreviewDone {
                                generation,
                                rgba,
                                width,
                                height,
                            });
                        }
                        Ok(None) => tracing::warn!("no embedded preview"),
                        Err(e) => tracing::warn!(error = %e, "preview extract failed"),
                    }
                })
                .expect("spawn preview worker");
        }
        // full decode
        {
            let tx = self.self_tx.clone();
            let profile_path = profile_path.clone();
            std::thread::Builder::new()
                .name("decode-worker".into())
                .spawn(move || {
                    use crate::raw::Decoder;
                    let dec = RawlerDecoder::default();
                    let started = Instant::now();
                    let result = dec
                        .decode_with_profile(&path, profile_path.as_deref())
                        .map(|img| Box::new(DecodedPayload::from_decoded(img)));
                    tracing::info!(
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        ok = result.is_ok(),
                        "full decode finished"
                    );
                    let _ = tx.blocking_send(EngineMsg::DecodeDone { generation, result });
                })
                .expect("spawn decode worker");
        }
    }

    pub(super) fn finish_decode(&mut self, payload: DecodedPayload) {
        let Some(gpu) = &self.gpu else {
            self.emit(EngineEvent::DecodeError {
                message: "gpu unavailable; cannot display decoded image".into(),
            });
            return;
        };
        let tex = upload_working_texture(gpu, &payload.rgba_f16, payload.width, payload.height);
        let view = tex.create_view(&Default::default());
        if let Some(cur) = &mut self.current {
            let camera_profile = cur.meta.camera_profile.clone();
            let available_profiles = cur.meta.available_profiles.clone();
            cur.working = Some((tex, view, payload.width, payload.height));
            cur.small_cpu = Some(payload.small_cpu);
            cur.meta = payload.meta;
            cur.meta.camera_profile = camera_profile;
            cur.meta.available_profiles = available_profiles;
        }
        let vw = self
            .last_view
            .filter(|v| v.out_w >= 64 && v.out_h >= 64)
            .map(|v| (v.out_w, v.out_h))
            .unwrap_or((1440, 860));
        let fit = ViewParams::fit(vw.0, vw.1);
        match self.render_view(fit) {
            Ok(info) => {
                self.emit(EngineEvent::ImageReady {
                    version: info.version,
                });
                self.emit(EngineEvent::FrameReady {
                    version: info.version,
                });
            }
            Err(e) => {
                tracing::error!(error = %e, "post-decode render failed");
                self.emit(EngineEvent::DecodeError {
                    message: e.to_string(),
                });
            }
        }
    }
}
