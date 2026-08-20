//! IGV — Integrated Gaussian Vector demosaicing on color differences.
//!
//! Luis Sanz Rodríguez (2013), using the high-order interpolation of Jim S.,
//! Jimmy Li and Sharmil Randhawa. IGV is a strong color-difference method: it
//! forms directional green estimates (a 7-tap high-order interpolation blended
//! by Hamilton-Adams gradients), then denoises the horizontal/vertical color
//! differences with an "integrated Gaussian vector over variance" before fusing
//! them — which gives clean edges with little zippering. Red/blue are then
//! diffused diagonally and cardinally from the color differences.
//!
//! This is an original Rust implementation written from the published algorithm
//! (constants/offsets are mathematical facts), not a port of the GPL reference.
//! It runs whole-image with a bilinear-seeded 8px border. The reference works in
//! a 0..65535 scale (note the `/65535`, `/3145680 = 48·65535`, and `LIM(.,0,1)`
//! clamps); we scale in/out to match and copy the constants verbatim.

use super::bilinear::Bilinear;
use super::Demosaic;
use crate::image::{CfaImage, RgbImage};

pub struct Igv;

const EPS: f32 = 1e-5;
const EPSSQ: f32 = 1e-5; // reference raised this from 1e-10 to avoid div-by-zero artifacts
const SCALE: f32 = 65535.0;
/// Outer border filled by the bilinear seed (reference redoes 8px separately).
const BORDER: usize = 7;

#[inline(always)]
fn sqr(x: f32) -> f32 {
    x * x
}
#[inline(always)]
fn med3(a: f32, b: f32, c: f32) -> f32 {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    hi.min(lo.max(c))
}
#[inline(always)]
fn lim(x: f32, lo: f32, hi: f32) -> f32 {
    x.max(lo).min(hi)
}

