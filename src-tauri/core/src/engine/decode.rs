use super::Engine;
use super::*;
use crate::export::{output_transform, TargetSpace};
use crate::image::RgbF32Buf;
use crate::message::Frame;
use crate::raw::Demosaic;

impl Engine {
    pub(super) fn open_image(
        &mut self,
        path: PathBuf,
        doc_id: Option<String>,
        reply: oneshot::Sender<Result<ImageMeta, CoreError>>,
    ) {
        self.flush_sidecar_now();
        // A running AI denoise job belongs to the outgoing image — its result
        // would be dropped as stale anyway, so stop burning the CPU.
        self.ai_denoise.cancel_active();
        self.pending_denoise = None;
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
                crate::profile::profile_display_name(&p.file, camera_key.as_deref().unwrap_or(""))
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

        let chosen =
            crate::profile::choose_profile(&meta, &index, doc.meta.profile_file.as_deref());
        if let Some(p) = chosen.as_ref() {
            meta.camera_profile = Some(crate::profile::profile_display_name(
                &p.file,
                camera_key.as_deref().unwrap_or(""),
            ));
        }
        // Decoder matrix only when a real .dcp exists (Adobe/RT fallbacks OK).
        let profile_path = crate::profile::decode_profile_path(chosen.as_ref());
        let dcp_profile = crate::profile::load_dcp_profile(&meta, chosen.as_ref())
            .map(std::sync::Arc::new);

        // Restore a previously-applied look LUT from the sidecar path, if any.
        // Re-check path safety here (defense in depth vs. older sidecars).
        doc.meta.lut_file = crate::path_safety::sanitize_lut_path(doc.meta.lut_file.take());
        let lut_cube =
            doc.meta.lut_file.as_ref().and_then(|p| {
                match crate::lut::CubeLut::load_cube(std::path::Path::new(p)) {
                    Ok(c) => Some(std::sync::Arc::new(c)),
                    Err(e) => {
                        tracing::warn!(error = %e, path = %p, "load look LUT on open");
                        None
                    }
                }
            });

        // Demosaic: sidecar override, else camera-aware default (Fuji → dht).
        let mut demosaic =
            Demosaic::for_open(&meta.camera_make, &meta.camera_model, doc.meta.demosaic.as_deref());
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
        meta.available_demosaic = available;

        let mut docs = sidecar::split_copies(doc);
        if docs
            .first()
            .is_some_and(|d| d.modules.is_empty() && d.masks.is_empty())
            && (!meta.camera_model.is_empty() || meta.iso.is_some())
        {
            if let Some(name) = super::doc_ops::auto_preset_for(&meta.camera_model, meta.iso) {
                if let Ok(partial) = super::doc_ops::load_preset(&name) {
                    let _ = crate::ops::apply_op(
                        &mut docs[0],
                        &crate::ops::Op::ApplyPreset { preset: partial },
                    );
                }
            }
        }

        let active_doc = doc_id
            .as_ref()
            .and_then(|id| docs.iter().position(|d| &d.doc_id == id))
            .unwrap_or(0);

        self.current = Some(CurrentImage {
            path: path.clone(),
            meta: meta.clone(),
            dcp_profile,
            lut_cube,
            working: None,
            small_cpu: None,
            clean_rgb: None,
            docs,
            active_doc,
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
            cur.clean_rgb = Some(payload.clean_rgb);
            cur.meta = payload.meta;
            cur.meta.camera_profile = camera_profile;
            cur.meta.available_profiles = available_profiles;
            cur.meta.available_profile_files = available_profile_files;
            // Keep sidecar demosaic in sync with the effective decode algo
            // (fallback may have remapped an unavailable zerawler choice).
            cur.doc_mut().meta.demosaic = Some(cur.meta.demosaic.clone());
        }
        // Sidecar may already have heal spots — rebuild onto the clean master.
        let needs_heal = self
            .current
            .as_ref()
            .map(|c| c.doc().retouch.iter().any(|s| s.enabled))
            .unwrap_or(false);
        if needs_heal {
            self.rebuild_retouch();
        }
        self.rerender_after_base_change();
    }

    /// The working master was replaced (re-decode, AI denoise). The graph
    /// caches by view key + module dirtiness and never tracks the working
    /// texture's identity — on an unchanged view a render without full
    /// invalidation serves the previous base's cached pixels.
    pub(super) fn rerender_after_base_change(&mut self) {
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
            cur.clean_rgb = Some(payload.clean_rgb);
            cur.meta = payload.meta;
            cur.meta.camera_profile = camera_profile;
            cur.meta.available_profiles = available_profiles;
            cur.meta.available_profile_files = available_profile_files;
            cur.doc_mut().meta.demosaic = Some(cur.meta.demosaic.clone());
        }
        let needs_heal = self
            .current
            .as_ref()
            .map(|c| c.doc().retouch.iter().any(|s| s.enabled))
            .unwrap_or(false);
        if needs_heal {
            self.rebuild_retouch();
        }
        let small = self
            .current
            .as_ref()
            .and_then(|c| c.small_cpu.clone())
            .unwrap_or(payload.small_cpu);
        let vw = self
            .last_view
            .filter(|v| v.out_w >= 64 && v.out_h >= 64)
            .map(|v| (v.out_w, v.out_h))
            .unwrap_or((1440, 860));
        let kind = self
            .current
            .as_ref()
            .map(|c| c.meta.kind)
            .unwrap_or(crate::raw::ImageKind::Rendered);
        let look = crate::raw::effective_display_look(kind, self.display_look);
        let frame = cpu_preview_frame(&small, look == 1, vw.0, vw.1);
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
            Some(cur) if !cur.meta.kind.allows_raw_only_stages() => {
                let _ = reply.send(Err(CoreError::InvalidOp(
                    "camera profiles apply to RAW only".into(),
                )));
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
                let Some(dcp) = crate::profile::load_dcp_profile(&cur.meta, Some(chosen)) else {
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
            Some(p) => {
                let p = if p.starts_with("bundled:") || p.starts_with("user:") {
                    p
                } else {
                    match crate::look::install_user_look(std::path::Path::new(&p)) {
                        Ok(key) => key,
                        Err(e) => {
                            let _ = reply.send(Err(e));
                            return;
                        }
                    }
                };
                let Some(safe_s) = crate::path_safety::sanitize_lut_path(Some(p)) else {
                    let _ = reply.send(Err(CoreError::InvalidOp("path not allowed".into())));
                    return;
                };
                match crate::lut::CubeLut::load_cube(std::path::Path::new(&safe_s)) {
                    Ok(cube) => {
                        if let Some(id) = safe_s.strip_prefix("bundled:") {
                            cur.doc_mut().set(
                                "lut",
                                "input_primaries",
                                crate::doc::ParamValue::F32(1.0),
                            );
                            cur.doc_mut().set(
                                "lut",
                                "output_primaries",
                                crate::doc::ParamValue::F32(1.0),
                            );
                            cur.doc_mut()
                                .set("lut", "shaper", crate::doc::ParamValue::F32(1.0));
                            cur.doc_mut().set(
                                "lut",
                                "kind",
                                crate::doc::ParamValue::F32(
                                    crate::look::look_info(id)
                                        .map(|l| l.kind as f32)
                                        .unwrap_or(0.0),
                                ),
                            );
                            cur.doc_mut()
                                .set("lut", "enabled", crate::doc::ParamValue::F32(1.0));
                            if let Some((g, sz)) = crate::look::grain_for(id) {
                                cur.doc_mut().set(
                                    "effects",
                                    "grain_amount",
                                    crate::doc::ParamValue::F32(g),
                                );
                                cur.doc_mut().set(
                                    "effects",
                                    "grain_size",
                                    crate::doc::ParamValue::F32(sz),
                                );
                            }
                        }
                        cur.lut_cube = Some(std::sync::Arc::new(cube));
                        cur.doc_mut().meta.lut_file = Some(safe_s.clone());
                        cur.doc_mut().meta.look_id = if safe_s.starts_with("bundled:") {
                            safe_s.strip_prefix("bundled:").map(|s| s.to_string())
                        } else if safe_s.starts_with("user:") {
                            Some(safe_s.clone())
                        } else {
                            None
                        };
                        cur.doc_dirty = true;
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e));
                        return;
                    }
                }
            }
            None => {
                cur.lut_cube = None;
                cur.doc_mut().meta.lut_file = None;
                cur.doc_mut().meta.look_id = None;
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

    pub(super) fn seek_video(
        &mut self,
        frame: u32,
        reply: oneshot::Sender<Result<ImageMeta, CoreError>>,
    ) {
        let Some(cur) = self.current.as_ref() else {
            let _ = reply.send(Err(CoreError::NoImage));
            return;
        };
        if cur.meta.kind != crate::raw::ImageKind::Video {
            let _ = reply.send(Err(CoreError::InvalidOp("not a video clip".into())));
            return;
        }
        let path = cur.path.clone();
        let max = cur
            .meta
            .video
            .as_ref()
            .map(|v| v.frame_count.saturating_sub(1))
            .unwrap_or(0);
        let frame = frame.min(max);
        let t = crate::registry::effective_f32(cur.doc(), "input", "transfer").round() as u32;
        let p = crate::registry::effective_f32(cur.doc(), "input", "primaries").round() as u32;
        let (override_t, override_p) = crate::video::overrides_from_input_params(t, p);
        let working = match (|| {
            let pr = crate::video::probe(&path)?;
            let land = crate::video::land_from_tags(&pr, override_t, override_p);
            crate::video::decode_frame(&path, frame, land)
        })() {
            Ok(w) => w,
            Err(e) => {
                let _ = reply.send(Err(e));
                return;
            }
        };
        let payload = crate::raw::DecodedImage {
            working,
            meta: cur.meta.clone(),
        };
        let mut boxed = DecodedPayload::from_decoded(payload);
        if let Some(v) = boxed.meta.video.as_mut() {
            v.frame = frame;
        }
        if let Some(cur) = self.current.as_mut() {
            cur.doc_mut()
                .unknown
                .insert("video_frame".into(), serde_json::json!(frame));
            if let Some(v) = cur.meta.video.as_mut() {
                v.frame = frame;
            }
            boxed.meta = cur.meta.clone();
        }
        let meta = boxed.meta.clone();
        self.finish_decode(boxed);
        let _ = reply.send(Ok(meta));
    }

    /// Change the demosaic algorithm for the current image and re-decode it.
    /// The working buffer (and thus preview + export) is rebuilt; edits are
    /// preserved since they live in the render graph, not the decoded buffer.
    pub(super) fn set_demosaic(
        &mut self,
        algo: String,
        reply: oneshot::Sender<Result<ImageMeta, CoreError>>,
    ) {
        if self
            .current
            .as_ref()
            .is_some_and(|c| !c.meta.kind.allows_raw_only_stages())
        {
            let _ = reply.send(Err(CoreError::InvalidOp(
                "demosaic applies to RAW only".into(),
            )));
            return;
        }
        let Some(demosaic) = crate::raw::Demosaic::from_name(&algo) else {
            let _ = reply.send(Err(CoreError::InvalidOp(format!(
                "unknown demosaic algorithm: {algo}"
            ))));
            return;
        };
        let available = crate::raw::Demosaic::available();
        if !available.iter().any(|n| n == demosaic.name()) {
            let _ = reply.send(Err(CoreError::InvalidOp(format!(
                "demosaic '{}' unavailable — sidecar worker not found on this system \
                 (in-process merawler algos still work)",
                demosaic.name()
            ))));
            return;
        }
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
            cur.meta.available_demosaic = available;
            let profile_file = cur.doc().meta.profile_file.clone();
            let index = crate::profile::ProfileIndex::embedded();
            let profile_path = crate::profile::decode_profile_path(
                crate::profile::choose_profile(&cur.meta, &index, profile_file.as_deref()).as_ref(),
            );
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
pub(super) fn cpu_preview_frame(
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
