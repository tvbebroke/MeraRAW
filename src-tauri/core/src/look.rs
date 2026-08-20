//! Original bundled cinematic Looks. Generated at runtime as tagged `.cube`
//! data so they share the production LUT path. No vendor/stock trademarks.

use crate::error::CoreError;
use crate::idt::{srgb_eotf, srgb_oetf};
use crate::lut::{CubeLut, LutKind};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
struct LookGen {
    contrast: f32,
    sat: f32,
    /// Add in linear Rec.709 shadows (hue RGB 0..1).
    shadow: [f32; 3],
    highlight: [f32; 3],
    fade: f32,
    lift: f32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub kind: u32,
    pub intensity_default: f32,
    pub grain_amount: f32,
    pub grain_size: f32,
    /// sRGB 8-bit probes: shadow grey, skin, sky — from the real LUT path.
    pub preview: [[u8; 3]; 3],
}

const LOOKS: &[(&str, &str, &str, &str, LookGen, f32, f32)] = &[
    (
        "neutral_contrast",
        "Neutral Contrast",
        "Gentle S-curve, no hue shift.",
        "Print",
        LookGen {
            contrast: 0.12,
            sat: 1.02,
            shadow: [0.0, 0.0, 0.0],
            highlight: [0.0, 0.0, 0.0],
            fade: 0.0,
            lift: 0.0,
        },
        0.0,
        2.0,
    ),
    (
        "soft_print",
        "Soft Print",
        "Rolled highlights, open shadows, mild warmth.",
        "Print",
        LookGen {
            contrast: 0.06,
            sat: 0.96,
            shadow: [0.02, 0.01, 0.0],
            highlight: [0.03, 0.015, 0.0],
            fade: 0.04,
            lift: 0.02,
        },
        4.0,
        2.2,
    ),
    (
        "hard_print",
        "Hard Print",
        "Punchy contrast, denser blacks.",
        "Print",
        LookGen {
            contrast: 0.22,
            sat: 1.06,
            shadow: [0.0, 0.0, 0.02],
            highlight: [0.02, 0.01, 0.0],
            fade: 0.0,
            lift: -0.02,
        },
        6.0,
        1.8,
    ),
    (
        "restrained_teal",
        "Restrained Teal",
        "Cool shadows, warm highlights — kept subtle.",
        "Cinematic",
        LookGen {
            contrast: 0.10,
            sat: 0.98,
            shadow: [0.0, 0.03, 0.05],
            highlight: [0.05, 0.02, 0.0],
            fade: 0.02,
            lift: 0.0,
        },
        3.0,
        2.0,
    ),
    (
        "night_sodium",
        "Night Sodium",
        "Amber street-light bias, crushed ambience.",
        "Cinematic",
        LookGen {
            contrast: 0.16,
            sat: 0.92,
            shadow: [0.04, 0.015, 0.0],
            highlight: [0.06, 0.03, 0.0],
            fade: 0.03,
            lift: -0.03,
        },
        8.0,
        2.4,
    ),
    (
        "bleach_bypass",
        "Bleach Bypass",
        "Desaturated, silver highlight sheen.",
        "Process",
        LookGen {
            contrast: 0.28,
            sat: 0.55,
            shadow: [0.0, 0.0, 0.0],
            highlight: [0.04, 0.04, 0.05],
            fade: 0.0,
            lift: -0.04,
        },
        10.0,
        1.6,
    ),
    (
        "period_warm",
        "Period Warm",
        "Tobacco shadows, golden mids.",
        "Period",
        LookGen {
            contrast: 0.10,
            sat: 0.94,
            shadow: [0.05, 0.02, 0.0],
            highlight: [0.06, 0.04, 0.01],
            fade: 0.05,
            lift: 0.01,
        },
        5.0,
        2.5,
    ),
    (
        "period_cool",
        "Period Cool",
        "Steel highlights, slight cyan drape.",
        "Period",
        LookGen {
            contrast: 0.10,
            sat: 0.90,
            shadow: [0.0, 0.02, 0.05],
            highlight: [0.01, 0.03, 0.05],
            fade: 0.04,
            lift: 0.0,
        },
        4.0,
        2.3,
    ),
    (
        "cross_process",
        "Cross Process",
        "Skewed primaries, cyan/yellow crossover.",
        "Process",
        LookGen {
            contrast: 0.18,
            sat: 1.12,
            shadow: [0.0, 0.04, 0.06],
            highlight: [0.08, 0.06, 0.0],
            fade: 0.0,
            lift: 0.0,
        },
        2.0,
        1.8,
    ),
    (
        "mono_silver",
        "Mono Silver",
        "Hue-free print with silver shoulder.",
        "Mono",
        LookGen {
            contrast: 0.14,
            sat: 0.0,
            shadow: [0.0, 0.0, 0.0],
            highlight: [0.02, 0.02, 0.025],
            fade: 0.02,
            lift: 0.0,
        },
        7.0,
        2.0,
    ),
    (
        "fade",
        "Fade",
        "Lifted blacks, lowered sat, time-worn.",
        "Print",
        LookGen {
            contrast: -0.04,
            sat: 0.78,
            shadow: [0.04, 0.03, 0.02],
            highlight: [0.02, 0.02, 0.01],
            fade: 0.10,
            lift: 0.06,
        },
        3.0,
        3.0,
    ),
    (
        "moonlight",
        "Moonlight",
        "Cool fill, compressed ambience, faint silver edge.",
        "Night",
        LookGen {
            contrast: 0.14,
            sat: 0.82,
            shadow: [0.0, 0.02, 0.06],
            highlight: [0.02, 0.03, 0.06],
            fade: 0.03,
            lift: -0.02,
        },
        6.0,
        2.2,
    ),
    (
        "desert_warm",
        "Desert Warm",
        "Dusty mids, amber highlights, dry shadows.",
        "Cinematic",
        LookGen {
            contrast: 0.11,
            sat: 1.04,
            shadow: [0.04, 0.02, 0.0],
            highlight: [0.08, 0.05, 0.01],
            fade: 0.02,
            lift: 0.0,
        },
        4.0,
        2.6,
    ),
    (
        "overcast",
        "Overcast",
        "Low contrast, muted chroma, open shadows.",
        "Print",
        LookGen {
            contrast: -0.02,
            sat: 0.86,
            shadow: [0.01, 0.02, 0.03],
            highlight: [0.02, 0.02, 0.03],
            fade: 0.06,
            lift: 0.03,
        },
        2.0,
        2.8,
    ),
    (
        "twilight",
        "Twilight",
        "Blue wrap in shadows, residual warmth in speculars.",
        "Night",
        LookGen {
            contrast: 0.13,
            sat: 0.92,
            shadow: [0.0, 0.02, 0.07],
            highlight: [0.06, 0.03, 0.01],
            fade: 0.03,
            lift: -0.01,
        },
        5.0,
        2.1,
    ),
    (
        "silver_highs",
        "Silver Highs",
        "Neutral body, metallic highlight roll.",
        "Mono",
        LookGen {
            contrast: 0.16,
            sat: 0.72,
            shadow: [0.0, 0.0, 0.01],
            highlight: [0.04, 0.04, 0.05],
            fade: 0.0,
            lift: -0.02,
        },
        7.0,
        1.7,
    ),
    (
        "lifted_print",
        "Lifted Print",
        "Opened blacks, restrained highlights, slight warmth.",
        "Print",
        LookGen {
            contrast: 0.04,
            sat: 0.93,
            shadow: [0.03, 0.02, 0.01],
            highlight: [0.02, 0.015, 0.01],
            fade: 0.07,
            lift: 0.05,
        },
        3.0,
        2.7,
    ),
    (
        "sandstorm",
        "Sandstorm",
        "Heavy warm veil, crushed distance, dry grain.",
        "Cinematic",
        LookGen {
            contrast: 0.08,
            sat: 0.98,
            shadow: [0.06, 0.03, 0.0],
            highlight: [0.07, 0.05, 0.02],
            fade: 0.08,
            lift: 0.02,
        },
        9.0,
        2.8,
    ),
    (
        "morning_mist",
        "Morning Mist",
        "Soft lift, cool air, restrained contrast.",
        "Print",
        LookGen {
            contrast: 0.03,
            sat: 0.9,
            shadow: [0.01, 0.02, 0.04],
            highlight: [0.03, 0.03, 0.04],
            fade: 0.05,
            lift: 0.04,
        },
        1.5,
        3.0,
    ),
    (
        "hdr_show",
        "HDR Show",
        "Show LUT: denser print, highlight roll — still Rec.709 display.",
        "Show",
        LookGen {
            contrast: 0.18,
            sat: 1.04,
            shadow: [0.01, 0.0, 0.02],
            highlight: [0.02, 0.02, 0.01],
            fade: 0.0,
            lift: -0.01,
        },
        0.0,
        2.0,
    ),
];

