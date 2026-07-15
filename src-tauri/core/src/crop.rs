//! Non-destructive crop metadata (Lightroom-style).

use crate::doc::EditDoc;
use crate::registry::effective_f32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CropParams {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub angle: f32,
    pub rotate_90: u32,
    pub flip_h: bool,
    pub flip_v: bool,
    pub aspect_locked: bool,
    pub aspect_w: f32,
    pub aspect_h: f32,
    pub constrain_crop: bool,
}

impl Default for CropParams {
    fn default() -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            right: 1.0,
            bottom: 1.0,
            angle: 0.0,
            rotate_90: 0,
            flip_h: false,
            flip_v: false,
            aspect_locked: false,
            aspect_w: 0.0,
            aspect_h: 0.0,
            constrain_crop: true,
        }
    }
}

impl CropParams {
    pub fn from_doc(doc: &EditDoc) -> Self {
        let b = |p: &str| effective_f32(doc, "crop", p) >= 0.5;
        Self {
            left: effective_f32(doc, "crop", "left"),
            top: effective_f32(doc, "crop", "top"),
            right: effective_f32(doc, "crop", "right"),
            bottom: effective_f32(doc, "crop", "bottom"),
            angle: effective_f32(doc, "crop", "angle"),
            rotate_90: effective_f32(doc, "crop", "rotate_90").round().clamp(0.0, 3.0) as u32,
            flip_h: b("flip_h"),
            flip_v: b("flip_v"),
            aspect_locked: b("aspect_locked"),
            aspect_w: effective_f32(doc, "crop", "aspect_w"),
            aspect_h: effective_f32(doc, "crop", "aspect_h"),
            constrain_crop: b("constrain_crop"),
        }
    }

    pub fn is_identity(&self) -> bool {
        self.left <= 0.001
            && self.top <= 0.001
            && self.right >= 0.999
            && self.bottom >= 0.999
            && self.angle.abs() < 0.001
            && self.rotate_90 == 0
            && !self.flip_h
            && !self.flip_v
    }

    /// Angle/rotate-90/flip only (rect ignored).
    pub fn geometry_identity(&self) -> bool {
        self.angle.abs() < 0.001 && self.rotate_90 == 0 && !self.flip_h && !self.flip_v
    }

    /// Extract-shader crop mode: 0 = none, 1 = full crop (rect + geometry),
    /// 2 = geometry only (crop-tool editing preview: the image rotates/flips
    /// live but the full frame stays visible; the rect is drawn by the UI).
    pub fn mode(&self, crop_preview: bool) -> u32 {
        if !crop_preview && !self.is_identity() {
            1
        } else if crop_preview && !self.geometry_identity() {
            2
        } else {
            0
        }
    }

    /// Physical pixel dims of the post-rotate-90 image space (the space the
    /// crop rect and straighten angle live in).
    pub fn rotated_dims(&self, img_w: u32, img_h: u32) -> (f32, f32) {
        if self.rotate_90 % 2 == 1 {
            (img_h as f32, img_w as f32)
        } else {
            (img_w as f32, img_h as f32)
        }
    }

    /// Pixel dims of the displayed content for a given crop mode:
    /// mode 0 = full image, 1 = crop rect region, 2 = rotated full image.
    pub fn content_dims(&self, img_w: u32, img_h: u32, mode: u32) -> (f32, f32) {
        let (rw, rh) = self.rotated_dims(img_w, img_h);
        match mode {
            1 => (
                ((self.right - self.left).max(0.01) * rw).max(1.0),
                ((self.bottom - self.top).max(0.01) * rh).max(1.0),
            ),
            2 => (rw, rh),
            _ => (img_w as f32, img_h as f32),
        }
    }

    /// Cache key for extract pass invalidation.
    /// In crop-preview (tool open) the rect is drawn by the UI overlay, so
    /// only geometry (angle/flip/rotate90) invalidates the GPU extract pass.
    pub fn signature(&self, crop_preview: bool) -> u64 {
        let mut h = 0u64;
        let mix = |h: &mut u64, v: u32| {
            *h = (*h).wrapping_mul(31).wrapping_add(v as u64);
        };
        mix(&mut h, if crop_preview { 1 } else { 0 });
        if !crop_preview {
            for v in [
                self.left.to_bits(),
                self.top.to_bits(),
                self.right.to_bits(),
                self.bottom.to_bits(),
            ] {
                mix(&mut h, v);
            }
        }
        for v in [
            self.angle.to_bits(),
            self.rotate_90,
            self.flip_h as u32,
            self.flip_v as u32,
        ] {
            mix(&mut h, v);
        }
        h
    }

    /// Pixel dims of the cropped output (rotate-90 aware: the rect is
    /// normalized in post-rotation space, so a 90° turn swaps the axes the
    /// rect scales against).
    pub fn effective_size(&self, img_w: u32, img_h: u32) -> (f32, f32) {
        self.content_dims(img_w, img_h, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{EditDoc, ParamValue};

    #[test]
    fn default_crop_is_identity() {
        let doc = EditDoc::new("/x.ARW");
        let c = CropParams::from_doc(&doc);
        assert!(c.is_identity());
    }

    #[test]
    fn partial_crop_not_identity() {
        let mut doc = EditDoc::new("/x.ARW");
        doc.set("crop", "right", ParamValue::F32(0.8));
        assert!(!CropParams::from_doc(&doc).is_identity());
    }

    #[test]
    fn mode_selection() {
        let mut c = CropParams::default();
        assert_eq!(c.mode(false), 0);
        assert_eq!(c.mode(true), 0);
        c.right = 0.8;
        assert_eq!(c.mode(false), 1);
        assert_eq!(c.mode(true), 0); // rect-only crop: editing preview shows plain full frame
        c.angle = 2.0;
        assert_eq!(c.mode(true), 2); // geometry present: editing preview rotates live
    }

    #[test]
    fn rotate90_swaps_content_dims() {
        let mut c = CropParams {
            left: 0.0,
            top: 0.0,
            right: 0.5,
            bottom: 1.0,
            ..CropParams::default()
        };
        // 6000×4000, rect covers left half of rotated space
        let (w, h) = c.effective_size(6000, 4000);
        assert_eq!((w.round() as u32, h.round() as u32), (3000, 4000));
        c.rotate_90 = 1; // rotated space is 4000×6000
        let (w, h) = c.effective_size(6000, 4000);
        assert_eq!((w.round() as u32, h.round() as u32), (2000, 6000));
        let (rw, rh) = c.content_dims(6000, 4000, 2);
        assert_eq!((rw as u32, rh as u32), (4000, 6000));
    }
}
