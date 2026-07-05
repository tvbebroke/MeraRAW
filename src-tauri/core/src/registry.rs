//! Param registry — contract A2, the quiet powerhouse. Single source of
//! truth for what params exist + valid ranges + defaults. Four consumers:
//! guard-wall, UI generation, serialization defaults, Claude tool schemas.

use crate::doc::ParamValue;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ParamType {
    F32,
    Bool,
    Enum,
    Curve,
    Color,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiHint {
    pub label: &'static str,
    pub step: f32,
    pub scale: &'static str, // "linear" | "log"
    pub group: &'static str,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamSpec {
    pub path: &'static str, // "exposure.stops"
    pub ty: ParamType,
    pub min: f32,
    pub max: f32,
    pub default: ParamValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<&'static [&'static str]>,
    pub ui: UiHint,
}

/// Fixed pipeline order (contract A5; spec 3.2). NOT stored in the doc.
/// P2 ships exposure + white_balance; P3 fills the rest at their slots.
pub const MODULE_ORDER: &[&str] = &[
    "exposure",      // 1: linear multiply
    "white_balance", // 2: chromatic adaptation
    "calibration",   // 3 (P3)
    "detail",        // 4 (P3, noise; sharpen pass at slot 8)
    "color_grade",   // 5 (P3)
    "hsl",           // 6 (P3)
    "tone_curve",    // 7 (P3)
    "lut",           // 8a: 3D look LUT (opacity param; cube path in meta)
                     // 8: sharpen (detail module, second pass)
];

fn f32_spec(
    path: &'static str,
    min: f32,
    max: f32,
    default: f32,
    label: &'static str,
    step: f32,
    group: &'static str,
) -> ParamSpec {
    ParamSpec {
        path,
        ty: ParamType::F32,
        min,
        max,
        default: ParamValue::F32(default),
        enum_values: None,
        ui: UiHint {
            label,
            step,
            scale: "linear",
            group,
        },
    }
}