pub fn catalog() -> Vec<LookInfo> {
    LOOKS
        .iter()
        .map(|(id, name, desc, cat, _, grain, gsize)| LookInfo {
            id: (*id).to_string(),
            name: (*name).to_string(),
            description: (*desc).to_string(),
            category: (*cat).to_string(),
            kind: if *id == "hdr_show" {
                LutKind::PrintShow as u32
            } else {
                LutKind::DisplayLook as u32
            },
            intensity_default: 100.0,
            grain_amount: *grain,
            grain_size: *gsize,
            preview: look_preview(id),
        })
        .collect()
}

fn look_preview(id: &str) -> [[u8; 3]; 3] {
    let Ok(cube) = generate_look_cube(id) else {
        return [[128, 128, 128]; 3];
    };
    let p = crate::lut::LutParams::display_rec709();
    let probes = [[0.12, 0.12, 0.12], [0.35, 0.22, 0.16], [0.18, 0.32, 0.62]];
    let mut out = [[0u8; 3]; 3];
    for (i, src) in probes.iter().enumerate() {
        let rgb = cube.apply_working(*src, &p);
        for c in 0..3 {
            out[i][c] = (srgb_oetf(rgb[c].max(0.0)) * 255.0).clamp(0.0, 255.0) as u8;
        }
    }
    out
}

