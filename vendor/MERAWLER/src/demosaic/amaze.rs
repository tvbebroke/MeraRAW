//! AMaZE — Aliasing Minimization and Zipper Elimination (Emil Martinec, 2010).
//!
//! AMaZE is the maximum-detail demosaicer, prized by landscape and astro
//! shooters. It is by far the most intricate algorithm in this crate. The flow:
//!
//! 1. Horizontal/vertical gradient weights (`dirwts`).
//! 2. Directional green via Hamilton-Adams *and* adaptive color-ratios, kept as
//!    color differences (`hcd`/`vcd`) with alternates (`hcdalt`/`vcdalt`).
//! 3. Pick the lower-variance interpolation, then bound it in saturated regions
//!    using medians.
//! 4. Directional-discrimination weight `hvwt` (variance + fluctuation based).
//! 5. **Nyquist texture test**: flag high-frequency/moiré regions and use a
//!    dedicated area interpolation + curvature refinement there.
//! 6. Populate green at red/blue sensels.
//! 7. Diagonal R+B interpolation (`rbp`/`rbm`), then refine green from R+B.
//! 8. "Fancy" chrominance interpolation of the G-R / G-B differences.
//! 9. Assemble R/G/B.
//!
//! This is an original Rust implementation written from the published algorithm
//! (the coefficients and neighbor offsets are mathematical facts), not a
//! line-by-line port of the GPL reference. Like that reference it processes the
//! image in overlapping 160×160 tiles (a 16px halo per side is scratch), which
//! keeps memory bounded and the checkerboard `>>1` buffer packing intact.
//!
//! Scope note: the reference's SIMD paths are ignored; the scalar path is ported.

use crate::image::{CfaImage, RgbImage};
use super::bilinear::Bilinear;
use super::Demosaic;

pub struct Amaze;

// Algorithm constants (see the paper / reference).
const EPS: f32 = 1e-5;
const EPSSQ: f32 = 1e-10;
const ARTHRESH: f32 = 0.75;
const CLIP_PT: f32 = 1.0; // = 1/initGain with initGain = 1
const CLIP_PT8: f32 = 0.8;
const NYQTHRESH: f32 = 0.5;
// gaussian on 5x5 quincunx, sigma=1.2
const GAUSSODD: [f32; 4] = [
    0.14659727707323927,
    0.103592713382435,
    0.0732036125103057,
    0.0365543548389495,
];
// gaussian on 5x5, pre-multiplied by NYQTHRESH
const GAUSSGRAD: [f32; 6] = [
    NYQTHRESH * 0.07384411893421103,
    NYQTHRESH * 0.06207511968171489,
    NYQTHRESH * 0.0521818194747806,
    NYQTHRESH * 0.03687419286733595,
    NYQTHRESH * 0.03099732204057846,
    NYQTHRESH * 0.018413194161458882,
];
// gaussian on 5x5 alt quincunx, sigma=1.5
const GAUSSEVEN: [f32; 2] = [0.13719494435797422, 0.05640252782101291];
// gaussian on quincunx grid
const GQUINC: [f32; 4] = [0.169917, 0.108947, 0.069855, 0.0287182];

const TS: usize = 160; // tile size

#[inline(always)]
fn sqr(x: f32) -> f32 {
    x * x
}
#[inline(always)]
fn med3(a: f32, b: f32, c: f32) -> f32 {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    hi.min(lo.max(c))
}
/// Linear interpolation, matching the reference `intp(a, b, c) = a*b + (1-a)*c`.
#[inline(always)]
fn intp(a: f32, b: f32, c: f32) -> f32 {
    a * (b - c) + c
}

