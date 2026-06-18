//! CPU image buffer ops: orientation bake, downscale, f32→f16 packing.

use rawler::Orientation;

/// Planar-interleaved RGB f32 buffer (3 channels).
pub struct RgbF32Buf {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>, // len = w*h*3
}

impl RgbF32Buf {
    /// Bake EXIF orientation so downstream is always upright (spec 1.7).
    pub fn bake_orientation(self, o: Orientation) -> RgbF32Buf {
        let (w, h) = (self.width, self.height);
        let src = &self.data;
        // (new_w, new_h, fn(new x, new y) -> (old x, old y))
        let (nw, nh, map): (usize, usize, Box<dyn Fn(usize, usize) -> (usize, usize)>) = match o {
            Orientation::Normal | Orientation::Unknown => return self,
            Orientation::HorizontalFlip => (w, h, Box::new(move |x, y| (w - 1 - x, y))),
            Orientation::Rotate180 => (w, h, Box::new(move |x, y| (w - 1 - x, h - 1 - y))),
            Orientation::VerticalFlip => (w, h, Box::new(move |x, y| (x, h - 1 - y))),
            // Rotate90 = rotate image 90° CW to display upright
            Orientation::Rotate90 => (h, w, Box::new(move |x, y| (y, h - 1 - x))),
            Orientation::Rotate270 => (h, w, Box::new(move |x, y| (w - 1 - y, x))),
            Orientation::Transpose => (h, w, Box::new(move |x, y| (y, x))),
            Orientation::Transverse => (h, w, Box::new(move |x, y| (w - 1 - y, h - 1 - x))),
        };
        let mut out = vec![0.0f32; nw * nh * 3];
        for ny in 0..nh {
            for nx in 0..nw {
                let (ox, oy) = map(nx, ny);
                let si = (oy * w + ox) * 3;
                let di = (ny * nw + nx) * 3;
                out[di] = src[si];
                out[di + 1] = src[si + 1];
                out[di + 2] = src[si + 2];
            }
        }
        RgbF32Buf {
            width: nw,
            height: nh,
            data: out,
        }
    }

    /// Box-filter downscale so max dimension ≤ `max_dim`. For the retained
    /// CPU copy (histogram/fallback) — quality is adequate, speed matters.
    pub fn downscale_to(&self, max_dim: usize) -> RgbF32Buf {
        let scale = (self.width.max(self.height) as f32 / max_dim as f32).max(1.0);
        if scale <= 1.0 {
            return RgbF32Buf {
                width: self.width,
                height: self.height,
                data: self.data.clone(),
            };
        }
        let nw = (self.width as f32 / scale).round().max(1.0) as usize;
        let nh = (self.height as f32 / scale).round().max(1.0) as usize;
        let mut out = vec![0.0f32; nw * nh * 3];
        for ny in 0..nh {
            let y0 = (ny as f32 * scale) as usize;
            let y1 = (((ny + 1) as f32 * scale) as usize).min(self.height).max(y0 + 1);
            for nx in 0..nw {
                let x0 = (nx as f32 * scale) as usize;
                let x1 = (((nx + 1) as f32 * scale) as usize).min(self.width).max(x0 + 1);
                let mut acc = [0.0f32; 3];
                let n = ((y1 - y0) * (x1 - x0)) as f32;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let i = (y * self.width + x) * 3;
                        acc[0] += self.data[i];
                        acc[1] += self.data[i + 1];
                        acc[2] += self.data[i + 2];
                    }
                }
                let di = (ny * nw + nx) * 3;
                out[di] = acc[0] / n;
                out[di + 1] = acc[1] / n;
                out[di + 2] = acc[2] / n;
            }
        }
        RgbF32Buf {
            width: nw,
            height: nh,
            data: out,
        }
    }

    /// Pack RGB f32 → RGBA f16 bytes for texture upload (alpha = 1).
    pub fn to_rgba_f16_bytes(&self) -> Vec<u8> {
        let px = self.width * self.height;
        let mut out = vec![0u8; px * 8];
        let one = half::f16::ONE.to_le_bytes();
        for i in 0..px {
            let si = i * 3;
            let di = i * 8;
            out[di..di + 2]
                .copy_from_slice(&half::f16::from_f32(self.data[si]).to_le_bytes());
            out[di + 2..di + 4]
                .copy_from_slice(&half::f16::from_f32(self.data[si + 1]).to_le_bytes());
            out[di + 4..di + 6]
                .copy_from_slice(&half::f16::from_f32(self.data[si + 2]).to_le_bytes());
            out[di + 6..di + 8].copy_from_slice(&one);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf2x3() -> RgbF32Buf {
        // 2 wide, 3 tall; pixel value = x*10 + y in R channel
        let mut data = vec![0.0; 2 * 3 * 3];
        for y in 0..3 {
            for x in 0..2 {
                data[(y * 2 + x) * 3] = (x * 10 + y) as f32;
            }
        }
        RgbF32Buf {
            width: 2,
            height: 3,
            data,
        }
    }

    #[test]
    fn rotate90_maps_corners() {
        let b = buf2x3().bake_orientation(Orientation::Rotate90);
        assert_eq!((b.width, b.height), (3, 2));
        // old top-left (0,0) → new top-right (nw-1, 0)
        assert_eq!(b.data[(0 * 3 + 2) * 3], 0.0);
        // old bottom-left (0,2) → new top-left (0,0)
        assert_eq!(b.data[0], 2.0);
    }

    #[test]
    fn rotate180_maps_corners() {
        let b = buf2x3().bake_orientation(Orientation::Rotate180);
        assert_eq!((b.width, b.height), (2, 3));
        // old (0,0)=0 → new (1,2)
        assert_eq!(b.data[(2 * 2 + 1) * 3], 0.0);
    }

    #[test]
    fn downscale_halves() {
        let b = RgbF32Buf {
            width: 4,
            height: 4,
            data: vec![2.0; 4 * 4 * 3],
        };
        let d = b.downscale_to(2);
        assert_eq!((d.width, d.height), (2, 2));
        assert!(d.data.iter().all(|v| (*v - 2.0).abs() < 1e-6));
    }

    #[test]
    fn f16_pack_round_trips() {
        let b = RgbF32Buf {
            width: 1,
            height: 1,
            data: vec![0.5, 2.0, 0.25],
        };
        let bytes = b.to_rgba_f16_bytes();
        let r = half::f16::from_le_bytes([bytes[0], bytes[1]]).to_f32();
        let g = half::f16::from_le_bytes([bytes[2], bytes[3]]).to_f32();
        let a = half::f16::from_le_bytes([bytes[6], bytes[7]]).to_f32();
        assert_eq!(r, 0.5);
        assert_eq!(g, 2.0); // >1.0 headroom survives f16
        assert_eq!(a, 1.0);
    }
}
