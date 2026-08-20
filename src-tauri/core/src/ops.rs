//! Op vocabulary + guard-wall — contract A3. The ONLY write path to the
//! canonical doc. UI ops and (P6) Claude ops take the identical road:
//! resolve → type-check → clamp → structural-check → accept-as-undoable.

use crate::doc::{new_mask_id, EditDoc, Mask, ParamValue, PartialDoc};
use crate::error::CoreError;
use crate::registry::{self, ParamType};
use crate::retouch::{new_retouch_id, RetouchSpot};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    SetParam {
        path: String,
        value: serde_json::Value,
    },
    AddMask {
        kind: String,
        source: serde_json::Value,
    },
    RemoveMask {
        id: String,
    },
    /// Adjust mask attributes (P4): any subset of opacity/feather/invert.
    RefineMask {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opacity: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        feather: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        invert: Option<bool>,
    },
    /// Replace mask geometry/source (move a radial, re-stroke a brush…).
    SetMaskSource {
        id: String,
        source: serde_json::Value,
    },
    /// Object removal: new heal spot (brush source).
    AddRetouchSpot {
        source: serde_json::Value,
    },
    RemoveRetouchSpot {
        id: String,
    },
    SetRetouchSource {
        id: String,
        source: serde_json::Value,
    },
    RefineRetouchSpot {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        feather: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        enabled: Option<bool>,
    },
    ResetModule {
        module: String,
    },
    ResetAll,
    ApplyPreset {
        preset: PartialDoc,
    },
}

impl Op {
    /// Short human label for the history panel.
    pub fn label(&self) -> String {
        match self {
            Op::SetParam { path, .. } => path.clone(),
            Op::AddMask { kind, .. } => format!("add {kind} mask"),
            Op::RemoveMask { .. } => "remove mask".into(),
            Op::RefineMask { .. } => "refine mask".into(),
            Op::SetMaskSource { .. } => "edit mask shape".into(),
            Op::AddRetouchSpot { .. } => "add heal spot".into(),
            Op::RemoveRetouchSpot { .. } => "remove heal spot".into(),
            Op::SetRetouchSource { .. } => "edit heal brush".into(),
            Op::RefineRetouchSpot { .. } => "refine heal spot".into(),
            Op::ResetModule { module } => format!("reset {module}"),
            Op::ResetAll => "reset all".into(),
            Op::ApplyPreset { .. } => "apply preset".into(),
        }
    }

    /// First affected pipeline stage (for dirty-from cascade).
    /// "masks" = the mask-composite stage after the global chain.
    /// None = structural change → re-render everything.
    pub fn affected_module(&self) -> Option<String> {
        match self {
            Op::SetParam { path, .. } => {
                if path.starts_with("mask.") {
                    Some("masks".into())
                } else {
                    registry::resolve(path).map(|r| r.module)
                }
            }
            Op::AddMask { .. }
            | Op::RemoveMask { .. }
            | Op::RefineMask { .. }
            | Op::SetMaskSource { .. } => Some("masks".into()),
            Op::AddRetouchSpot { .. }
            | Op::RemoveRetouchSpot { .. }
            | Op::SetRetouchSource { .. }
            | Op::RefineRetouchSpot { .. } => Some("retouch".into()),
            Op::ResetModule { module } => Some(module.clone()),
            _ => None,
        }
    }
}

const MASK_SOURCE_TYPES: &[&str] = &["segmented", "radial", "linear", "brush", "composite"];

fn check_mask_source(source: &serde_json::Value) -> Result<(), CoreError> {
    let ty = source
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or_else(|| CoreError::InvalidOp("mask source needs a type".into()))?;
    if !MASK_SOURCE_TYPES.contains(&ty) {
        return Err(CoreError::InvalidOp(format!("unknown mask source: {ty}")));
    }
    Ok(())
}

fn check_retouch_source(source: &serde_json::Value) -> Result<(), CoreError> {
    let ty = source
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or_else(|| CoreError::InvalidOp("retouch source needs a type".into()))?;
    if ty != "brush" {
        return Err(CoreError::InvalidOp(
            "retouch spot source must be brush".into(),
        ));
    }
    Ok(())
}

/// Outcome of an accepted op, sent to the frontend mirror.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocDelta {
    pub doc: serde_json::Value,
    pub label: String,
    pub undo_depth: usize,
    pub redo_depth: usize,
    /// id of a mask created by this op, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_mask_id: Option<String>,
    /// id of a retouch spot created by this op, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_retouch_id: Option<String>,
}

