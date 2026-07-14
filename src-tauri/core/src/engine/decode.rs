use super::Engine;
use super::*;
use crate::export::{output_transform, TargetSpace};
use crate::image::RgbF32Buf;
use crate::message::Frame;

impl Engine {
    pub(super) fn open_image(
        &mut self,
        path: PathBuf,
        reply: oneshot::Sender<Result<ImageMeta, CoreError>>,
    ) {
        self.flush_sidecar_now();
        self.generation += 1;
        let generation = self.generation;

        let decoder = crate::raw::decoder_for(&path);
        if !decoder.probe(&path) {
            let _ = reply.send(Err(CoreError::Decode(format!(
                "unsupported file: {}",
                path.display()
            ))));
            return;
        }
        let mut meta = match decoder.metadata(&path) {
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
        meta.available_profile_files = available.iter().map(|p| p.file.clone()).collect();

        // sidecar = canonical for edits; else Adobe XMP; else fresh doc
        let mut doc = match sidecar::load_edits(&path) {
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

        // Seed denoise ISO profile hint for the classical noise node.
        if let Some(iso) = meta.iso {
            doc.unknown
                .insert("denoise_iso".into(), serde_json::json!(iso));
        }

        let chosen = crate::profile::choose_profile(
            &meta,
            &index,
            doc.meta.profile_file.as_deref(),
        );
        let profile_path = chosen.as_ref().map(|p| {
            meta.camera_profile = Some(crate::profile::profile_display_name(
                &p.file,
                camera_key.as_deref().unwrap_or(""),
            ));
            crate::profile::profile_path(&p.file)
        });

        let dcp_profile = profile_path.as_ref().and_then(|p| {
            DcpProfile::load(p)
                .ok()
                .filter(|d| d.matches_camera(&meta.camera_make, &meta.camera_model))
                .map(std::sync::Arc::new)
        });

        // Restore a previously-applied look LUT from the sidecar path, if any.
        let lut_cube = doc.meta.lut_file.as_ref().and_then(|p| {
            match crate::lut::CubeLut::load_cube(std::path::Path::new(p)) {
                Ok(c) => Some(std::sync::Arc::new(c)),
                Err(e) => {
                    tracing::warn!(error = %e, path = %p, "load look LUT on open");
                    None
                }
            }
        });

        // Demosaic algorithm from the doc (None = engine default). Surfaced on
        // meta so the UI picker shows the effective algorithm. Reset to an
        // in-process default when a sidecar algo isn't available on this OS.
        let mut demosaic = crate::raw::Demosaic::parse_or_default(doc.meta.demosaic.as_deref());
        let available = crate::raw::Demosaic::available();
        if !available.iter().any(|n| n == demosaic.name()) {
            tracing::warn!(
                requested = demosaic.name(),
                "demosaic unavailable on this platform; using rcd"
            );
            demosaic = crate::raw::Demosaic::Rcd;
            doc.meta.demosaic = Some(demosaic.name().to_string());
        }
        meta.demosaic = demosaic.name().to_string();

        self.current = Some(CurrentImage {
            path: path.clone(),
            meta: meta.clone(),
            dcp_profile,
            lut_cube,
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
                    let dec = crate::raw::decoder_for(&path);
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
        // full decode with the chosen demosaic algorithm
        self.spawn_full_decode(path, profile_path, demosaic, generation);
    }

    /// Spawn the background full-decode worker. Shared by `open_image` and
    /// `set_demosaic` (re-decode). Result is delivered as `DecodeDone`.
    pub(super) fn spawn_full_decode(
        &self,
        path: PathBuf,
        profile_path: Option<PathBuf>,
        demosaic: crate::raw::Demosaic,
        generation: u64,
    ) {
        let tx = self.self_tx.clone();
        std::thread::Builder::new()
            .name("decode-worker".into())
            .spawn(move || {
                let dec = crate::raw::decoder_for(&path);
                let started = Instant::now();
                let result = dec
                    .decode_with_options(&path, profile_path.as_deref(), demosaic)
                    .map(|img| Box::new(DecodedPayload::from_decoded(img)));
                tracing::info!(
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    ok = result.is_ok(),
                    demosaic = demosaic.name(),
                    "full decode finished"
                );
                let _ = tx.blocking_send(EngineMsg::DecodeDone { generation, result });
            })
            .expect("spawn decode worker");
    }

    pub(super) fn finish_decode(&mut self, payload: DecodedPayload) {
        let Some(gpu) = &self.gpu else {
            return self.finish_decode_cpu(payload);
        };
        let tex = upload_working_texture(gpu, &payload.rgba_f16, payload.width, payload.height);
        let view = tex.create_view(&Default::default());
        if let Some(cur) = &mut self.current {
            let camera_profile = cur.meta.camera_profile.clone();
            let available_profiles = cur.meta.available_profiles.clone();
            let available_profile_files = cur.meta.available_profile_files.clone();
            cur.working = Some((tex, view, payload.width, payload.height));
            cur.small_cpu = Some(payload.small_cpu);
            cur.meta = payload.meta;
            cur.meta.camera_profile = camera_profile;
            cur.meta.available_profiles = available_profiles;
            cur.meta.available_profile_files = available_profile_files;
        }
        // The graph caches by view key + module dirtiness and never tracks the
        // working texture's identity. On a re-decode (set_demosaic) the view is
        // unchanged, so without a full invalidation the render below serves the
        // previous decode's cached pixels.
        if let Some(g) = &mut self.graph {
            g.invalidate_all();
        }
        if let Some(g) = &mut self.export_graph {
            g.invalidate_all();
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

    /// When wgpu init failed (common on Linux VMs / bad drivers), still show a
    /// CPU-tonemapped preview so CR2/NEF open instead of hard-failing.
    fn finish_decode_cpu(&mut self, payload: DecodedPayload) {
        tracing::warn!("gpu unavailable; using CPU preview fallback");
        if let Some(cur) = &mut self.current {
            let camera_profile = cur.meta.camera_profile.clone();
            let available_profiles = cur.meta.available_profiles.clone();
            let available_profile_files = cur.meta.available_profile_files.clone();
            cur.small_cpu = Some(payload.small_cpu.clone());
            cur.meta = payload.meta;
            cur.meta.camera_profile = camera_profile;
            cur.meta.available_profiles = available_profiles;
            cur.meta.available_profile_files = available_profile_files;
        }
        let vw = self
            .last_view
            .filter(|v| v.out_w >= 64 && v.out_h >= 64)
            .map(|v| (v.out_w, v.out_h))
            .unwrap_or((1440, 860));
        let frame = cpu_preview_frame(
            &payload.small_cpu,
            self.display_look == 1,
            vw.0,
            vw.1,
        );
        let version = self.next_version();
        let mut frame = frame;
        frame.version = version;
        self.latest_frame = Some(frame);
        self.emit(EngineEvent::ImageReady { version });
        self.emit(EngineEvent::FrameReady { version });
    }

    pub(super) fn set_camera_profile(
        &mut self,
        profile_file: String,
        reply: oneshot::Sender<Result<ImageMeta, CoreError>>,
    ) {
        let index = crate::profile::ProfileIndex::embedded();
        let out_meta = match self.current.as_mut() {
            None => {
                let _ = reply.send(Err(CoreError::NoImage));
                return;
            }
            Some(cur) => {
                let profiles = crate::profile::resolve_profiles(&cur.meta, &index);
                let Some(chosen) = crate::profile::find_profile(&profiles, &profile_file) else {
                    let _ = reply.send(Err(CoreError::InvalidOp(format!(
                        "unknown profile: {profile_file}"
                    ))));
                    return;
                };
                let path = crate::profile::profile_path(&chosen.file);
                let dcp = DcpProfile::load(&path)
                    .ok()
                    .filter(|d| d.matches_camera(&cur.meta.camera_make, &cur.meta.camera_model));
                let Some(dcp) = dcp else {
                    let _ = reply.send(Err(CoreError::InvalidOp(format!(
                        "profile failed to load: {}",
                        chosen.file
                    ))));
                    return;
                };
                let camera_key = crate::profile::matched_camera_key(&cur.meta, &index);
                let display = crate::profile::profile_display_name(
                    &chosen.file,
                    camera_key.as_deref().unwrap_or(""),
                );
                cur.dcp_profile = Some(std::sync::Arc::new(dcp));
                cur.meta.camera_profile = Some(display);
                cur.doc_mut().meta.profile_file = Some(chosen.file.clone());
                cur.doc_dirty = true;
                cur.meta.clone()
            }
        };
        if let Some(g) = &mut self.graph {
            g.invalidate_all();
        }
        let display_look = self.display_look;
        if let Some(g) = &mut self.export_graph {
            g.set_look(display_look);
        }
        self.schedule_render();
        let _ = reply.send(Ok(out_meta));
    }

    /// Load (Some path) or clear (None) the 3D look LUT for the current image.
    /// The parsed cube is cached on the image; the path is stored in the doc so
    /// the look persists in the sidecar.
    pub(super) fn set_lut(
        &mut self,
        path: Option<String>,
        reply: oneshot::Sender<Result<(), CoreError>>,
    ) {
        let Some(cur) = self.current.as_mut() else {
            let _ = reply.send(Err(CoreError::NoImage));
            return;
        };
        match path {
            Some(p) => match crate::lut::CubeLut::load_cube(std::path::Path::new(&p)) {
                Ok(cube) => {
                    cur.lut_cube = Some(std::sync::Arc::new(cube));
                    cur.doc_mut().meta.lut_file = Some(p);
                    cur.doc_dirty = true;
                }
                Err(e) => {
                    let _ = reply.send(Err(e));
                    return;
                }
            },
            None => {
                cur.lut_cube = None;
                cur.doc_mut().meta.lut_file = None;
                cur.doc_dirty = true;
            }
        }
        if let Some(g) = &mut self.graph {
            g.invalidate_from_module("lut");
        }
        self.schedule_render();
        self.schedule_settle();
        let _ = reply.send(Ok(()));
    }

    /// Change the demosaic algorithm for the current image and re-decode it.
    /// The working buffer (and thus preview + export) is rebuilt; edits are
    /// preserved since they live in the render graph, not the decoded buffer.
    pub(super) fn set_demosaic(
        &mut self,
        algo: String,
        reply: oneshot::Sender<Result<ImageMeta, CoreError>>,
    ) {
        let Some(demosaic) = crate::raw::Demosaic::from_name(&algo) else {
            let _ = reply.send(Err(CoreError::InvalidOp(format!(
                "unknown demosaic algorithm: {algo}"
            ))));
            return;
        };
        // Update the doc + meta, then gather what the re-decode needs (ending the
        // &mut self.current borrow before we call spawn_full_decode).
        let (path, profile_path, out_meta) = {
            let Some(cur) = self.current.as_mut() else {
                let _ = reply.send(Err(CoreError::NoImage));
                return;
            };
            cur.doc_mut().meta.demosaic = Some(demosaic.name().to_string());
            cur.doc_dirty = true;
            cur.meta.demosaic = demosaic.name().to_string();
            let profile_file = cur.doc().meta.profile_file.clone();
            let index = crate::profile::ProfileIndex::embedded();
            let profile_path =
                crate::profile::choose_profile(&cur.meta, &index, profile_file.as_deref())
                    .map(|p| crate::profile::profile_path(&p.file));
            (cur.path.clone(), profile_path, cur.meta.clone())
        };
        // Re-decode with the same generation so DecodeDone isn't discarded as
        // stale; finish_decode replaces the working buffer and re-renders.
        self.spawn_full_decode(path, profile_path, demosaic, self.generation);
        let _ = reply.send(Ok(out_meta));
    }
}

/// CPU preview when wgpu is unavailable — fit the downscaled working buffer
/// and apply the same display look as export/present.
fn cpu_preview_frame(
    src: &RgbF32Buf,
    camera_look: bool,
    out_w: u32,
    out_h: u32,
) -> Frame {
    let out_w = out_w.max(1);
    let out_h = out_h.max(1);
    let sw = src.width as f32;
    let sh = src.height as f32;
    let scale = (out_w as f32 / sw).min(out_h as f32 / sh);
    let mut linear = vec![0.0f32; (out_w as usize) * (out_h as usize) * 3];
    for y in 0..out_h {
        for x in 0..out_w {
            let sx = ((x as f32 + 0.5) / scale - 0.5).clamp(0.0, sw - 1.0);
            let sy = ((y as f32 + 0.5) / scale - 0.5).clamp(0.0, sh - 1.0);
            let ix = sx.floor() as usize;
            let iy = sy.floor() as usize;
            let fx = sx - ix as f32;
            let fy = sy - iy as f32;
            let ix1 = (ix + 1).min(src.width.saturating_sub(1));
            let iy1 = (iy + 1).min(src.height.saturating_sub(1));
            let sample = |px: usize, py: usize| {
                let i = (py * src.width + px) * 3;
                [src.data[i], src.data[i + 1], src.data[i + 2]]
            };
            let c00 = sample(ix, iy);
            let c10 = sample(ix1, iy);
            let c01 = sample(ix, iy1);
            let c11 = sample(ix1, iy1);
            let o = ((y * out_w + x) * 3) as usize;
            for ch in 0..3 {
                linear[o + ch] = c00[ch] * (1.0 - fx) * (1.0 - fy)
                    + c10[ch] * fx * (1.0 - fy)
                    + c01[ch] * (1.0 - fx) * fy
                    + c11[ch] * fx * fy;
            }
        }
    }
    let enc = output_transform(&linear, out_w, out_h, TargetSpace::Srgb, false, camera_look);
    let px = (out_w * out_h) as usize;
    let mut rgba = vec![0u8; px * 4];
    for i in 0..px {
        rgba[i * 4] = enc.rgb8[i * 3];
        rgba[i * 4 + 1] = enc.rgb8[i * 3 + 1];
        rgba[i * 4 + 2] = enc.rgb8[i * 3 + 2];
        rgba[i * 4 + 3] = 255;
    }
    Frame {
        width: out_w,
        height: out_h,
        rgba,
        version: 0,
    }
}