pub fn is_bundled_id(id: &str) -> bool {
    LOOKS.iter().any(|(i, ..)| *i == id)
}

pub fn looks_dir() -> PathBuf {
    crate::catalog::data_dir().join("looks")
}

pub fn is_user_id(id: &str) -> bool {
    id.strip_prefix("user:").is_some_and(user_stem_ok)
}

fn user_stem_ok(stem: &str) -> bool {
    !stem.is_empty()
        && stem.len() <= 80
        && stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn safe_stem(path: &Path) -> String {
    let raw = path.file_stem().and_then(|s| s.to_str()).unwrap_or("look");
    let mut s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    s.truncate(80);
    if s.is_empty() {
        "look".into()
    } else {
        s
    }
}

pub fn user_cube_path(stem: &str) -> Option<PathBuf> {
    if !user_stem_ok(stem) {
        return None;
    }
    Some(looks_dir().join(format!("{stem}.cube")))
}

/// Copy a picked `.cube` into the user Looks folder. Returns `user:{stem}`.
pub fn install_user_look(src: &Path) -> Result<String, CoreError> {
    let dir = looks_dir();
    std::fs::create_dir_all(&dir).map_err(|e| CoreError::Io(format!("looks dir: {e}")))?;
    let base = safe_stem(src);
    if src
        .parent()
        .is_some_and(|p| p.canonicalize().ok() == dir.canonicalize().ok())
    {
        return Ok(format!("user:{base}"));
    }
    let mut stem = base.clone();
    let mut dest = dir.join(format!("{stem}.cube"));
    let mut n = 2u32;
    while dest.exists() && src.canonicalize().ok() != dest.canonicalize().ok() {
        stem = format!("{base}-{n}");
        if !user_stem_ok(&stem) {
            stem = format!("look-{n}");
        }
        dest = dir.join(format!("{stem}.cube"));
        n += 1;
        if n > 99 {
            return Err(CoreError::InvalidOp(
                "too many user looks with this name".into(),
            ));
        }
    }
    if src.canonicalize().ok() != dest.canonicalize().ok() {
        std::fs::copy(src, &dest).map_err(|e| CoreError::Io(format!("install look: {e}")))?;
    }
    Ok(format!("user:{stem}"))
}

/// Remove an imported user look. Bundled ids are rejected.
pub fn delete_user_look(id: &str) -> Result<(), CoreError> {
    let stem = id
        .strip_prefix("user:")
        .filter(|s| user_stem_ok(s))
        .ok_or_else(|| CoreError::InvalidOp("not a user look".into()))?;
    let path = looks_dir().join(format!("{stem}.cube"));
    if !path.is_file() {
        return Err(CoreError::InvalidOp("user look not found".into()));
    }
    std::fs::remove_file(&path).map_err(|e| CoreError::Io(format!("delete look: {e}")))?;
    Ok(())
}

fn user_preview(cube: &CubeLut) -> [[u8; 3]; 3] {
    let p = crate::lut::LutParams::display_rec709();
    let probes = [[0.12, 0.12, 0.12], [0.35, 0.22, 0.16], [0.18, 0.32, 0.62]];
    let mut out = [[0u8; 3]; 3];
    for (i, src) in probes.iter().enumerate() {
        let rgb = cube.apply_working(*src, &p);
        for c in 0..3 {
            out[i][c] = (srgb_oetf(rgb[c].max(0.0)) * 255.0).clamp(0.0, 255.0) as u8;
        }
    }
    out
}

pub fn scan_user_looks() -> Vec<LookInfo> {
    let dir = looks_dir();
    let rd = match std::fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for ent in rd.flatten() {
        let p = ent.path();
        if !p
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("cube"))
        {
            continue;
        }
        let stem = match p.file_stem().and_then(|s| s.to_str()) {
            Some(s) if user_stem_ok(s) => s.to_string(),
            _ => continue,
        };
        let cube = crate::lut::CubeLut::load_cube(&p).ok();
        let title = cube
            .as_ref()
            .and_then(|c| c.title.clone())
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| stem.replace('_', " "));
        let preview = cube
            .as_ref()
            .map(user_preview)
            .unwrap_or([[128, 128, 128]; 3]);
        out.push(LookInfo {
            id: format!("user:{stem}"),
            name: title,
            description: "Imported cube".into(),
            category: "User".into(),
            kind: LutKind::DisplayLook as u32,
            intensity_default: 100.0,
            grain_amount: 0.0,
            grain_size: 2.0,
            preview,
        });
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

pub fn all_looks() -> Vec<LookInfo> {
    let mut rows = catalog();
    rows.extend(scan_user_looks());
    rows
}

pub fn look_info(id: &str) -> Option<LookInfo> {
    catalog().into_iter().find(|l| l.id == id)
}

pub fn grain_for(id: &str) -> Option<(f32, f32)> {
    LOOKS
        .iter()
        .find(|(i, ..)| *i == id)
        .map(|(_, _, _, _, _, g, s)| (*g, *s))
}

fn apply_gen(lin: [f32; 3], g: &LookGen) -> [f32; 3] {
    let luma = 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
    // Pivot contrast around ~0.18 scene grey.
    let pivot = 0.18f32;
    let c = (1.0 + g.contrast).max(0.05);
    let shaped = (luma - pivot) * c + pivot + g.lift;
    let ratio = if luma.abs() > 1e-6 {
        shaped / luma
    } else {
        1.0
    };
    let mut rgb = [lin[0] * ratio, lin[1] * ratio, lin[2] * ratio];
    let l2 = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    rgb = [
        l2 + (rgb[0] - l2) * g.sat,
        l2 + (rgb[1] - l2) * g.sat,
        l2 + (rgb[2] - l2) * g.sat,
    ];
    let w_sh = 1.0 - smoothstep(0.0, 0.35, l2);
    let w_hi = smoothstep(0.55, 1.0, l2);
    for c in 0..3 {
        rgb[c] += w_sh * g.shadow[c] * 0.12;
        rgb[c] += w_hi * g.highlight[c] * 0.10;
        rgb[c] = rgb[c] * (1.0 - g.fade) + g.fade * 0.18;
        rgb[c] = rgb[c].max(0.0);
    }
    rgb
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0).max(1e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

const LOOK_SIZE: usize = 17;

pub fn generate_look_cube(id: &str) -> Result<CubeLut, CoreError> {
    let gen = LOOKS
        .iter()
        .find(|(i, ..)| *i == id)
        .map(|(_, _, _, _, g, _, _)| g)
        .ok_or_else(|| CoreError::InvalidOp(format!("unknown bundled look '{id}'")))?;
    let n = (LOOK_SIZE - 1) as f32;
    let mut data = Vec::with_capacity(LOOK_SIZE * LOOK_SIZE * LOOK_SIZE);
    for b in 0..LOOK_SIZE {
        for g in 0..LOOK_SIZE {
            for r in 0..LOOK_SIZE {
                let enc = [r as f32 / n, g as f32 / n, b as f32 / n];
                let lin = [srgb_eotf(enc[0]), srgb_eotf(enc[1]), srgb_eotf(enc[2])];
                let out = apply_gen(lin, gen);
                data.push([srgb_oetf(out[0]), srgb_oetf(out[1]), srgb_oetf(out[2])]);
            }
        }
    }
    Ok(CubeLut {
        size: LOOK_SIZE,
        data,
        domain_min: [0.0; 3],
        domain_max: [1.0; 3],
        title: Some(id.to_string()),
    })
}

/// Debug helper: dump a look as `.cube` text (authoring / fixtures).
pub fn look_cube_text(id: &str) -> Result<String, CoreError> {
    let cube = generate_look_cube(id)?;
    let mut s = format!("TITLE \"{id}\"\nLUT_3D_SIZE {}\n", cube.size);
    for px in &cube.data {
        s.push_str(&format!("{} {} {}\n", px[0], px[1], px[2]));
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lut::{CubeLut, LutParams};

    #[test]
    fn catalog_len() {
        assert_eq!(catalog().len(), 20);
        assert!(is_bundled_id("restrained_teal"));
        assert!(!is_bundled_id("kodak_2383"));
    }

    #[test]
    fn bundled_look_changes_a_probe() {
        let cube = generate_look_cube("bleach_bypass").unwrap();
        let p = LutParams::display_rec709();
        let skin = [0.35, 0.22, 0.16];
        let out = cube.apply_working(skin, &p);
        let delta: f32 = (0..3).map(|c| (out[c] - skin[c]).abs()).sum();
        assert!(delta > 0.02, "look was a no-op: {out:?}");
    }

    #[test]
    fn hdr_show_is_print_show() {
        assert_eq!(
            look_info("hdr_show").unwrap().kind,
            LutKind::PrintShow as u32
        );
    }

    #[test]
    fn apply_then_zero_opacity_is_identity() {
        let cube = generate_look_cube("soft_print").unwrap();
        let mut p = LutParams::display_rec709();
        let skin = [0.32, 0.20, 0.14];
        p.opacity = 1.0;
        let graded = cube.apply_working(skin, &p);
        p.opacity = 0.0;
        let cleared = cube.apply_working(skin, &p);
        assert_eq!(cleared, skin);
        let delta: f32 = (0..3).map(|c| (graded[c] - skin[c]).abs()).sum();
        assert!(delta > 0.005, "{graded:?}");
    }

    #[test]
    fn install_user_look_is_discoverable() {
        let dir = std::env::temp_dir().join(format!(
            "meratech-looks-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let prev = std::env::var("MERATECH_DATA_DIR").ok();
        std::env::set_var("MERATECH_DATA_DIR", &dir);
        let src = dir.join("soft-print.cube");
        std::fs::write(
            &src,
            "TITLE \"Soft Print User\"\nLUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n",
        )
        .unwrap();
        let key = install_user_look(&src).unwrap();
        assert_eq!(key, "user:soft-print");
        let users = scan_user_looks();
        assert!(users.iter().any(|l| l.id == "user:soft-print"), "{users:?}");
        let cube = CubeLut::load_cube(std::path::Path::new(&key)).unwrap();
        assert_eq!(cube.size, 2);
        delete_user_look(&key).unwrap();
        assert!(scan_user_looks().iter().all(|l| l.id != key));
        assert!(delete_user_look("bundled:soft_print").is_err());
        match prev {
            Some(v) => std::env::set_var("MERATECH_DATA_DIR", v),
            None => std::env::remove_var("MERATECH_DATA_DIR"),
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