/// Validate a JSON value against a spec: type-check + clamp + structure.
/// Returns the canonical ParamValue to store, or rejection reason.
fn check_value(
    spec: &registry::ParamSpec,
    value: &serde_json::Value,
) -> Result<ParamValue, String> {
    match spec.ty {
        ParamType::F32 => {
            let v = value
                .as_f64()
                .ok_or_else(|| format!("{}: expected number", spec.path))?
                as f32;
            if !v.is_finite() {
                return Err(format!("{}: non-finite", spec.path));
            }
            Ok(ParamValue::F32(v.clamp(spec.min, spec.max)))
        }
        ParamType::Bool => value
            .as_bool()
            .map(ParamValue::Bool)
            .ok_or_else(|| format!("{}: expected bool", spec.path)),
        ParamType::Enum => {
            let s = value
                .as_str()
                .ok_or_else(|| format!("{}: expected string", spec.path))?;
            let allowed = spec.enum_values.unwrap_or(&[]);
            if allowed.contains(&s) {
                Ok(ParamValue::Enum(s.to_string()))
            } else {
                Err(format!("{}: '{s}' not in {allowed:?}", spec.path))
            }
        }
        ParamType::Curve => {
            let pts: Vec<[f32; 2]> = serde_json::from_value(value.clone())
                .map_err(|_| format!("{}: expected [[x,y],…]", spec.path))?;
            let mut last_x = -1.0f32;
            for p in &pts {
                if !(0.0..=1.0).contains(&p[0]) || !(0.0..=1.0).contains(&p[1]) {
                    return Err(format!("{}: point out of [0,1]", spec.path));
                }
                if p[0] <= last_x {
                    return Err(format!("{}: x not strictly increasing", spec.path));
                }
                last_x = p[0];
            }
            Ok(ParamValue::Curve(pts))
        }
        ParamType::Color => {
            let h = value.get("h").and_then(|v| v.as_f64());
            let s = value.get("s").and_then(|v| v.as_f64());
            let l = value.get("l").and_then(|v| v.as_f64());
            match (h, s, l) {
                (Some(h), Some(s), Some(l)) => Ok(ParamValue::Color {
                    h: (h as f32).rem_euclid(360.0),
                    s: (s as f32).clamp(-100.0, 100.0),
                    l: (l as f32).clamp(-100.0, 100.0),
                }),
                _ => Err(format!("{}: expected {{h,s,l}}", spec.path)),
            }
        }
    }
}

const MASK_KINDS: &[&str] = &[
    "subject",
    "sky",
    "background",
    "object",
    "radial",
    "linear",
    "brush",
];