impl Demosaic for Igv {
    fn demosaic(&self, img: &CfaImage) -> RgbImage {
        let (w, h) = (img.width, img.height);
        if w < 20 || h < 20 {
            return Bilinear.demosaic(img);
        }
        let mut out = Bilinear.demosaic(img); // seeds the 7px border + fallback
        let data = &img.data[..];
        let pattern = img.pattern;
        let fc = |row: usize, col: usize| pattern.color_at(col, row) as usize;

        let n = w * h;
        let (v1, v2, v3, v4, v5, v6) = (w, 2 * w, 3 * w, 4 * w, 5 * w, 6 * w);
        let (h1, h2, h3, h4, h5, h6) = (1usize, 2, 3, 4, 5, 6);

        // Scaled mosaic (measured value at every sensel) and green plane.
        let mut mos = vec![0.0f32; n];
        let mut g = vec![0.0f32; n];
        for row in 0..h {
            for col in 0..w {
                let i = row * w + col;
                mos[i] = data[i] * SCALE;
                if fc(row, col) == 1 {
                    g[i] = mos[i];
                }
            }
        }
        // Half-res directional color-difference buffers.
        let mut vdif = vec![0.0f32; n / 2 + w];
        let mut hdif = vec![0.0f32; n / 2 + w];
        // Full-res chrominance planes (chr0 = G-R diff, chr1 = G-B diff).
        let mut chr0 = vec![0.0f32; n];
        let mut chr1 = vec![0.0f32; n];

        // --- directional green + horizontal/vertical color differences ---
        for row in 5..h - 5 {
            let mut col = 5 + (fc(row, 1) & 1);
            while col < w - 5 {
                let i = row * w + col;
                let ng =
                    EPS + ((g[i - v1] - g[i - v3]).abs() + (mos[i] - mos[i - v2]).abs()) / SCALE;
                let eg =
                    EPS + ((g[i + h1] - g[i + h3]).abs() + (mos[i] - mos[i + h2]).abs()) / SCALE;
                let wg =
                    EPS + ((g[i - h1] - g[i - h3]).abs() + (mos[i] - mos[i - h2]).abs()) / SCALE;
                let sg =
                    EPS + ((g[i + v1] - g[i + v3]).abs() + (mos[i] - mos[i + v2]).abs()) / SCALE;
                // high-order interpolation (Li & Randhawa), 48*65535 = 3145680
                let nv = lim(
                    (23.0 * g[i - v1] + 23.0 * g[i - v3] + g[i - v5] + g[i + v1] + 40.0 * mos[i]
                        - 32.0 * mos[i - v2]
                        - 8.0 * mos[i - v4])
                        / 3145680.0,
                    0.0,
                    1.0,
                );
                let ev = lim(
                    (23.0 * g[i + h1] + 23.0 * g[i + h3] + g[i + h5] + g[i - h1] + 40.0 * mos[i]
                        - 32.0 * mos[i + h2]
                        - 8.0 * mos[i + h4])
                        / 3145680.0,
                    0.0,
                    1.0,
                );
                let wv = lim(
                    (23.0 * g[i - h1] + 23.0 * g[i - h3] + g[i - h5] + g[i + h1] + 40.0 * mos[i]
                        - 32.0 * mos[i - h2]
                        - 8.0 * mos[i - h4])
                        / 3145680.0,
                    0.0,
                    1.0,
                );
                let sv = lim(
                    (23.0 * g[i + v1] + 23.0 * g[i + v3] + g[i + v5] + g[i - v1] + 40.0 * mos[i]
                        - 32.0 * mos[i + v2]
                        - 8.0 * mos[i + v4])
                        / 3145680.0,
                    0.0,
                    1.0,
                );
                vdif[i >> 1] = (sg * nv + ng * sv) / (ng + sg) - mos[i] / SCALE;
                hdif[i >> 1] = (wg * ev + eg * wv) / (eg + wg) - mos[i] / SCALE;
                col += 2;
            }
        }

        // --- integrated-Gaussian-vector variance → green population + chr[d] ---
        for row in 7..h - 7 {
            let mut col = 7 + (fc(row, 1) & 1);
            while col < w - 7 {
                let i = row * w + col;
                let d = fc(row, col) / 2; // R->0, B->1
                let (vd, vd2m, vd2p, vd4m, vd4p, vd6m, vd6p) = (
                    vdif[i >> 1],
                    vdif[(i - v2) >> 1],
                    vdif[(i + v2) >> 1],
                    vdif[(i - v4) >> 1],
                    vdif[(i + v4) >> 1],
                    vdif[(i - v6) >> 1],
                    vdif[(i + v6) >> 1],
                );
                let ng = lim(
                    EPSSQ
                        + 78.0 * sqr(vd)
                        + 69.0 * (sqr(vd2m) + sqr(vd2p))
                        + 51.0 * (sqr(vd4m) + sqr(vd4p))
                        + 21.0 * (sqr(vd6m) + sqr(vd6p))
                        - 6.0 * sqr(vd2m + vd + vd2p)
                        - 10.0 * (sqr(vd4m + vd2m + vd) + sqr(vd + vd2p + vd4p))
                        - 7.0 * (sqr(vd6m + vd4m + vd2m) + sqr(vd2p + vd4p + vd6p)),
                    0.0,
                    1.0,
                );
                let (hd, hd2m, hd2p, hd4m, hd4p, hd6m, hd6p) = (
                    hdif[i >> 1],
                    hdif[(i - h2) >> 1],
                    hdif[(i + h2) >> 1],
                    hdif[(i - h4) >> 1],
                    hdif[(i + h4) >> 1],
                    hdif[(i - h6) >> 1],
                    hdif[(i + h6) >> 1],
                );
                let eg = lim(
                    EPSSQ
                        + 78.0 * sqr(hd)
                        + 69.0 * (sqr(hd2m) + sqr(hd2p))
                        + 51.0 * (sqr(hd4m) + sqr(hd4p))
                        + 21.0 * (sqr(hd6m) + sqr(hd6p))
                        - 6.0 * sqr(hd2m + hd + hd2p)
                        - 10.0 * (sqr(hd4m + hd2m + hd) + sqr(hd + hd2p + hd4p))
                        - 7.0 * (sqr(hd6m + hd4m + hd2m) + sqr(hd2p + hd4p + hd6p)),
                    0.0,
                    1.0,
                );
                // median-limited chrominance
                let nv = med3(0.725 * vd + 0.1375 * vd2m + 0.1375 * vd2p, vd2m, vd2p);
                let ev = med3(0.725 * hd + 0.1375 * hd2m + 0.1375 * hd2p, hd2m, hd2p);
                let chr = (eg * nv + ng * ev) / (ng + eg);
                if d == 0 {
                    chr0[i] = chr;
                } else {
                    chr1[i] = chr;
                }
                g[i] = mos[i] + SCALE * chr;
                col += 2;
            }
        }

        // --- diagonal chroma diffusion (R@B, B@R), two coset passes ---
        for start in [7usize, 8] {
            let mut row = start;
            while row < h - 7 {
                let mut col = 7 + (fc(row, 1) & 1);
                while col < w - 7 {
                    let i = row * w + col;
                    let c = 1 - fc(row, col) / 2; // opposite chroma plane
                    let ch = if c == 0 { &mut chr0 } else { &mut chr1 };
                    let nwg = 1.0
                        / (EPS
                            + (ch[i - v1 - h1] - ch[i - v3 - h3]).abs()
                            + (ch[i + v1 + h1] - ch[i - v3 - h3]).abs());
                    let neg = 1.0
                        / (EPS
                            + (ch[i - v1 + h1] - ch[i - v3 + h3]).abs()
                            + (ch[i + v1 - h1] - ch[i - v3 + h3]).abs());
                    let swg = 1.0
                        / (EPS
                            + (ch[i + v1 - h1] - ch[i + v3 + h3]).abs()
                            + (ch[i - v1 + h1] - ch[i + v3 - h3]).abs());
                    let seg = 1.0
                        / (EPS
                            + (ch[i + v1 + h1] - ch[i + v3 - h3]).abs()
                            + (ch[i - v1 - h1] - ch[i + v3 + h3]).abs());
                    let nwv = med3(ch[i - v1 - h1], ch[i - v3 - h1], ch[i - v1 - h3]);
                    let nev = med3(ch[i - v1 + h1], ch[i - v3 + h1], ch[i - v1 + h3]);
                    let swv = med3(ch[i + v1 - h1], ch[i + v3 - h1], ch[i + v1 - h3]);
                    let sev = med3(ch[i + v1 + h1], ch[i + v3 + h1], ch[i + v1 + h3]);
                    ch[i] =
                        (nwg * nwv + neg * nev + swg * swv + seg * sev) / (nwg + neg + swg + seg);
                    col += 2;
                }
                row += 2;
            }
        }

        // --- cardinal chroma diffusion at green sensels (chr0 then chr1) ---
        for (plane, chan) in [(0usize, 0usize), (1, 1)] {
            let _ = plane;
            let ch = if chan == 0 { &mut chr0 } else { &mut chr1 };
            for row in 7..h - 7 {
                let mut col = 7 + (fc(row, 0) & 1);
                while col < w - 7 {
                    let i = row * w + col;
                    let ng = 1.0
                        / (EPS + (ch[i - v1] - ch[i - v3]).abs() + (ch[i + v1] - ch[i - v3]).abs());
                    let eg = 1.0
                        / (EPS + (ch[i + h1] - ch[i + h3]).abs() + (ch[i - h1] - ch[i + h3]).abs());
                    let wg = 1.0
                        / (EPS + (ch[i - h1] - ch[i - h3]).abs() + (ch[i + h1] - ch[i - h3]).abs());
                    let sg = 1.0
                        / (EPS + (ch[i + v1] - ch[i + v3]).abs() + (ch[i - v1] - ch[i + v3]).abs());
                    ch[i] = (ng * ch[i - v1] + eg * ch[i + h1] + wg * ch[i - h1] + sg * ch[i + v1])
                        / (ng + eg + wg + sg);
                    col += 2;
                }
            }
        }

        // --- assemble ---
        for row in BORDER..h - BORDER {
            for col in BORDER..w - BORDER {
                let i = row * w + col;
                let green = g[i];
                let red = green - SCALE * chr0[i];
                let blue = green - SCALE * chr1[i];
                out.data[i] = [
                    (red / SCALE).max(0.0),
                    (green / SCALE).max(0.0),
                    (blue / SCALE).max(0.0),
                ];
            }
        }
        out
    }

