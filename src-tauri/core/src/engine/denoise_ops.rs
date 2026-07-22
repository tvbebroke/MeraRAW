//! AI denoise engine ops: start (full-res decode job), worker → engine
//! terminal handling (swap the working master), and reset (plain re-decode).

use super::Engine;
use crate::error::CoreError;
use crate::image::RgbF32Buf;
use crate::message::{DenoiseOutcome, EngineEvent};
use std::sync::Arc;
use tokio::sync::oneshot;

impl Engine {
    /// Start a full-res AI denoise job for the current image. The worker
    /// re-decodes at full resolution (the retained CPU copy is preview-sized),
    /// so the finished base can replace the working master 1:1.
    pub(super) fn denoise_ai_start(&mut self) -> Result<u64, CoreError> {
        let (hash, settings, path, profile_path, demosaic) = {
            let cur = self.current.as_ref().ok_or(CoreError::NoImage)?;
            let small = cur
                .small_cpu
                .as_ref()
                .ok_or_else(|| CoreError::Decode("image not fully decoded".into()))?;
            let settings = crate::denoise::DenoiseSettings::from_doc(
                cur.doc(),
                &crate::denoise::NoiseProfile::from_iso(cur.meta.iso.unwrap_or(800)),
            );
            // Content-addressed cache key (audit F6) — path hash alone
            // served stale results when the file was replaced in place.
            // The small copy is a deterministic function of the decode
            // (demosaic + profile included), so it identifies the base.
            let mut hasher = blake3::Hasher::new();
            hasher.update(&(small.width as u32).to_le_bytes());
            hasher.update(&(small.height as u32).to_le_bytes());
            hasher.update(bytemuck::cast_slice::<f32, u8>(&small.data));
            let hash = hasher.finalize().to_hex().to_string();

            let demosaic =
                crate::raw::Demosaic::parse_or_default(cur.doc().meta.demosaic.as_deref());
            let profile_file = cur.doc().meta.profile_file.clone();
            let index = crate::profile::ProfileIndex::embedded();
            let profile_path =
                crate::profile::choose_profile(&cur.meta, &index, profile_file.as_deref())
                    .map(|p| crate::profile::profile_path(&p.file));
            (hash, settings, cur.path.clone(), profile_path, demosaic)
        };

        let enqueued = self.ai_denoise.enqueue(
            &hash,
            &settings.ai_model,
            settings.ai_amount,
            crate::denoise::ai::JobSource::Decode {
                path: path.clone(),
                profile_path,
                demosaic,
            },
        )?;
        match enqueued {
            crate::denoise::ai::Enqueued::Started(id) => {
                self.pending_denoise = Some((id.0, path));
                Ok(id.0)
            }
            crate::denoise::ai::Enqueued::Cached(id, cache_path) => {
                // No worker, no notify — apply synchronously from disk.
                let (w, h, rgb) = crate::denoise::ai::CacheStore::load(&cache_path)?;
                self.apply_denoise_base(Arc::new(rgb), w as u32, h as u32);
                self.emit(EngineEvent::DenoiseDone { job: id.0 });
                Ok(id.0)
            }
        }
    }

    pub(super) fn finish_denoise(&mut self, job: u64, outcome: DenoiseOutcome) {
        // Only the job we started for the image that is still open may land.
        let expected = self.pending_denoise.take();
        let valid = match (&expected, self.current.as_ref()) {
            (Some((jid, jpath)), Some(cur)) => *jid == job && cur.path == *jpath,
            _ => false,
        };
        match outcome {
            DenoiseOutcome::Done { rgb, width, height } => {
                if !valid {
                    tracing::info!(job, "dropping stale denoise result (image switched)");
                    return;
                }
                self.apply_denoise_base(rgb, width, height);
                self.emit(EngineEvent::DenoiseDone { job });
            }
            DenoiseOutcome::Cancelled => {
                self.emit(EngineEvent::DenoiseError {
                    job,
                    message: "cancelled".into(),
                });
            }
            DenoiseOutcome::Failed { message } => {
                tracing::warn!(job, %message, "denoise job failed");
                self.emit(EngineEvent::DenoiseError { job, message });
            }
        }
    }

    /// Swap the denoised buffer in as the new working master (the
    /// `finish_decode` pattern: full graph invalidation + fit re-render).
    fn apply_denoise_base(&mut self, rgb: Arc<Vec<f32>>, width: u32, height: u32) {
        let buf = RgbF32Buf {
            width: width as usize,
            height: height as usize,
            data: Arc::try_unwrap(rgb).unwrap_or_else(|a| (*a).clone()),
        };
        // Denoise becomes the new clean master; heal spots re-apply on top.
        let clean = Arc::new(buf);
        if let Some(cur) = &mut self.current {
            cur.clean_rgb = Some(clean.clone());
        }
        self.upload_working_rgb(RgbF32Buf {
            width: clean.width,
            height: clean.height,
            data: clean.data.clone(),
        });
        self.rebuild_retouch();
        match &self.gpu {
            Some(_) => {
                self.rerender_after_base_change();
            }
            None => {
                let small = self
                    .current
                    .as_ref()
                    .and_then(|c| c.small_cpu.clone())
                    .expect("small_cpu set by upload_working_rgb");
                let vw = self
                    .last_view
                    .filter(|v| v.out_w >= 64 && v.out_h >= 64)
                    .map(|v| (v.out_w, v.out_h))
                    .unwrap_or((1440, 860));
                let mut frame = super::decode::cpu_preview_frame(
                    &small,
                    self.display_look == 1,
                    vw.0,
                    vw.1,
                );
                let version = self.next_version();
                frame.version = version;
                self.latest_frame = Some(frame);
                self.emit(EngineEvent::ImageReady { version });
                self.emit(EngineEvent::FrameReady { version });
            }
        }
    }

    /// AI Denoise unchecked: restore the plain decode as the working master.
    /// Mirrors `set_demosaic` (re-decode with the current settings; same
    /// generation so DecodeDone isn't discarded as stale).
    pub(super) fn denoise_ai_reset(
        &mut self,
        reply: oneshot::Sender<Result<(), CoreError>>,
    ) {
        self.ai_denoise.cancel_active();
        self.pending_denoise = None;
        let (path, profile_path, demosaic) = {
            let Some(cur) = self.current.as_ref() else {
                let _ = reply.send(Err(CoreError::NoImage));
                return;
            };
            let demosaic =
                crate::raw::Demosaic::parse_or_default(cur.doc().meta.demosaic.as_deref());
            let profile_file = cur.doc().meta.profile_file.clone();
            let index = crate::profile::ProfileIndex::embedded();
            let profile_path =
                crate::profile::choose_profile(&cur.meta, &index, profile_file.as_deref())
                    .map(|p| crate::profile::profile_path(&p.file));
            (cur.path.clone(), profile_path, demosaic)
        };
        self.spawn_full_decode(path, profile_path, demosaic, self.generation);
        let _ = reply.send(Ok(()));
    }
}