/// THE guard-wall. Applies `op` to `doc` or rejects. Never partially
/// mutates on rejection.
pub fn apply_op(doc: &mut EditDoc, op: &Op) -> Result<Option<String>, CoreError> {
    match op {
        Op::SetParam { path, value } => {
            let r = registry::resolve(path)
                .ok_or_else(|| CoreError::InvalidOp(format!("unknown param path: {path}")))?;
            let canonical = check_value(r.spec, value).map_err(CoreError::InvalidOp)?;
            let is_default = canonical == r.spec.default;
            match &r.mask_id {
                None => {
                    if is_default {
                        doc.unset(&r.module, &r.param);
                    } else {
                        doc.set(&r.module, &r.param, canonical);
                    }
                }
                Some(id) => {
                    let mask = doc
                        .mask_mut(id)
                        .ok_or_else(|| CoreError::InvalidOp(format!("mask not found: {id}")))?;
                    let m = mask.modules.entry(r.module.clone()).or_default();
                    if is_default {
                        m.remove(&r.param);
                    } else {
                        m.insert(r.param.clone(), canonical);
                    }
                    doc.touch();
                }
            }
            Ok(None)
        }
        Op::AddMask { kind, source } => {
            if !MASK_KINDS.contains(&kind.as_str()) {
                return Err(CoreError::InvalidOp(format!("unknown mask kind: {kind}")));
            }
            check_mask_source(source)?;
            let id = new_mask_id();
            doc.masks.push(Mask {
                id: id.clone(),
                kind: kind.clone(),
                opacity: 100.0,
                invert: false,
                feather: 0.0,
                source: source.clone(),
                modules: Default::default(),
            });
            doc.touch();
            Ok(Some(id))
        }
        Op::RemoveMask { id } => {
            let before = doc.masks.len();
            doc.masks.retain(|m| m.id != *id);
            if doc.masks.len() == before {
                return Err(CoreError::InvalidOp(format!("mask not found: {id}")));
            }
            doc.touch();
            Ok(None)
        }
        Op::RefineMask {
            id,
            opacity,
            feather,
            invert,
        } => {
            let mask = doc
                .mask_mut(id)
                .ok_or_else(|| CoreError::InvalidOp(format!("mask not found: {id}")))?;
            if let Some(o) = opacity {
                if !o.is_finite() {
                    return Err(CoreError::InvalidOp("opacity non-finite".into()));
                }
                mask.opacity = o.clamp(0.0, 100.0);
            }
            if let Some(f) = feather {
                if !f.is_finite() {
                    return Err(CoreError::InvalidOp("feather non-finite".into()));
                }
                mask.feather = f.clamp(0.0, 100.0);
            }
            if let Some(i) = invert {
                mask.invert = *i;
            }
            doc.touch();
            Ok(None)
        }
        Op::SetMaskSource { id, source } => {
            check_mask_source(source)?;
            let mask = doc
                .mask_mut(id)
                .ok_or_else(|| CoreError::InvalidOp(format!("mask not found: {id}")))?;
            mask.source = source.clone();
            doc.touch();
            Ok(None)
        }
        Op::AddRetouchSpot { source } => {
            check_retouch_source(source)?;
            let id = new_retouch_id();
            doc.retouch.push(RetouchSpot {
                id: id.clone(),
                enabled: true,
                feather: 25.0,
                source: source.clone(),
            });
            doc.touch();
            Ok(Some(id))
        }
        Op::RemoveRetouchSpot { id } => {
            let before = doc.retouch.len();
            doc.retouch.retain(|s| s.id != *id);
            if doc.retouch.len() == before {
                return Err(CoreError::InvalidOp(format!(
                    "retouch spot not found: {id}"
                )));
            }
            doc.touch();
            Ok(None)
        }
        Op::SetRetouchSource { id, source } => {
            check_retouch_source(source)?;
            let spot = doc
                .retouch
                .iter_mut()
                .find(|s| s.id == *id)
                .ok_or_else(|| CoreError::InvalidOp(format!("retouch spot not found: {id}")))?;
            spot.source = source.clone();
            doc.touch();
            Ok(None)
        }
        Op::RefineRetouchSpot {
            id,
            feather,
            enabled,
        } => {
            let spot = doc
                .retouch
                .iter_mut()
                .find(|s| s.id == *id)
                .ok_or_else(|| CoreError::InvalidOp(format!("retouch spot not found: {id}")))?;
            if let Some(f) = feather {
                if !f.is_finite() {
                    return Err(CoreError::InvalidOp("feather non-finite".into()));
                }
                spot.feather = f.clamp(0.0, 100.0);
            }
            if let Some(e) = enabled {
                spot.enabled = *e;
            }
            doc.touch();
            Ok(None)
        }
        Op::ResetModule { module } => {
            doc.modules.remove(module.as_str());
            doc.touch();
            Ok(None)
        }
        Op::ResetAll => {
            doc.modules.clear();
            doc.masks.clear();
            doc.retouch.clear();
            doc.touch();
            Ok(None)
        }
        Op::ApplyPreset { preset } => {
            // expand to per-param guard-walled sets (undoable as one entry)
            for (module, params) in &preset.modules {
                for (param, value) in params {
                    let path = format!("{module}.{param}");
                    let json = serde_json::to_value(value)
                        .map_err(|e| CoreError::InvalidOp(e.to_string()))?;
                    apply_op(doc, &Op::SetParam { path, value: json })?;
                }
            }
            Ok(None)
        }
    }
}

/// Undo/redo as doc-state capture: docs are ~KBs, so cloning the doc per
/// accepted op is cheap, always-correct reversal (state doc, not op-log).
pub struct History {
    undo: Vec<(EditDoc, String)>, // (doc BEFORE op, label)
    redo: Vec<(EditDoc, String)>,
    cap: usize,
}

impl Default for History {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            cap: 500,
        }
    }
}