    fn name(&self) -> &'static str {
        "igv"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demosaic::tests_common::{max_interior_error, mosaic_from_fn};
    use crate::image::CfaPattern;

    #[test]
    fn constant_field_reconstructs() {
        let cfa = mosaic_from_fn(48, 40, CfaPattern::Rggb, |_, _| 0.5);
        let rgb = Igv.demosaic(&cfa);
        for px in &rgb.data {
            for c in px {
                assert!((c - 0.5).abs() < 2e-3, "got {c}");
            }
        }
    }

    #[test]
    fn linear_gradient_low_error() {
        let grad = |x: usize, y: usize| 0.2 + 0.003 * x as f32 + 0.002 * y as f32;
        let cfa = mosaic_from_fn(64, 56, CfaPattern::Rggb, grad);
        let rgb = Igv.demosaic(&cfa);
        let err = max_interior_error(&rgb, 8, grad);
        assert!(err < 2e-2, "max interior error {err}");
    }

    #[test]
    fn all_patterns_reconstruct_constant() {
        for p in [
            CfaPattern::Rggb,
            CfaPattern::Bggr,
            CfaPattern::Grbg,
            CfaPattern::Gbrg,
        ] {
            let cfa = mosaic_from_fn(40, 40, p, |_, _| 0.4);
            let rgb = Igv.demosaic(&cfa);
            for px in &rgb.data {
                for c in px {
                    assert!((c - 0.4).abs() < 2e-3, "pattern {:?} got {c}", p);
                }
            }
        }
    }
}