fn build_registry() -> BTreeMap<&'static str, ParamSpec> {
    let mut m = BTreeMap::new();
    let mut specs = vec![
        // ---- slot 1–2 (P2 reference modules) ----
        f32_spec("exposure.stops", -5.0, 5.0, 0.0, "Exposure", 0.01, "Light"),
        f32_spec(
            "white_balance.temp",
            2000.0,
            50000.0,
            5200.0,
            "Temp",
            10.0,
            "White Balance",
        ),
        f32_spec(
            "white_balance.tint",
            -150.0,
            150.0,
            0.0,
            "Tint",
            1.0,
            "White Balance",
        ),
        // ---- slot 3: calibration (schema §3.3) ----
        f32_spec("calibration.red_hue", -100.0, 100.0, 0.0, "Red Hue", 1.0, "Calibration"),
        f32_spec("calibration.red_sat", -100.0, 100.0, 0.0, "Red Sat", 1.0, "Calibration"),
        f32_spec("calibration.green_hue", -100.0, 100.0, 0.0, "Green Hue", 1.0, "Calibration"),
        f32_spec("calibration.green_sat", -100.0, 100.0, 0.0, "Green Sat", 1.0, "Calibration"),
        f32_spec("calibration.blue_hue", -100.0, 100.0, 0.0, "Blue Hue", 1.0, "Calibration"),
        f32_spec("calibration.blue_sat", -100.0, 100.0, 0.0, "Blue Sat", 1.0, "Calibration"),
        f32_spec("calibration.shadow_tint", -100.0, 100.0, 0.0, "Shadow Tint", 1.0, "Calibration"),
        // ---- slot 4 + 8: detail (schema §3.4) ----
        f32_spec("detail.noise_luma", 0.0, 100.0, 0.0, "Luma NR", 1.0, "Detail"),
        f32_spec("detail.noise_chroma", 0.0, 100.0, 0.0, "Chroma NR", 1.0, "Detail"),
        f32_spec("detail.detail_preserve", 0.0, 100.0, 50.0, "Detail Preserve", 1.0, "Detail"),
        f32_spec("detail.sharpen_amount", 0.0, 150.0, 0.0, "Sharpen", 1.0, "Detail"),
        f32_spec("detail.sharpen_radius", 0.5, 3.0, 1.0, "Sharpen Radius", 0.1, "Detail"),
        f32_spec("detail.sharpen_detail", 0.0, 100.0, 25.0, "Sharpen Detail", 1.0, "Detail"),
        // ---- slot 5: color_grade (schema §3.5) ----
        // Grading model: 0=Perceptual (Oklab, constant-hue), 1=Classic (RGB
        // offset wheels, hue crosstalk), 2=Light (LMS von Kries). Selects HOW
        // the 3-way wheels inject color; identity at zero wheels for all models.
        f32_spec("color_grade.model", 0.0, 2.0, 0.0, "Model", 1.0, "Color Grade"),
        f32_spec("color_grade.shadows_hue", 0.0, 360.0, 0.0, "Shadow Hue", 1.0, "Color Grade"),
        f32_spec("color_grade.shadows_sat", -100.0, 100.0, 0.0, "Shadow Sat", 1.0, "Color Grade"),
        f32_spec("color_grade.shadows_lum", -100.0, 100.0, 0.0, "Shadow Lum", 1.0, "Color Grade"),
        f32_spec("color_grade.midtones_hue", 0.0, 360.0, 0.0, "Mid Hue", 1.0, "Color Grade"),
        f32_spec("color_grade.midtones_sat", -100.0, 100.0, 0.0, "Mid Sat", 1.0, "Color Grade"),
        f32_spec("color_grade.midtones_lum", -100.0, 100.0, 0.0, "Mid Lum", 1.0, "Color Grade"),
        f32_spec("color_grade.highlights_hue", 0.0, 360.0, 0.0, "High Hue", 1.0, "Color Grade"),
        f32_spec("color_grade.highlights_sat", -100.0, 100.0, 0.0, "High Sat", 1.0, "Color Grade"),
        f32_spec("color_grade.highlights_lum", -100.0, 100.0, 0.0, "High Lum", 1.0, "Color Grade"),
        f32_spec("color_grade.global_chroma", -100.0, 100.0, 0.0, "Chroma", 1.0, "Color Grade"),
        f32_spec("color_grade.perceptual_sat", -100.0, 100.0, 0.0, "Saturation", 1.0, "Color Grade"),
        f32_spec("color_grade.shadow_range", 0.0, 1.0, 0.25, "Shadow Range", 0.01, "Color Grade"),
        f32_spec("color_grade.highlight_range", 0.0, 1.0, 0.75, "Highlight Range", 0.01, "Color Grade"),
        // ---- slot 7: tone_curve (schema §3.7) ----
        f32_spec("tone_curve.contrast", -100.0, 100.0, 0.0, "Contrast", 1.0, "Tone"),
        f32_spec("tone_curve.shadows", -100.0, 100.0, 0.0, "Shadows", 1.0, "Tone"),
        f32_spec("tone_curve.darks", -100.0, 100.0, 0.0, "Darks", 1.0, "Tone"),
        f32_spec("tone_curve.lights", -100.0, 100.0, 0.0, "Lights", 1.0, "Tone"),
        f32_spec("tone_curve.highlights", -100.0, 100.0, 0.0, "Highlights", 1.0, "Tone"),
        // ---- slot 8a: lut (3D look LUT). The .cube path lives in
        // meta.lut_file (set via set_lut); only opacity is a doc param. ----
        f32_spec("lut.opacity", 0.0, 100.0, 100.0, "LUT Opacity", 1.0, "LUT"),
    ];
    // tone curve point list (Curve type — UI widget later; ops/assistant now)
    specs.push(ParamSpec {
        path: "tone_curve.points",
        ty: ParamType::Curve,
        min: 0.0,
        max: 1.0,
        default: ParamValue::Curve(vec![]),
        enum_values: None,
        ui: UiHint {
            label: "Curve RGB",
            step: 0.01,
            scale: "linear",
            group: "Tone",
        },
    });
    for (key, label) in [("points_r", "Curve Red"), ("points_g", "Curve Green"), ("points_b", "Curve Blue")] {
        let path: &'static str = Box::leak(format!("tone_curve.{key}").into_boxed_str());
        specs.push(ParamSpec {
            path,
            ty: ParamType::Curve,
            min: 0.0,
            max: 1.0,
            default: ParamValue::Curve(vec![]),
            enum_values: None,
            ui: UiHint {
                label,
                step: 0.01,
                scale: "linear",
                group: "Tone",
            },
        });
    }
    // ---- slot 6: hsl — 8 bands × hue/sat/lum (schema §3.6) ----
    const BANDS: &[(&str, &str)] = &[
        ("red", "Red"),
        ("orange", "Orange"), // skin band — first-class (spec 3.11)
        ("yellow", "Yellow"),
        ("green", "Green"),
        ("aqua", "Aqua"),
        ("blue", "Blue"),
        ("purple", "Purple"),
        ("magenta", "Magenta"),
    ];
    // static paths need 'static strs — build with leaked strings once
    for (band, label) in BANDS {
        for (param, plabel) in [("hue", "Hue"), ("sat", "Sat"), ("lum", "Lum")] {
            let path: &'static str =
                Box::leak(format!("hsl.{band}.{param}").into_boxed_str());
            let lbl: &'static str = Box::leak(format!("{label} {plabel}").into_boxed_str());
            specs.push(f32_spec(path, -100.0, 100.0, 0.0, lbl, 1.0, "HSL"));
        }
    }
    // ---- crop (non-destructive reframe) ----
    specs.extend([
        f32_spec("crop.left", 0.0, 1.0, 0.0, "Left", 0.001, "Crop"),
        f32_spec("crop.top", 0.0, 1.0, 0.0, "Top", 0.001, "Crop"),
        f32_spec("crop.right", 0.0, 1.0, 1.0, "Right", 0.001, "Crop"),
        f32_spec("crop.bottom", 0.0, 1.0, 1.0, "Bottom", 0.001, "Crop"),
        f32_spec("crop.angle", -45.0, 45.0, 0.0, "Angle", 0.01, "Crop"),
        f32_spec("crop.rotate_90", 0.0, 3.0, 0.0, "Rotate 90", 1.0, "Crop"),
        f32_spec("crop.flip_h", 0.0, 1.0, 0.0, "Flip H", 1.0, "Crop"),
        f32_spec("crop.flip_v", 0.0, 1.0, 0.0, "Flip V", 1.0, "Crop"),
        f32_spec("crop.aspect_locked", 0.0, 1.0, 0.0, "Aspect Lock", 1.0, "Crop"),
        f32_spec("crop.aspect_w", 0.0, 100.0, 0.0, "Aspect W", 1.0, "Crop"),
        f32_spec("crop.aspect_h", 0.0, 100.0, 0.0, "Aspect H", 1.0, "Crop"),
        f32_spec("crop.constrain_crop", 0.0, 1.0, 1.0, "Constrain", 1.0, "Crop"),
    ]);
    for s in specs {
        m.insert(s.path, s);
    }
    m
}

