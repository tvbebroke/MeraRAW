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

    pub fn apply_enabled(&self, crop_preview: bool) -> bool {
        !crop_preview && !self.is_identity()
    }

    /// Cache key for extract pass invalidation.
    pub fn signature(&self, crop_preview: bool) -> u64 {
        let mut h = 0u64;
        let mix = |h: &mut u64, v: u32| {
            *h = (*h).wrapping_mul(31).wrapping_add(v as u64);
        };
        mix(&mut h, if crop_preview { 1 } else { 0 });
        for v in [
            self.left.to_bits(),
            self.top.to_bits(),
            self.right.to_bits(),
            self.bottom.to_bits(),
            self.angle.to_bits(),
            self.rotate_90,
            self.flip_h as u32,
            self.flip_v as u32,
            self.aspect_w.to_bits(),
            self.aspect_h.to_bits(),
        ] {
            mix(&mut h, v);
        }
        h
    }

    pub fn effective_size(&self, img_w: u32, img_h: u32) -> (f32, f32) {
        let w = (self.right - self.left).max(0.01) * img_w as f32;
        let h = (self.bottom - self.top).max(0.01) * img_h as f32;
        (w, h)
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
}
