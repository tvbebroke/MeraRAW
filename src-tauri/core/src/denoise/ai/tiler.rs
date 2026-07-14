//! Tile split / overlap / cosine-merge for AI inference (doc 03 §6).

pub const TILE: usize = 512;
pub const TILE_OVERLAP: usize = 64;

#[derive(Debug, Clone)]
pub struct Tile {
    /// Top-left including halo (may be negative before clamp).
    pub x: i32,
    pub y: i32,
    pub pw: usize,
    pub ph: usize,
    pub halo: usize,
}

/// Cover `w×h` with overlapping TILE² patches.
pub fn split_tiles(w: usize, h: usize) -> Vec<Tile> {
    let halo = TILE_OVERLAP / 2;
    let step = TILE - TILE_OVERLAP;
    let mut tiles = Vec::new();
    let mut y = 0i32;
    while y < h as i32 {
        let mut x = 0i32;
        while x < w as i32 {
            let x0 = x - halo as i32;
            let y0 = y - halo as i32;
            let x1 = (x + step as i32 + halo as i32).min(w as i32 + halo as i32);
            let y1 = (y + step as i32 + halo as i32).min(h as i32 + halo as i32);
            // Clamp patch to image bounds for sizing; extract clamps samples.
            let pw = (x1 - x0).max(1) as usize;
            let ph = (y1 - y0).max(1) as usize;
            tiles.push(Tile {
                x: x0,
                y: y0,
                pw: pw.min(TILE + TILE_OVERLAP),
                ph: ph.min(TILE + TILE_OVERLAP),
                halo,
            });
            x += step as i32;
            if x >= w as i32 {
                break;
            }
        }
        y += step as i32;
        if y >= h as i32 {
            break;
        }
    }
    if tiles.is_empty() {
        tiles.push(Tile {
            x: 0,
            y: 0,
            pw: w,
            ph: h,
            halo: 0,
        });
    }
    tiles
}

/// Raised-cosine weight for a pixel inside a tile (sums to ~1 across overlap).
pub fn tile_weight(t: &Tile, lx: usize, ly: usize) -> f32 {
    if t.halo == 0 {
        return 1.0;
    }
    let hx = t.halo as f32;
    let wx = edge_weight(lx as f32, t.pw as f32, hx);
    let wy = edge_weight(ly as f32, t.ph as f32, hx);
    wx * wy
}

fn edge_weight(p: f32, len: f32, halo: f32) -> f32 {
    if p < halo {
        let t = (p / halo).clamp(0.0, 1.0);
        0.5 * (1.0 - (std::f32::consts::PI * t).cos())
    } else if p > len - halo {
        let t = ((len - p) / halo).clamp(0.0, 1.0);
        0.5 * (1.0 - (std::f32::consts::PI * t).cos())
    } else {
        1.0
    }
}

/// Feather-merge tile results into a full buffer (weights sum normalized).
pub fn merge_tiles(w: usize, h: usize, tiles: &[(Tile, Vec<f32>)]) -> Vec<f32> {
    let mut acc = vec![0f32; w * h * 3];
    let mut wsum = vec![0f32; w * h];
    for (t, src) in tiles {
        for y in 0..t.ph {
            for x in 0..t.pw {
                let dx = t.x + x as i32;
                let dy = t.y + y as i32;
                if dx < 0 || dy < 0 || dx >= w as i32 || dy >= h as i32 {
                    continue;
                }
                let wt = tile_weight(t, x, y);
                let di = dy as usize * w + dx as usize;
                let si = (y * t.pw + x) * 3;
                for c in 0..3 {
                    acc[di * 3 + c] += src[si + c] * wt;
                }
                wsum[di] += wt;
            }
        }
    }
    for i in 0..w * h {
        let s = wsum[i].max(1e-6);
        for c in 0..3 {
            acc[i * 3 + c] /= s;
        }
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_cover_image() {
        let tiles = split_tiles(1000, 800);
        assert!(tiles.len() >= 4);
        let mut hit = vec![false; 1000 * 800];
        for t in &tiles {
            for y in 0..t.ph {
                for x in 0..t.pw {
                    let dx = t.x + x as i32;
                    let dy = t.y + y as i32;
                    if dx >= 0 && dy >= 0 && dx < 1000 && dy < 800 {
                        hit[dy as usize * 1000 + dx as usize] = true;
                    }
                }
            }
        }
        assert!(hit.iter().all(|&h| h), "tiles must cover every pixel");
    }

    #[test]
    fn merge_weights_sum_near_one() {
        let (w, h) = (200, 200);
        let tiles = split_tiles(w, h);
        let mut wsum = vec![0f32; w * h];
        for t in &tiles {
            for y in 0..t.ph {
                for x in 0..t.pw {
                    let dx = t.x + x as i32;
                    let dy = t.y + y as i32;
                    if dx >= 0 && dy >= 0 && dx < w as i32 && dy < h as i32 {
                        wsum[dy as usize * w + dx as usize] += tile_weight(t, x, y);
                    }
                }
            }
        }
        for (i, &s) in wsum.iter().enumerate() {
            assert!(s > 0.5, "pixel {i} weight {s}");
        }
    }
}
