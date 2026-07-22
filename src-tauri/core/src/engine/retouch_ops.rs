//! Object removal: rebuild the working master from the clean buffer + heal spots.

use super::Engine;
use crate::gpu::display::upload_working_texture;
use crate::image::RgbF32Buf;
use crate::message::EngineEvent;
use crate::retouch;
use std::sync::Arc;

impl Engine {
    /// Re-apply all enabled retouch spots onto `clean_rgb` and upload as working.
    /// No-op when there is no clean master yet (decode still running).
    pub(super) fn rebuild_retouch(&mut self) {
        let (clean, spots) = {
            let Some(cur) = self.current.as_ref() else {
                return;
            };
            let Some(clean) = cur.clean_rgb.clone() else {
                return;
            };
            (clean, cur.doc().retouch.clone())
        };

        let out = match retouch::apply_all(&clean, &spots) {
            Ok(buf) => buf,
            Err(e) => {
                tracing::warn!(error = %e, "retouch rebuild failed");
                self.emit(EngineEvent::RetouchError {
                    message: e.to_string(),
                });
                return;
            }
        };

        self.upload_working_rgb(out);
        self.emit(EngineEvent::RetouchDone);
    }

    /// Upload an RGB buffer as the working master (keeps `clean_rgb` untouched).
    pub(super) fn upload_working_rgb(&mut self, buf: RgbF32Buf) {
        let width = buf.width as u32;
        let height = buf.height as u32;
        let small = Arc::new(buf.downscale_to(2048));
        match &self.gpu {
            Some(gpu) => {
                let bytes = buf.to_rgba_f16_bytes();
                let tex = upload_working_texture(gpu, &bytes, width, height);
                let view = tex.create_view(&Default::default());
                if let Some(cur) = &mut self.current {
                    cur.working = Some((tex, view, width, height));
                    cur.small_cpu = Some(small);
                }
            }
            None => {
                if let Some(cur) = &mut self.current {
                    cur.small_cpu = Some(small);
                }
            }
        }
    }
}