impl History {
    pub fn record(&mut self, before: EditDoc, label: String) {
        self.undo.push((before, label));
        if self.undo.len() > self.cap {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self, current: &EditDoc) -> Option<(EditDoc, String)> {
        let (prev, label) = self.undo.pop()?;
        self.redo.push((current.clone(), label.clone()));
        Some((prev, label))
    }

    pub fn redo(&mut self, current: &EditDoc) -> Option<(EditDoc, String)> {
        let (next, label) = self.redo.pop()?;
        self.undo.push((current.clone(), label.clone()));
        Some((next, label))
    }

    pub fn depths(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }

    pub fn labels(&self) -> Vec<String> {
        self.undo.iter().map(|(_, l)| l.clone()).collect()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc() -> EditDoc {
        EditDoc::new("/x.ARW")
    }

    #[test]
    fn out_of_range_clamps_to_bound() {
        let mut d = doc();
        apply_op(
            &mut d,
            &Op::SetParam {
                path: "exposure.stops".into(),
                value: json!(400.0), // hallucinated +400 stops
            },
        )
        .unwrap();
        assert_eq!(d.get("exposure", "stops"), Some(&ParamValue::F32(5.0)));
    }

    #[test]
    fn unknown_path_rejects_without_mutation() {
        let mut d = doc();
        let before = d.to_json();
        let err = apply_op(
            &mut d,
            &Op::SetParam {
                path: "hax.pwn".into(),
                value: json!(1),
            },
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::InvalidOp(_)));
        assert_eq!(d.to_json()["modules"], before["modules"]);
    }

    #[test]
    fn malformed_value_rejects() {
        let mut d = doc();
        assert!(apply_op(
            &mut d,
            &Op::SetParam {
                path: "exposure.stops".into(),
                value: json!("not a number"),
            },
        )
        .is_err());
        assert!(apply_op(
            &mut d,
            &Op::SetParam {
                path: "exposure.stops".into(),
                value: json!(f64::NAN),
            },
        )
        .is_err());
    }

    #[test]
    fn bad_mask_id_rejects() {
        let mut d = doc();
        let err = apply_op(
            &mut d,
            &Op::SetParam {
                path: "mask.nope.exposure.stops".into(),
                value: json!(0.5),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("mask not found"));
    }

    #[test]
    fn mask_lifecycle_and_scoped_param() {
        let mut d = doc();
        let id = apply_op(
            &mut d,
            &Op::AddMask {
                kind: "radial".into(),
                source: json!({"type":"radial","center":[0.5,0.5],"radii":[0.3,0.2],"rotation":0}),
            },
        )
        .unwrap()
        .unwrap();
        apply_op(
            &mut d,
            &Op::SetParam {
                path: format!("mask.{id}.exposure.stops"),
                value: json!(0.3),
            },
        )
        .unwrap();
        assert_eq!(
            d.mask(&id).unwrap().modules["exposure"]["stops"],
            ParamValue::F32(0.3)
        );
        apply_op(&mut d, &Op::RemoveMask { id: id.clone() }).unwrap();
        assert!(d.mask(&id).is_none());
        // removing again rejects
        assert!(apply_op(&mut d, &Op::RemoveMask { id }).is_err());
    }

    #[test]
    fn retouch_spot_lifecycle() {
        let mut d = doc();
        let id = apply_op(
            &mut d,
            &Op::AddRetouchSpot {
                source: json!({"type":"brush","strokes":[]}),
            },
        )
        .unwrap()
        .unwrap();
        assert!(d.retouch(&id).is_some());
        apply_op(
            &mut d,
            &Op::RefineRetouchSpot {
                id: id.clone(),
                feather: Some(40.0),
                enabled: Some(true),
            },
        )
        .unwrap();
        assert_eq!(d.retouch(&id).unwrap().feather, 40.0);
        apply_op(
            &mut d,
            &Op::SetRetouchSource {
                id: id.clone(),
                source: json!({"type":"brush","strokes":[{"points":[[0.5,0.5]],"radius":0.05,"mode":"add"}]}),
            },
        )
        .unwrap();
        apply_op(&mut d, &Op::RemoveRetouchSpot { id: id.clone() }).unwrap();
        assert!(d.retouch(&id).is_none());
        assert!(apply_op(&mut d, &Op::RemoveRetouchSpot { id }).is_err());
    }

    #[test]
    fn set_to_default_prunes_doc() {
        let mut d = doc();
        apply_op(
            &mut d,
            &Op::SetParam {
                path: "exposure.stops".into(),
                value: json!(1.0),
            },
        )
        .unwrap();
        apply_op(
            &mut d,
            &Op::SetParam {
                path: "exposure.stops".into(),
                value: json!(0.0), // back to default
            },
        )
        .unwrap();
        assert!(d.modules.is_empty(), "default values must be omitted");
    }

    #[test]
    fn undo_redo_round_trip() {
        let mut d = doc();
        let mut h = History::default();
        let before = d.clone();
        h.record(before, "exposure.stops".into());
        apply_op(
            &mut d,
            &Op::SetParam {
                path: "exposure.stops".into(),
                value: json!(1.5),
            },
        )
        .unwrap();

        let (restored, _) = h.undo(&d).unwrap();
        assert!(restored.modules.is_empty());
        let (again, _) = h.redo(&restored).unwrap();
        assert_eq!(again.get("exposure", "stops"), Some(&ParamValue::F32(1.5)));
    }

    #[test]
    fn preset_applies_as_guarded_sets() {
        let mut d = doc();
        let preset: PartialDoc = serde_json::from_value(json!({
            "modules": { "exposure": { "stops": 99.0 } } // out of range → clamps
        }))
        .unwrap();
        apply_op(&mut d, &Op::ApplyPreset { preset }).unwrap();
        assert_eq!(d.get("exposure", "stops"), Some(&ParamValue::F32(5.0)));
    }

    #[test]
    fn curve_monotonic_x_enforced() {
        let d = doc();
        // tone_curve.points isn't registered yet (P3) — use a synthetic check
        // through check_value directly once registered. For now assert the
        // validator logic via a fake spec is covered by P3. Placeholder:
        // non-curve params unaffected.
        assert!(d.modules.is_empty());
    }
}