impl Demosaic for Amaze {
    fn demosaic(&self, img: &CfaImage) -> RgbImage {
        let (w, h) = (img.width, img.height);
        // Too small for the 16px halo + algorithm reach: fall back.
        if w < 64 || h < 64 {
            return Bilinear.demosaic(img);
        }

        // Bilinear seed guarantees a value everywhere (AMaZE fills the interior).
        let mut out = Bilinear.demosaic(img);
        let raw = &img.data[..];
        let pattern = img.pattern;
        // CFA color at absolute pixel; tile offsets (top/left) are even, so tile
        // parity equals absolute parity and we can index by tile coords.
        let fc = |rr: isize, cc: isize| pattern.color_at(cc as usize, rr as usize) as usize;

        // (ey,ex) = offset of the red sensel within the quartet.
        let (ex, ey): (usize, usize) = if fc(0, 0) == 1 {
            if fc(0, 1) == 0 {
                (1, 0)
            } else {
                (0, 1)
            }
        } else if fc(0, 0) == 0 {
            (0, 0)
        } else {
            (1, 1)
        };

        // Clamped raw fetch in image coordinates.
        let rawat = |row: isize, col: isize| -> f32 {
            let r = row.clamp(0, h as isize - 1) as usize;
            let c = col.clamp(0, w as isize - 1) as usize;
            raw[r * w + c]
        };

        let (wi, hi) = (w as isize, h as isize);
        let v1 = TS;
        let v2 = 2 * TS;
        let v3 = 3 * TS;

        // Per-tile scratch (reallocated per tile, zero-initialized).
        let fsz = TS * TS;
        let hsz = TS * TS / 2 + 4 * TS;

        let mut top = -16isize;
        while top < hi {
            let mut left = -16isize;
            while left < wi {
                let bottom = (top + TS as isize).min(hi + 16);
                let right = (left + TS as isize).min(wi + 16);
                let rr1 = (bottom - top) as usize;
                let cc1 = (right - left) as usize;
                let rrmin = if top < 0 { 16 } else { 0 };
                let ccmin = if left < 0 { 16 } else { 0 };
                let rrmax = if bottom > hi { (hi - top) as usize } else { rr1 };
                let ccmax = if right > wi { (wi - left) as usize } else { cc1 };

                // --- tile load with mirrored halo (phase-preserving, offsets even) ---
                let mut cfa = vec![0.0f32; fsz];
                // inner
                for rr in rrmin..rrmax {
                    let row = top + rr as isize;
                    for cc in ccmin..ccmax {
                        cfa[rr * TS + cc] = rawat(row, left + cc as isize);
                    }
                }
                // upper / lower borders
                if rrmin > 0 {
                    for rr in 0..16 {
                        let row = 32 - rr as isize + top;
                        for cc in ccmin..ccmax {
                            cfa[rr * TS + cc] = rawat(row, left + cc as isize);
                        }
                    }
                }
                if rrmax < rr1 {
                    for rr in 0..16 {
                        for cc in ccmin..ccmax {
                            cfa[(rrmax + rr) * TS + cc] =
                                rawat(hi - rr as isize - 2, left + cc as isize);
                        }
                    }
                }
                // left / right borders
                if ccmin > 0 {
                    for rr in rrmin..rrmax {
                        let row = top + rr as isize;
                        for cc in 0..16 {
                            cfa[rr * TS + cc] = rawat(row, 32 - cc as isize + left);
                        }
                    }
                }
                if ccmax < cc1 {
                    for rr in rrmin..rrmax {
                        for cc in 0..16 {
                            cfa[rr * TS + ccmax + cc] =
                                rawat(top + rr as isize, wi - cc as isize - 2);
                        }
                    }
                }
                // corners
                if rrmin > 0 && ccmin > 0 {
                    for rr in 0..16 {
                        for cc in 0..16 {
                            cfa[rr * TS + cc] = rawat(32 - rr as isize, 32 - cc as isize);
                        }
                    }
                }
                if rrmax < rr1 && ccmax < cc1 {
                    for rr in 0..16 {
                        for cc in 0..16 {
                            cfa[(rrmax + rr) * TS + ccmax + cc] =
                                rawat(hi - rr as isize - 2, wi - cc as isize - 2);
                        }
                    }
                }
                if rrmin > 0 && ccmax < cc1 {
                    for rr in 0..16 {
                        for cc in 0..16 {
                            cfa[rr * TS + ccmax + cc] =
                                rawat(32 - rr as isize, wi - cc as isize - 2);
                        }
                    }
                }
                if rrmax < rr1 && ccmin > 0 {
                    for rr in 0..16 {
                        for cc in 0..16 {
                            cfa[(rrmax + rr) * TS + cc] =
                                rawat(hi - rr as isize - 2, 32 - cc as isize);
                        }
                    }
                }

                let mut rgbgreen = cfa.clone();

                // scratch buffers
                let mut dirwts0 = vec![0.0f32; fsz];
                let mut dirwts1 = vec![0.0f32; fsz];
                let mut delhvsqsum = vec![0.0f32; fsz];
                let mut vcd = vec![0.0f32; fsz];
                let mut hcd = vec![0.0f32; fsz];
                let mut vcdalt = vec![0.0f32; fsz];
                let mut hcdalt = vec![0.0f32; fsz];
                let mut cddiffsq = vec![0.0f32; fsz];
                let mut dgintv = vec![0.0f32; fsz];
                let mut dginth = vec![0.0f32; fsz];
                let mut hvwt = vec![0.0f32; hsz];
                let mut dgrb0 = vec![0.0f32; hsz];
                let mut dgrb1 = vec![0.0f32; hsz];
                let mut dgrb2h = vec![0.0f32; hsz];
                let mut dgrb2v = vec![0.0f32; hsz];
                let mut delp = vec![0.0f32; hsz];
                let mut delm = vec![0.0f32; hsz];
                let mut dgrbsq1p = vec![0.0f32; hsz];
                let mut dgrbsq1m = vec![0.0f32; hsz];
                let mut rbm = vec![0.0f32; hsz];
                let mut rbp = vec![0.0f32; hsz];
                let mut pmwt = vec![0.0f32; hsz];
                let mut rbint = vec![0.0f32; hsz];
                let mut nyqutest = vec![0.0f32; hsz];
                let mut nyquist = vec![0u8; hsz];
                let mut nyquist2 = vec![0u8; hsz];

                // --- horizontal/vertical gradients ---
                for rr in 2..rr1 - 2 {
                    for cc in 2..cc1 - 2 {
                        let indx = rr * TS + cc;
                        let delh = (cfa[indx + 1] - cfa[indx - 1]).abs();
                        let delv = (cfa[indx + v1] - cfa[indx - v1]).abs();
                        dirwts0[indx] = EPS
                            + (cfa[indx + v2] - cfa[indx]).abs()
                            + (cfa[indx] - cfa[indx - v2]).abs()
                            + delv;
                        dirwts1[indx] = EPS
                            + (cfa[indx + 2] - cfa[indx]).abs()
                            + (cfa[indx] - cfa[indx - 2]).abs()
                            + delh;
                        delhvsqsum[indx] = sqr(delh) + sqr(delv);
                    }
                }

                // --- directional green (Hamilton-Adams + adaptive ratios) ---
                for rr in 4..rr1 - 4 {
                    for cc in 4..cc1 - 4 {
                        let indx = rr * TS + cc;
                        let cfai = cfa[indx];
                        let cru = cfa[indx - v1] * (dirwts0[indx - v2] + dirwts0[indx])
                            / (dirwts0[indx - v2] * (EPS + cfai)
                                + dirwts0[indx] * (EPS + cfa[indx - v2]));
                        let crd = cfa[indx + v1] * (dirwts0[indx + v2] + dirwts0[indx])
                            / (dirwts0[indx + v2] * (EPS + cfai)
                                + dirwts0[indx] * (EPS + cfa[indx + v2]));
                        let crl = cfa[indx - 1] * (dirwts1[indx - 2] + dirwts1[indx])
                            / (dirwts1[indx - 2] * (EPS + cfai)
                                + dirwts1[indx] * (EPS + cfa[indx - 2]));
                        let crr = cfa[indx + 1] * (dirwts1[indx + 2] + dirwts1[indx])
                            / (dirwts1[indx + 2] * (EPS + cfai)
                                + dirwts1[indx] * (EPS + cfa[indx + 2]));

                        let guha = cfa[indx - v1] + 0.5 * (cfai - cfa[indx - v2]);
                        let gdha = cfa[indx + v1] + 0.5 * (cfai - cfa[indx + v2]);
                        let glha = cfa[indx - 1] + 0.5 * (cfai - cfa[indx - 2]);
                        let grha = cfa[indx + 1] + 0.5 * (cfai - cfa[indx + 2]);

                        let mut guar = if (1.0 - cru).abs() < ARTHRESH { cfai * cru } else { guha };
                        let mut gdar = if (1.0 - crd).abs() < ARTHRESH { cfai * crd } else { gdha };
                        let mut glar = if (1.0 - crl).abs() < ARTHRESH { cfai * crl } else { glha };
                        let mut grar = if (1.0 - crr).abs() < ARTHRESH { cfai * crr } else { grha };

                        let hwt = dirwts1[indx - 1] / (dirwts1[indx - 1] + dirwts1[indx + 1]);
                        let vwt = dirwts0[indx - v1] / (dirwts0[indx + v1] + dirwts0[indx - v1]);

                        let gintvha = vwt * gdha + (1.0 - vwt) * guha;
                        let ginthha = hwt * grha + (1.0 - hwt) * glha;

                        // sign of the color difference: G - C at green sites, C - G at R/B
                        let green_site = fc(rr as isize, cc as isize) == 1;
                        if green_site {
                            vcd[indx] = cfai - (vwt * gdar + (1.0 - vwt) * guar);
                            hcd[indx] = cfai - (hwt * grar + (1.0 - hwt) * glar);
                            vcdalt[indx] = cfai - gintvha;
                            hcdalt[indx] = cfai - ginthha;
                        } else {
                            vcd[indx] = (vwt * gdar + (1.0 - vwt) * guar) - cfai;
                            hcd[indx] = (hwt * grar + (1.0 - hwt) * glar) - cfai;
                            vcdalt[indx] = gintvha - cfai;
                            hcdalt[indx] = ginthha - cfai;
                        }

                        if cfai > CLIP_PT8 || gintvha > CLIP_PT8 || ginthha > CLIP_PT8 {
                            guar = guha;
                            gdar = gdha;
                            glar = glha;
                            grar = grha;
                            vcd[indx] = vcdalt[indx];
                            hcd[indx] = hcdalt[indx];
                        }

                        dgintv[indx] = sqr(guha - gdha).min(sqr(guar - gdar));
                        dginth[indx] = sqr(glha - grha).min(sqr(glar - grar));
                    }
                }

                // --- choose smaller-variance interpolation + saturation bounding ---
                for rr in 4..rr1 - 4 {
                    for cc in 4..cc1 - 4 {
                        let indx = rr * TS + cc;
                        let hcdvar = 3.0 * (sqr(hcd[indx - 2]) + sqr(hcd[indx]) + sqr(hcd[indx + 2]))
                            - sqr(hcd[indx - 2] + hcd[indx] + hcd[indx + 2]);
                        let hcdaltvar = 3.0
                            * (sqr(hcdalt[indx - 2]) + sqr(hcdalt[indx]) + sqr(hcdalt[indx + 2]))
                            - sqr(hcdalt[indx - 2] + hcdalt[indx] + hcdalt[indx + 2]);
                        let vcdvar = 3.0 * (sqr(vcd[indx - v2]) + sqr(vcd[indx]) + sqr(vcd[indx + v2]))
                            - sqr(vcd[indx - v2] + vcd[indx] + vcd[indx + v2]);
                        let vcdaltvar = 3.0
                            * (sqr(vcdalt[indx - v2]) + sqr(vcdalt[indx]) + sqr(vcdalt[indx + v2]))
                            - sqr(vcdalt[indx - v2] + vcdalt[indx] + vcdalt[indx + v2]);
                        if hcdaltvar < hcdvar {
                            hcd[indx] = hcdalt[indx];
                        }
                        if vcdaltvar < vcdvar {
                            vcd[indx] = vcdalt[indx];
                        }

                        let cfai = cfa[indx];
                        if fc(rr as isize, cc as isize) == 1 {
                            // green site
                            let ginth = -hcd[indx] + cfai;
                            let gintv = -vcd[indx] + cfai;
                            if hcd[indx] > 0.0 {
                                if 3.0 * hcd[indx] > (ginth + cfai) {
                                    hcd[indx] = -med3(ginth, cfa[indx - 1], cfa[indx + 1]) + cfai;
                                } else {
                                    let hwt = 1.0 - 3.0 * hcd[indx] / (EPS + ginth + cfai);
                                    hcd[indx] = hwt * hcd[indx]
                                        + (1.0 - hwt)
                                            * (-med3(ginth, cfa[indx - 1], cfa[indx + 1]) + cfai);
                                }
                            }
                            if vcd[indx] > 0.0 {
                                if 3.0 * vcd[indx] > (gintv + cfai) {
                                    vcd[indx] = -med3(gintv, cfa[indx - v1], cfa[indx + v1]) + cfai;
                                } else {
                                    let vwt = 1.0 - 3.0 * vcd[indx] / (EPS + gintv + cfai);
                                    vcd[indx] = vwt * vcd[indx]
                                        + (1.0 - vwt)
                                            * (-med3(gintv, cfa[indx - v1], cfa[indx + v1]) + cfai);
                                }
                            }
                            if ginth > CLIP_PT {
                                hcd[indx] = -med3(ginth, cfa[indx - 1], cfa[indx + 1]) + cfai;
                            }
                            if gintv > CLIP_PT {
                                vcd[indx] = -med3(gintv, cfa[indx - v1], cfa[indx + v1]) + cfai;
                            }
                        } else {
                            // red/blue site
                            let ginth = hcd[indx] + cfai;
                            let gintv = vcd[indx] + cfai;
                            if hcd[indx] < 0.0 {
                                if 3.0 * hcd[indx] < -(ginth + cfai) {
                                    hcd[indx] = med3(ginth, cfa[indx - 1], cfa[indx + 1]) - cfai;
                                } else {
                                    let hwt = 1.0 + 3.0 * hcd[indx] / (EPS + ginth + cfai);
                                    hcd[indx] = hwt * hcd[indx]
                                        + (1.0 - hwt)
                                            * (med3(ginth, cfa[indx - 1], cfa[indx + 1]) - cfai);
                                }
                            }
                            if vcd[indx] < 0.0 {
                                if 3.0 * vcd[indx] < -(gintv + cfai) {
                                    vcd[indx] = med3(gintv, cfa[indx - v1], cfa[indx + v1]) - cfai;
                                } else {
                                    let vwt = 1.0 + 3.0 * vcd[indx] / (EPS + gintv + cfai);
                                    vcd[indx] = vwt * vcd[indx]
                                        + (1.0 - vwt)
                                            * (med3(gintv, cfa[indx - v1], cfa[indx + v1]) - cfai);
                                }
                            }
                            if ginth > CLIP_PT {
                                hcd[indx] = med3(ginth, cfa[indx - 1], cfa[indx + 1]) - cfai;
                            }
                            if gintv > CLIP_PT {
                                vcd[indx] = med3(gintv, cfa[indx - v1], cfa[indx + v1]) - cfai;
                            }
                            cddiffsq[indx] = sqr(vcd[indx] - hcd[indx]);
                        }
                    }
                }

                // --- adaptive H/V discrimination weight (hvwt, at R/B sites) ---
                for rr in 6..rr1 - 6 {
                    let cc0 = 6 + (fc(rr as isize, 2) & 1);
                    let mut cc = cc0;
                    while cc < cc1 - 6 {
                        let indx = rr * TS + cc;
                        let uave = vcd[indx] + vcd[indx - v1] + vcd[indx - v2] + vcd[indx - v3];
                        let dave = vcd[indx] + vcd[indx + v1] + vcd[indx + v2] + vcd[indx + v3];
                        let lave = hcd[indx] + hcd[indx - 1] + hcd[indx - 2] + hcd[indx - 3];
                        let rave = hcd[indx] + hcd[indx + 1] + hcd[indx + 2] + hcd[indx + 3];
                        let dgrbvvaru = sqr(vcd[indx] - uave)
                            + sqr(vcd[indx - v1] - uave)
                            + sqr(vcd[indx - v2] - uave)
                            + sqr(vcd[indx - v3] - uave);
                        let dgrbvvard = sqr(vcd[indx] - dave)
                            + sqr(vcd[indx + v1] - dave)
                            + sqr(vcd[indx + v2] - dave)
                            + sqr(vcd[indx + v3] - dave);
                        let dgrbhvarl = sqr(hcd[indx] - lave)
                            + sqr(hcd[indx - 1] - lave)
                            + sqr(hcd[indx - 2] - lave)
                            + sqr(hcd[indx - 3] - lave);
                        let dgrbhvarr = sqr(hcd[indx] - rave)
                            + sqr(hcd[indx + 1] - rave)
                            + sqr(hcd[indx + 2] - rave)
                            + sqr(hcd[indx + 3] - rave);
                        let hwt = dirwts1[indx - 1] / (dirwts1[indx - 1] + dirwts1[indx + 1]);
                        let vwt = dirwts0[indx - v1] / (dirwts0[indx + v1] + dirwts0[indx - v1]);
                        let vcdvar = EPSSQ + vwt * dgrbvvard + (1.0 - vwt) * dgrbvvaru;
                        let hcdvar = EPSSQ + hwt * dgrbhvarr + (1.0 - hwt) * dgrbhvarl;

                        let dgrbvvaru2 = dgintv[indx] + dgintv[indx - v1] + dgintv[indx - v2];
                        let dgrbvvard2 = dgintv[indx] + dgintv[indx + v1] + dgintv[indx + v2];
                        let dgrbhvarl2 = dginth[indx] + dginth[indx - 1] + dginth[indx - 2];
                        let dgrbhvarr2 = dginth[indx] + dginth[indx + 1] + dginth[indx + 2];
                        let vcdvar1 = EPSSQ + vwt * dgrbvvard2 + (1.0 - vwt) * dgrbvvaru2;
                        let hcdvar1 = EPSSQ + hwt * dgrbhvarr2 + (1.0 - hwt) * dgrbhvarl2;

                        let varwt = hcdvar / (vcdvar + hcdvar);
                        let diffwt = hcdvar1 / (vcdvar1 + hcdvar1);
                        hvwt[indx >> 1] = if (0.5 - varwt) * (0.5 - diffwt) > 0.0
                            && (0.5 - diffwt).abs() < (0.5 - varwt).abs()
                        {
                            varwt
                        } else {
                            diffwt
                        };
                        cc += 2;
                    }
                }

                // --- Nyquist texture test value ---
                for rr in 6..rr1 - 6 {
                    let mut cc = 6 + (fc(rr as isize, 2) & 1);
                    while cc < cc1 - 6 {
                        let indx = rr * TS + cc;
                        // diagonal offsets: m = SE("\"), p = NE("/")
                        let se1 = indx + v1 + 1;
                        let ne1 = indx + 1 - v1;
                        let nw1 = indx - v1 - 1;
                        let sw1 = indx + v1 - 1;
                        let se2 = indx + v2 + 2;
                        let ne2 = indx + 2 - v2;
                        let nw2 = indx - v2 - 2;
                        let sw2 = indx + v2 - 2;
                        let val = GAUSSODD[0] * cddiffsq[indx]
                            + GAUSSODD[1]
                                * (cddiffsq[nw1] + cddiffsq[ne1] + cddiffsq[sw1] + cddiffsq[se1])
                            + GAUSSODD[2]
                                * (cddiffsq[indx - v2]
                                    + cddiffsq[indx - 2]
                                    + cddiffsq[indx + 2]
                                    + cddiffsq[indx + v2])
                            + GAUSSODD[3]
                                * (cddiffsq[nw2] + cddiffsq[ne2] + cddiffsq[sw2] + cddiffsq[se2])
                            - (GAUSSGRAD[0] * delhvsqsum[indx]
                                + GAUSSGRAD[1]
                                    * (delhvsqsum[indx - v1]
                                        + delhvsqsum[indx + 1]
                                        + delhvsqsum[indx - 1]
                                        + delhvsqsum[indx + v1])
                                + GAUSSGRAD[2]
                                    * (delhvsqsum[nw1]
                                        + delhvsqsum[ne1]
                                        + delhvsqsum[sw1]
                                        + delhvsqsum[se1])
                                + GAUSSGRAD[3]
                                    * (delhvsqsum[indx - v2]
                                        + delhvsqsum[indx - 2]
                                        + delhvsqsum[indx + 2]
                                        + delhvsqsum[indx + v2])
                                + GAUSSGRAD[4]
                                    * (delhvsqsum[indx - v2 - 1]
                                        + delhvsqsum[indx - v2 + 1]
                                        + delhvsqsum[indx - TS - 2]
                                        + delhvsqsum[indx - TS + 2]
                                        + delhvsqsum[indx + TS - 2]
                                        + delhvsqsum[indx + TS + 2]
                                        + delhvsqsum[indx + v2 - 1]
                                        + delhvsqsum[indx + v2 + 1])
                                + GAUSSGRAD[5]
                                    * (delhvsqsum[nw2]
                                        + delhvsqsum[ne2]
                                        + delhvsqsum[sw2]
                                        + delhvsqsum[se2]));
                        nyqutest[indx >> 1] = val;
                        cc += 2;
                    }
                }

                // --- Nyquist test: flag regions & bounding box ---
                let mut nystartrow = 0usize;
                let mut nyendrow = 0usize;
                let mut nystartcol = TS + 1;
                let mut nyendcol = 0usize;
                for rr in 6..rr1 - 6 {
                    let mut cc = 6 + (fc(rr as isize, 2) & 1);
                    while cc < cc1 - 6 {
                        let indx = rr * TS + cc;
                        if nyqutest[indx >> 1] > 0.0 {
                            nyquist[indx >> 1] = 1;
                            nystartrow = if nystartrow != 0 { nystartrow } else { rr };
                            nyendrow = rr;
                            nystartcol = nystartcol.min(cc);
                            nyendcol = nyendcol.max(cc);
                        }
                        cc += 2;
                    }
                }

                let do_nyquist = nystartrow != nyendrow && nystartcol != nyendcol;
                if do_nyquist {
                    nyendrow += 1;
                    nyendcol += 1;
                    nystartcol -= nystartcol & 1;
                    nystartrow = nystartrow.max(8);
                    nyendrow = nyendrow.min(rr1 - 8);
                    nystartcol = nystartcol.max(8);
                    nyendcol = nyendcol.min(cc1 - 8);

                    // majority-vote smoothing of the nyquist flag
                    for rr in nystartrow..nyendrow {
                        let mut indx = rr * TS + nystartcol + (fc(rr as isize, 2) & 1);
                        while indx < rr * TS + nyendcol {
                            let se1 = indx + v1 + 1;
                            let ne1 = indx + 1 - v1;
                            let nw1 = indx - v1 - 1;
                            let sw1 = indx + v1 - 1;
                            let t = nyquist[(indx - v2) >> 1] as u32
                                + nyquist[nw1 >> 1] as u32
                                + nyquist[ne1 >> 1] as u32
                                + nyquist[(indx - 2) >> 1] as u32
                                + nyquist[(indx + 2) >> 1] as u32
                                + nyquist[sw1 >> 1] as u32
                                + nyquist[se1 >> 1] as u32
                                + nyquist[(indx + v2) >> 1] as u32;
                            nyquist2[indx >> 1] = if t > 4 {
                                1
                            } else if t < 4 {
                                0
                            } else {
                                nyquist[indx >> 1]
                            };
                            indx += 2;
                        }
                    }

                    // area interpolation in Nyquist regions
                    for rr in nystartrow..nyendrow {
                        let mut indx = rr * TS + nystartcol + (fc(rr as isize, 2) & 1);
                        while indx < rr * TS + nyendcol {
                            if nyquist2[indx >> 1] != 0 {
                                let mut sumcfa = 0.0;
                                let mut sumh = 0.0;
                                let mut sumv = 0.0;
                                let mut sumsqh = 0.0;
                                let mut sumsqv = 0.0;
                                let mut areawt = 0.0;
                                let mut i = -6isize;
                                while i < 7 {
                                    let mut indx1 = (indx as isize + i * TS as isize - 6) as usize;
                                    let mut _j = -6isize;
                                    while _j < 7 {
                                        if nyquist2[indx1 >> 1] != 0 {
                                            let cfat = cfa[indx1];
                                            sumcfa += cfat;
                                            sumh += cfa[indx1 - 1] + cfa[indx1 + 1];
                                            sumv += cfa[indx1 - v1] + cfa[indx1 + v1];
                                            sumsqh +=
                                                sqr(cfat - cfa[indx1 - 1]) + sqr(cfat - cfa[indx1 + 1]);
                                            sumsqv += sqr(cfat - cfa[indx1 - v1])
                                                + sqr(cfat - cfa[indx1 + v1]);
                                            areawt += 1.0;
                                        }
                                        indx1 += 2;
                                        _j += 2;
                                    }
                                    i += 2;
                                }
                                let sumh2 = sumcfa - 0.5 * sumh;
                                let sumv2 = sumcfa - 0.5 * sumv;
                                let areawt2 = 0.5 * areawt;
                                let hcdvar = EPSSQ + (areawt2 * sumsqh - sumh2 * sumh2).abs();
                                let vcdvar = EPSSQ + (areawt2 * sumsqv - sumv2 * sumv2).abs();
                                hvwt[indx >> 1] = hcdvar / (vcdvar + hcdvar);
                            }
                            indx += 2;
                        }
                    }
                }

                // --- populate green at red/blue sensels ---
                for rr in 8..rr1 - 8 {
                    let mut indx = rr * TS + 8 + (fc(rr as isize, 2) & 1);
                    while indx < rr * TS + cc1 - 8 {
                        let se1 = indx + v1 + 1;
                        let ne1 = indx + 1 - v1;
                        let nw1 = indx - v1 - 1;
                        let sw1 = indx + v1 - 1;
                        let hvwtalt = 0.25
                            * (hvwt[nw1 >> 1] + hvwt[ne1 >> 1] + hvwt[sw1 >> 1] + hvwt[se1 >> 1]);
                        if (0.5 - hvwt[indx >> 1]).abs() < (0.5 - hvwtalt).abs() {
                            hvwt[indx >> 1] = hvwtalt;
                        }
                        dgrb0[indx >> 1] = intp(hvwt[indx >> 1], vcd[indx], hcd[indx]);
                        rgbgreen[indx] = cfa[indx] + dgrb0[indx >> 1];
                        if nyquist2[indx >> 1] != 0 {
                            dgrb2h[indx >> 1] =
                                sqr(rgbgreen[indx] - 0.5 * (rgbgreen[indx - 1] + rgbgreen[indx + 1]));
                            dgrb2v[indx >> 1] = sqr(
                                rgbgreen[indx] - 0.5 * (rgbgreen[indx - v1] + rgbgreen[indx + v1]),
                            );
                        } else {
                            dgrb2h[indx >> 1] = 0.0;
                            dgrb2v[indx >> 1] = 0.0;
                        }
                        indx += 2;
                    }
                }

                // --- refine Nyquist regions using green curvature ---
                if do_nyquist {
                    for rr in nystartrow..nyendrow {
                        let mut indx = rr * TS + nystartcol + (fc(rr as isize, 2) & 1);
                        while indx < rr * TS + nyendcol {
                            if nyquist2[indx >> 1] != 0 {
                                let se1 = indx + v1 + 1;
                                let ne1 = indx + 1 - v1;
                                let nw1 = indx - v1 - 1;
                                let sw1 = indx + v1 - 1;
                                let se2 = indx + v2 + 2;
                                let ne2 = indx + 2 - v2;
                                let nw2 = indx - v2 - 2;
                                let sw2 = indx + v2 - 2;
                                let gvarh = EPSSQ
                                    + (GQUINC[0] * dgrb2h[indx >> 1]
                                        + GQUINC[1]
                                            * (dgrb2h[nw1 >> 1]
                                                + dgrb2h[ne1 >> 1]
                                                + dgrb2h[sw1 >> 1]
                                                + dgrb2h[se1 >> 1])
                                        + GQUINC[2]
                                            * (dgrb2h[(indx - v2) >> 1]
                                                + dgrb2h[(indx - 2) >> 1]
                                                + dgrb2h[(indx + 2) >> 1]
                                                + dgrb2h[(indx + v2) >> 1])
                                        + GQUINC[3]
                                            * (dgrb2h[nw2 >> 1]
                                                + dgrb2h[ne2 >> 1]
                                                + dgrb2h[sw2 >> 1]
                                                + dgrb2h[se2 >> 1]));
                                let gvarv = EPSSQ
                                    + (GQUINC[0] * dgrb2v[indx >> 1]
                                        + GQUINC[1]
                                            * (dgrb2v[nw1 >> 1]
                                                + dgrb2v[ne1 >> 1]
                                                + dgrb2v[sw1 >> 1]
                                                + dgrb2v[se1 >> 1])
                                        + GQUINC[2]
                                            * (dgrb2v[(indx - v2) >> 1]
                                                + dgrb2v[(indx - 2) >> 1]
                                                + dgrb2v[(indx + 2) >> 1]
                                                + dgrb2v[(indx + v2) >> 1])
                                        + GQUINC[3]
                                            * (dgrb2v[nw2 >> 1]
                                                + dgrb2v[ne2 >> 1]
                                                + dgrb2v[sw2 >> 1]
                                                + dgrb2v[se2 >> 1]));
                                dgrb0[indx >> 1] =
                                    (hcd[indx] * gvarv + vcd[indx] * gvarh) / (gvarv + gvarh);
                                rgbgreen[indx] = cfa[indx] + dgrb0[indx >> 1];
                            }
                            indx += 2;
                        }
                    }
                }

                // --- diagonal derivatives (delp/delm) and R-B diagonal variances ---
                for rr in 6..rr1 - 6 {
                    let even_row = (fc(rr as isize, 2) & 1) == 0;
                    let mut cc = 6;
                    while cc < cc1 - 6 {
                        let indx = rr * TS + cc;
                        let p1a = indx + 1 - v1; // NE of indx
                        let p1b = indx + v1 - 1; // SW of indx
                        let m1a = indx + v1 + 1; // SE of indx
                        let m1b = indx - v1 - 1; // NW of indx
                        if even_row {
                            delp[indx >> 1] = (cfa[p1a] - cfa[p1b]).abs();
                            delm[indx >> 1] = (cfa[m1a] - cfa[m1b]).abs();
                            let t = cfa[indx + 1];
                            dgrbsq1p[indx >> 1] =
                                sqr(t - cfa[indx + 1 - (v1 - 1)]) + sqr(t - cfa[indx + 1 + (v1 - 1)]);
                            dgrbsq1m[indx >> 1] =
                                sqr(t - cfa[indx + 1 - (v1 + 1)]) + sqr(t - cfa[indx + 1 + (v1 + 1)]);
                        } else {
                            let t = cfa[indx];
                            dgrbsq1p[indx >> 1] = sqr(t - cfa[indx + 1 - v1]) + sqr(t - cfa[indx + v1 - 1]);
                            dgrbsq1m[indx >> 1] = sqr(t - cfa[indx - v1 - 1]) + sqr(t - cfa[indx + v1 + 1]);
                            delp[indx >> 1] =
                                (cfa[indx + 1 + 1 - v1] - cfa[indx + 1 + v1 - 1]).abs();
                            delm[indx >> 1] =
                                (cfa[indx + 1 + v1 + 1] - cfa[indx + 1 - v1 - 1]).abs();
                        }
                        cc += 2;
                    }
                }

                // --- diagonal R+B interpolation (rbm along "\", rbp along "/") ---
                for rr in 8..rr1 - 8 {
                    let mut cc = 8 + (fc(rr as isize, 2) & 1);
                    while cc < cc1 - 8 {
                        let indx = rr * TS + cc;
                        let indx1 = indx >> 1;
                        let cfai = cfa[indx];
                        // SE/NW along "\" (m), NE/SW along "/" (p)
                        let se = indx + v1 + 1;
                        let nw = indx - v1 - 1;
                        let ne = indx + 1 - v1;
                        let sw = indx + v1 - 1;
                        let se2 = indx + v2 + 2;
                        let nw2 = indx - v2 - 2;
                        let ne2 = indx + 2 - v2;
                        let sw2 = indx + v2 - 2;

                        let crse = 2.0 * cfa[se] / (EPS + cfai + cfa[se2]);
                        let crnw = 2.0 * cfa[nw] / (EPS + cfai + cfa[nw2]);
                        let crne = 2.0 * cfa[ne] / (EPS + cfai + cfa[ne2]);
                        let crsw = 2.0 * cfa[sw] / (EPS + cfai + cfa[sw2]);

                        let rbse = if (1.0 - crse).abs() < ARTHRESH {
                            cfai * crse
                        } else {
                            cfa[se] + 0.5 * (cfai - cfa[se2])
                        };
                        let rbnw = if (1.0 - crnw).abs() < ARTHRESH {
                            cfai * crnw
                        } else {
                            cfa[nw] + 0.5 * (cfai - cfa[nw2])
                        };
                        let rbne = if (1.0 - crne).abs() < ARTHRESH {
                            cfai * crne
                        } else {
                            cfa[ne] + 0.5 * (cfai - cfa[ne2])
                        };
                        let rbsw = if (1.0 - crsw).abs() < ARTHRESH {
                            cfai * crsw
                        } else {
                            cfa[sw] + 0.5 * (cfai - cfa[sw2])
                        };

                        let wtse = EPS + delm[indx1] + delm[se >> 1] + delm[se2 >> 1];
                        let wtnw = EPS + delm[indx1] + delm[nw >> 1] + delm[nw2 >> 1];
                        let wtne = EPS + delp[indx1] + delp[ne >> 1] + delp[ne2 >> 1];
                        let wtsw = EPS + delp[indx1] + delp[sw >> 1] + delp[sw2 >> 1];

                        let mut rbmv = (wtse * rbnw + wtnw * rbse) / (wtse + wtnw);
                        let mut rbpv = (wtne * rbsw + wtsw * rbne) / (wtne + wtsw);

                        let rbvarm = EPSSQ
                            + (GAUSSEVEN[0]
                                * (dgrbsq1m[(indx - v1) >> 1]
                                    + dgrbsq1m[(indx - 1) >> 1]
                                    + dgrbsq1m[(indx + 1) >> 1]
                                    + dgrbsq1m[(indx + v1) >> 1])
                                + GAUSSEVEN[1]
                                    * (dgrbsq1m[(indx - v2 - 1) >> 1]
                                        + dgrbsq1m[(indx - v2 + 1) >> 1]
                                        + dgrbsq1m[(indx - 2 - v1) >> 1]
                                        + dgrbsq1m[(indx + 2 - v1) >> 1]
                                        + dgrbsq1m[(indx - 2 + v1) >> 1]
                                        + dgrbsq1m[(indx + 2 + v1) >> 1]
                                        + dgrbsq1m[(indx + v2 - 1) >> 1]
                                        + dgrbsq1m[(indx + v2 + 1) >> 1]));
                        let rbvarp = EPSSQ
                            + (GAUSSEVEN[0]
                                * (dgrbsq1p[(indx - v1) >> 1]
                                    + dgrbsq1p[(indx - 1) >> 1]
                                    + dgrbsq1p[(indx + 1) >> 1]
                                    + dgrbsq1p[(indx + v1) >> 1])
                                + GAUSSEVEN[1]
                                    * (dgrbsq1p[(indx - v2 - 1) >> 1]
                                        + dgrbsq1p[(indx - v2 + 1) >> 1]
                                        + dgrbsq1p[(indx - 2 - v1) >> 1]
                                        + dgrbsq1p[(indx + 2 - v1) >> 1]
                                        + dgrbsq1p[(indx - 2 + v1) >> 1]
                                        + dgrbsq1p[(indx + 2 + v1) >> 1]
                                        + dgrbsq1p[(indx + v2 - 1) >> 1]
                                        + dgrbsq1p[(indx + v2 + 1) >> 1]));
                        pmwt[indx1] = rbvarm / (rbvarp + rbvarm);

                        // saturation bounding
                        if rbpv < cfai {
                            if 2.0 * rbpv < cfai {
                                rbpv = med3(rbpv, cfa[ne], cfa[sw]);
                            } else {
                                let pwt = 2.0 * (cfai - rbpv) / (EPS + rbpv + cfai);
                                rbpv = pwt * rbpv + (1.0 - pwt) * med3(rbpv, cfa[ne], cfa[sw]);
                            }
                        }
                        if rbmv < cfai {
                            if 2.0 * rbmv < cfai {
                                rbmv = med3(rbmv, cfa[nw], cfa[se]);
                            } else {
                                let mwt = 2.0 * (cfai - rbmv) / (EPS + rbmv + cfai);
                                rbmv = mwt * rbmv + (1.0 - mwt) * med3(rbmv, cfa[nw], cfa[se]);
                            }
                        }
                        if rbpv > CLIP_PT {
                            rbpv = med3(rbpv, cfa[ne], cfa[sw]);
                        }
                        if rbmv > CLIP_PT {
                            rbmv = med3(rbmv, cfa[nw], cfa[se]);
                        }
                        rbm[indx1] = rbmv;
                        rbp[indx1] = rbpv;
                        cc += 2;
                    }
                }

                // --- combine diagonal into R+B (rbint) ---
                for rr in 10..rr1 - 10 {
                    let mut cc = 10 + (fc(rr as isize, 2) & 1);
                    while cc < cc1 - 10 {
                        let indx = rr * TS + cc;
                        let indx1 = indx >> 1;
                        let se1 = indx + v1 + 1;
                        let ne1 = indx + 1 - v1;
                        let nw1 = indx - v1 - 1;
                        let sw1 = indx + v1 - 1;
                        let pmwtalt = 0.25
                            * (pmwt[nw1 >> 1] + pmwt[ne1 >> 1] + pmwt[sw1 >> 1] + pmwt[se1 >> 1]);
                        if (0.5 - pmwt[indx1]).abs() < (0.5 - pmwtalt).abs() {
                            pmwt[indx1] = pmwtalt;
                        }
                        rbint[indx1] =
                            0.5 * (cfa[indx] + rbm[indx1] * (1.0 - pmwt[indx1]) + rbp[indx1] * pmwt[indx1]);
                        cc += 2;
                    }
                }

                // --- refine green using the R+B values (cardinal) ---
                for rr in 12..rr1 - 12 {
                    let mut cc = 12 + (fc(rr as isize, 2) & 1);
                    while cc < cc1 - 12 {
                        let indx = rr * TS + cc;
                        let indx1 = indx >> 1;
                        if (0.5 - pmwt[indx1]).abs() < (0.5 - hvwt[indx1]).abs() {
                            cc += 2;
                            continue;
                        }
                        // rbint lives on the R/B checkerboard; its neighbors are
                        // addressed in half-res index space (indx1 ± v1 = the R/B
                        // site 2 rows away, indx1 ± 1 = 2 columns away).
                        let rbi = rbint[indx1];
                        let cru = cfa[indx - v1] * 2.0 / (EPS + rbi + rbint[indx1 - v1]);
                        let crd = cfa[indx + v1] * 2.0 / (EPS + rbi + rbint[indx1 + v1]);
                        let crl = cfa[indx - 1] * 2.0 / (EPS + rbi + rbint[indx1 - 1]);
                        let crr = cfa[indx + 1] * 2.0 / (EPS + rbi + rbint[indx1 + 1]);

                        let gu = if (1.0 - cru).abs() < ARTHRESH {
                            rbi * cru
                        } else {
                            cfa[indx - v1] + 0.5 * (rbi - rbint[indx1 - v1])
                        };
                        let gd = if (1.0 - crd).abs() < ARTHRESH {
                            rbi * crd
                        } else {
                            cfa[indx + v1] + 0.5 * (rbi - rbint[indx1 + v1])
                        };
                        let gl = if (1.0 - crl).abs() < ARTHRESH {
                            rbi * crl
                        } else {
                            cfa[indx - 1] + 0.5 * (rbi - rbint[indx1 - 1])
                        };
                        let gr = if (1.0 - crr).abs() < ARTHRESH {
                            rbi * crr
                        } else {
                            cfa[indx + 1] + 0.5 * (rbi - rbint[indx1 + 1])
                        };

                        let mut gintv =
                            (dirwts0[indx - v1] * gd + dirwts0[indx + v1] * gu)
                                / (dirwts0[indx + v1] + dirwts0[indx - v1]);
                        let mut ginth =
                            (dirwts1[indx - 1] * gr + dirwts1[indx + 1] * gl)
                                / (dirwts1[indx - 1] + dirwts1[indx + 1]);

                        if gintv < rbi {
                            if 2.0 * gintv < rbi {
                                gintv = med3(gintv, cfa[indx - v1], cfa[indx + v1]);
                            } else {
                                let vwt = 2.0 * (rbi - gintv) / (EPS + gintv + rbi);
                                gintv = vwt * gintv
                                    + (1.0 - vwt) * med3(gintv, cfa[indx - v1], cfa[indx + v1]);
                            }
                        }
                        if ginth < rbi {
                            if 2.0 * ginth < rbi {
                                ginth = med3(ginth, cfa[indx - 1], cfa[indx + 1]);
                            } else {
                                let hwt = 2.0 * (rbi - ginth) / (EPS + ginth + rbi);
                                ginth = hwt * ginth
                                    + (1.0 - hwt) * med3(ginth, cfa[indx - 1], cfa[indx + 1]);
                            }
                        }
                        if ginth > CLIP_PT {
                            ginth = med3(ginth, cfa[indx - 1], cfa[indx + 1]);
                        }
                        if gintv > CLIP_PT {
                            gintv = med3(gintv, cfa[indx - v1], cfa[indx + v1]);
                        }
                        rgbgreen[indx] = ginth * (1.0 - hvwt[indx1]) + gintv * hvwt[indx1];
                        dgrb0[indx1] = rgbgreen[indx] - cfa[indx];
                        cc += 2;
                    }
                }

                // --- split G-R / G-B and fancy chrominance interpolation ---
                // At B cosets, move the difference into dgrb1 (G-B) and zero dgrb0.
                let mut rr = 13 - ey;
                while rr < rr1 - 12 {
                    let mut indx1 = (rr * TS + 13 - ex) >> 1;
                    let end = (rr * TS + cc1 - 12) >> 1;
                    while indx1 < end {
                        dgrb1[indx1] = dgrb0[indx1];
                        dgrb0[indx1] = 0.0;
                        indx1 += 1;
                    }
                    rr += 2;
                }

                for rr in 14..rr1 - 14 {
                    let mut cc = 14 + (fc(rr as isize, 2) & 1);
                    while cc < cc1 - 14 {
                        let indx = rr * TS + cc;
                        // which difference is present at this sensel
                        let sel = 1 - fc(rr as isize, cc as isize) / 2; // R->1(G-B), B->0(G-R)
                        let d = if sel == 0 { &mut dgrb0 } else { &mut dgrb1 };
                        // diagonal neighbor half-res indices
                        let m1 = indx - v1 - 1; // NW
                        let m1o = indx + v1 + 1; // SE
                        let p1 = indx + 1 - v1; // NE
                        let p1o = indx + v1 - 1; // SW
                        let m3 = indx - v3 - 3; // NW3
                        let m3o = indx + v3 + 3; // SE3
                        let p3 = indx + 3 - v3; // NE3
                        let p3o = indx + v3 - 3; // SW3

                        let wtnw = 1.0
                            / (EPS
                                + (d[m1 >> 1] - d[m1o >> 1]).abs()
                                + (d[m1 >> 1] - d[m3 >> 1]).abs()
                                + (d[m1o >> 1] - d[m3 >> 1]).abs());
                        let wtne = 1.0
                            / (EPS
                                + (d[p1 >> 1] - d[p1o >> 1]).abs()
                                + (d[p1 >> 1] - d[p3 >> 1]).abs()
                                + (d[p1o >> 1] - d[p3 >> 1]).abs());
                        let wtsw = 1.0
                            / (EPS
                                + (d[p1o >> 1] - d[p1 >> 1]).abs()
                                + (d[p1o >> 1] - d[m3o >> 1]).abs()
                                + (d[p1 >> 1] - d[p3o >> 1]).abs());
                        let wtse = 1.0
                            / (EPS
                                + (d[m1o >> 1] - d[m1 >> 1]).abs()
                                + (d[m1o >> 1] - d[p3o >> 1]).abs()
                                + (d[m1 >> 1] - d[m3o >> 1]).abs());

                        d[indx >> 1] = (wtnw
                            * (1.325 * d[m1 >> 1] - 0.175 * d[m3 >> 1]
                                - 0.075 * d[(m1 - 2) >> 1]
                                - 0.075 * d[(m1 - v2) >> 1])
                            + wtne
                                * (1.325 * d[p1 >> 1] - 0.175 * d[p3 >> 1]
                                    - 0.075 * d[(p1 + 2) >> 1]
                                    - 0.075 * d[(p1 + v2) >> 1])
                            + wtsw
                                * (1.325 * d[p1o >> 1] - 0.175 * d[p3o >> 1]
                                    - 0.075 * d[(p1o - 2) >> 1]
                                    - 0.075 * d[(p1o - v2) >> 1])
                            + wtse
                                * (1.325 * d[m1o >> 1] - 0.175 * d[m3o >> 1]
                                    - 0.075 * d[(m1o + 2) >> 1]
                                    - 0.075 * d[(m1o + v2) >> 1]))
                            / (wtnw + wtne + wtsw + wtse);
                        cc += 2;
                    }
                }

                // --- assemble R/G/B and write the tile interior to the output ---
                for rr in 16..rr1 - 16 {
                    let row = top + rr as isize;
                    if row < 0 || row >= hi {
                        continue;
                    }
                    for cc in 16..cc1 - 16 {
                        let col = left + cc as isize;
                        if col < 0 || col >= wi {
                            continue;
                        }
                        let indx = rr * TS + cc;
                        let green = rgbgreen[indx];
                        let (red, blue) = if fc(rr as isize, cc as isize) == 1 {
                            // green sensel: weighted from neighbors
                            let temp = 1.0
                                / (hvwt[(indx - v1) >> 1] + 2.0
                                    - hvwt[(indx + 1) >> 1]
                                    - hvwt[(indx - 1) >> 1]
                                    + hvwt[(indx + v1) >> 1]);
                            let r = green
                                - (hvwt[(indx - v1) >> 1] * dgrb0[(indx - v1) >> 1]
                                    + (1.0 - hvwt[(indx + 1) >> 1]) * dgrb0[(indx + 1) >> 1]
                                    + (1.0 - hvwt[(indx - 1) >> 1]) * dgrb0[(indx - 1) >> 1]
                                    + hvwt[(indx + v1) >> 1] * dgrb0[(indx + v1) >> 1])
                                    * temp;
                            let b = green
                                - (hvwt[(indx - v1) >> 1] * dgrb1[(indx - v1) >> 1]
                                    + (1.0 - hvwt[(indx + 1) >> 1]) * dgrb1[(indx + 1) >> 1]
                                    + (1.0 - hvwt[(indx - 1) >> 1]) * dgrb1[(indx - 1) >> 1]
                                    + hvwt[(indx + v1) >> 1] * dgrb1[(indx + v1) >> 1])
                                    * temp;
                            (r, b)
                        } else {
                            (green - dgrb0[indx >> 1], green - dgrb1[indx >> 1])
                        };
                        let o = (row as usize) * w + col as usize;
                        out.data[o] = [red.max(0.0), green.max(0.0), blue.max(0.0)];
                    }
                }

                left += TS as isize - 32;
            }
            top += TS as isize - 32;
        }

        out
    }

    fn name(&self) -> &'static str {
        "amaze"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::CfaPattern;
    use crate::demosaic::tests_common::{max_interior_error, mosaic_from_fn};

    #[test]
    fn constant_field_reconstructs() {
        let cfa = mosaic_from_fn(200, 180, CfaPattern::Rggb, |_, _| 0.5);
        let rgb = Amaze.demosaic(&cfa);
        for px in &rgb.data {
            for c in px {
                assert!((c - 0.5).abs() < 2e-3, "got {c}");
            }
        }
    }

    #[test]
    fn linear_gradient_low_error() {
        let grad = |x: usize, y: usize| 0.2 + 0.002 * x as f32 + 0.0015 * y as f32;
        let cfa = mosaic_from_fn(220, 200, CfaPattern::Rggb, grad);
        let rgb = Amaze.demosaic(&cfa);
        // exclude the outer tile halo region from the check
        let err = max_interior_error(&rgb, 20, grad);
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
            let cfa = mosaic_from_fn(160, 160, p, |_, _| 0.42);
            let rgb = Amaze.demosaic(&cfa);
            for px in &rgb.data {
                for c in px {
                    assert!((c - 0.42).abs() < 3e-3, "pattern {:?} got {c}", p);
                }
            }
        }
    }
}