static REGISTRY: OnceLock<BTreeMap<&'static str, ParamSpec>> = OnceLock::new();

pub fn registry() -> &'static BTreeMap<&'static str, ParamSpec> {
    REGISTRY.get_or_init(build_registry)
}

/// Register additional specs (P3 modules call this at startup… in practice
/// P3 extends build_registry; kept simple while specs are static).
pub fn all_specs() -> Vec<&'static ParamSpec> {
    registry().values().collect()
}

/// A resolved param path: optional mask scope + module + param + spec.
pub struct ResolvedPath {
    pub mask_id: Option<String>,
    pub module: String,
    pub param: String,
    pub spec: &'static ParamSpec,
}

/// Resolve "exposure.stops" or "mask.<id>.exposure.stops" → spec.
/// Mask-scoped params reuse the global spec (contract A2).
pub fn resolve(path: &str) -> Option<ResolvedPath> {
    let (mask_id, rest) = if let Some(stripped) = path.strip_prefix("mask.") {
        let (id, rest) = stripped.split_once('.')?;
        (Some(id.to_string()), rest)
    } else {
        (None, path)
    };
    let (module, param) = rest.split_once('.')?;
    let spec = registry().get(rest)?;
    Some(ResolvedPath {
        mask_id,
        module: module.to_string(),
        param: param.to_string(),
        spec,
    })
}

/// Default for a (module, param) — absent in doc means this value.
pub fn default_of(module: &str, param: &str) -> Option<&'static ParamValue> {
    registry()
        .get(format!("{module}.{param}").as_str())
        .map(|s| &s.default)
}

/// Effective value: doc value or registry default.
pub fn effective_f32(doc: &crate::doc::EditDoc, module: &str, param: &str) -> f32 {
    doc.get(module, param)
        .and_then(|v| v.as_f32())
        .or_else(|| default_of(module, param).and_then(|v| v.as_f32()))
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spec_has_valid_range_and_default() {
        for s in all_specs() {
            assert!(s.min < s.max, "{}: bad range", s.path);
            if let ParamValue::F32(d) = s.default {
                assert!(
                    d >= s.min && d <= s.max,
                    "{}: default outside range",
                    s.path
                );
            }
        }
    }

    #[test]
    fn resolve_global_and_mask_paths() {
        let g = resolve("exposure.stops").unwrap();
        assert!(g.mask_id.is_none());
        assert_eq!(g.module, "exposure");

        let m = resolve("mask.m-123.exposure.stops").unwrap();
        assert_eq!(m.mask_id.as_deref(), Some("m-123"));
        assert_eq!(m.param, "stops");
        assert_eq!(m.spec.path, "exposure.stops");
    }

    #[test]
    fn unknown_path_rejects() {
        assert!(resolve("nonexistent.param").is_none());
        assert!(resolve("exposure.nope").is_none());
        assert!(resolve("garbage").is_none());
    }

    #[test]
    fn module_order_contains_reference_modules() {
        assert_eq!(MODULE_ORDER[0], "exposure");
        assert_eq!(MODULE_ORDER[1], "white_balance");
    }
}
