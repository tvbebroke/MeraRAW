//! Camera profile resolution — match RAW metadata to bundled `.dcp` files.

pub mod dcp;
pub mod hue_sat_map;
pub mod tone_curve;

pub use dcp::DcpProfile;

use crate::error::CoreError;
use crate::raw::ImageMeta;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static PROFILES_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Called once at app startup (Tauri main) with the resolved profiles folder.
pub fn set_profiles_dir(dir: PathBuf) {
    let _ = PROFILES_DIR.set(dir);
}

pub fn profiles_dir() -> PathBuf {
    PROFILES_DIR.get().cloned().unwrap_or_else(|| {
        if let Ok(d) = std::env::var("MERARAW_PROFILES_DIR") {
            return PathBuf::from(d);
        }
        for candidate in [
            "meraraw-derivatives",
            "../meraraw-derivatives",
            "../../meraraw-derivatives",
        ] {
            let p = PathBuf::from(candidate);
            if p.is_dir() {
                return p.canonicalize().unwrap_or(p);
            }
        }
        PathBuf::from("meraraw-derivatives")
    })
}

pub fn normalize_key(s: &str) -> String {
    s.split_whitespace()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub fn camera_model_key(make: &str, model: &str) -> String {
    normalize_key(&format!("{} {}", make.trim(), model.trim()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileRef {
    #[serde(default)]
    pub name: String,
    pub file: String,
}

fn profile_name_from_file(file: &str) -> String {
    Path::new(file)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.to_string())
}

fn profile_ref_from_value(v: serde_json::Value) -> Result<ProfileRef, String> {
    if let Some(s) = v.as_str() {
        return Ok(ProfileRef {
            name: profile_name_from_file(s),
            file: s.to_string(),
        });
    }
    if let Some(obj) = v.as_object() {
        let file = obj
            .get("file")
            .and_then(|f| f.as_str())
            .ok_or("profile entry missing file")?
            .to_string();
        let name = obj
            .get("name")
            .and_then(|n| n.as_str())
            .map(|s| s.trim_end_matches('\0').trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| profile_name_from_file(&file));
        return Ok(ProfileRef { name, file });
    }
    Err("invalid profile entry".into())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileIndex {
    #[serde(default, deserialize_with = "deserialize_cameras")]
    pub cameras: BTreeMap<String, Vec<ProfileRef>>,
}

fn deserialize_cameras<'de, D>(deserializer: D) -> Result<BTreeMap<String, Vec<ProfileRef>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: BTreeMap<String, Vec<serde_json::Value>> =
        BTreeMap::deserialize(deserializer)?;
    raw.into_iter()
        .map(|(k, vals)| {
            let refs = vals
                .into_iter()
                .map(|v| profile_ref_from_value(v).map_err(serde::de::Error::custom))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((k, refs))
        })
        .collect()
}

impl ProfileIndex {
    pub fn embedded() -> Self {
        serde_json::from_str(include_str!("../../models/profile_index.json"))
            .unwrap_or_else(|e| panic!("profile_index.json: {e}"))
    }

    /// Lookup profiles for a camera, case-insensitive on the index key.
    pub fn profiles_for(&self, make: &str, model: &str) -> Option<&[ProfileRef]> {
        let candidates = [
            camera_model_key(make, model),
            normalize_key(model),
            normalize_key(&format!("{} {}", title_case_word(make), model.trim())),
        ];
        for key in &self.cameras {
            let nk = normalize_key(key.0);
            if candidates.iter().any(|c| c == &nk) {
                return Some(key.1.as_slice());
            }
        }
        None
    }
}

fn title_case_word(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        return String::new();
    }
    let mut chars = t.chars();
    let first = chars.next().unwrap().to_uppercase().collect::<String>();
    format!("{first}{}", chars.as_str().to_lowercase())
}

/// Profiles available for this raw's camera metadata.
pub fn resolve_profiles(meta: &ImageMeta, index: &ProfileIndex) -> Vec<ProfileRef> {
    index
        .profiles_for(&meta.camera_make, &meta.camera_model)
        .map(|v| v.to_vec())
        .unwrap_or_default()
}

const DEFAULT_PROFILE_NAMES: &[&str] = &[
    "MeraRAW Standard",
    "Adobe Standard (MeraRAW)",
    "Adobe Standard",
    "Camera Standard",
];

/// Pick the default profile for autoload.
pub fn default_profile(profiles: &[ProfileRef]) -> Option<&ProfileRef> {
    if let Some(p) = profiles
        .iter()
        .find(|p| p.file.contains("MeraRAW Standard"))
    {
        return Some(p);
    }
    for want in DEFAULT_PROFILE_NAMES {
        if let Some(p) = profiles.iter().find(|p| p.name.eq_ignore_ascii_case(want)) {
            return Some(p);
        }
    }
    profiles.first()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveProfile {
    pub name: String,
    pub file: String,
    pub unique_camera_model: String,
}

/// Resolve + load the default DCP for a raw file.
pub fn autoload_profile(
    meta: &ImageMeta,
    index: &ProfileIndex,
) -> Result<Option<ActiveProfile>, CoreError> {
    let profiles = resolve_profiles(meta, index);
    if profiles.is_empty() {
        tracing::info!(
            make = %meta.camera_make,
            model = %meta.camera_model,
            "no DCP profiles matched"
        );
        return Ok(None);
    }
    let Some(chosen) = default_profile(&profiles) else {
        return Ok(None);
    };
    let path = profiles_dir().join(&chosen.file);
    if !path.exists() {
        tracing::warn!(file = %chosen.file, "DCP file missing on disk");
        return Ok(None);
    }
    let dcp = DcpProfile::load(&path)?;
    if !dcp.matches_camera(&meta.camera_make, &meta.camera_model) {
        tracing::warn!(
            profile = %dcp.unique_camera_model,
            make = %meta.camera_make,
            model = %meta.camera_model,
            "DCP camera model mismatch; skipping"
        );
        return Ok(None);
    }
    tracing::info!(
        profile = %dcp.profile_name,
        camera = %dcp.unique_camera_model,
        file = %chosen.file,
        "autoloaded camera profile"
    );
    Ok(Some(ActiveProfile {
        name: profile_display_name(&chosen.file, &dcp.unique_camera_model),
        file: chosen.file.clone(),
        unique_camera_model: dcp.unique_camera_model,
    }))
}

pub fn profile_path(file: &str) -> PathBuf {
    profiles_dir().join(file)
}

pub fn find_profile<'a>(profiles: &'a [ProfileRef], name_or_file: &str) -> Option<&'a ProfileRef> {
    let key = normalize_key(name_or_file);
    profiles.iter().find(|p| {
        normalize_key(&p.name) == key
            || normalize_key(&p.file) == key
            || normalize_key(p.file.strip_suffix(".dcp").unwrap_or(&p.file)) == key
    })
}

/// Pick a profile from sidecar override, explicit name, or the default autoload.
pub fn choose_profile(
    meta: &ImageMeta,
    index: &ProfileIndex,
    override_file: Option<&str>,
) -> Option<ProfileRef> {
    let profiles = resolve_profiles(meta, index);
    if profiles.is_empty() {
        return None;
    }
    if let Some(want) = override_file.filter(|s| !s.is_empty()) {
        if let Some(p) = find_profile(&profiles, want) {
            return Some(p.clone());
        }
    }
    default_profile(&profiles).cloned()
}

/// User-facing label from the `.dcp` filename (e.g. "MeraRAW Standard"), not the
/// internal Adobe ProfileName tag (e.g. "Adobe Standard (MeraRAW)").
pub fn profile_display_name(file: &str, unique_camera_model: &str) -> String {
    let stem = file.strip_suffix(".dcp").unwrap_or(file);
    if let Some(rest) = stem.strip_prefix(&format!("{unique_camera_model} ")) {
        return rest.to_string();
    }
    stem.to_string()
}

pub fn matched_camera_key(meta: &ImageMeta, index: &ProfileIndex) -> Option<String> {
    let candidates = [
        camera_model_key(&meta.camera_make, &meta.camera_model),
        normalize_key(&meta.camera_model),
        normalize_key(&format!(
            "{} {}",
            title_case_word(&meta.camera_make),
            meta.camera_model.trim()
        )),
    ];
    for key in index.cameras.keys() {
        if candidates.iter().any(|c| normalize_key(key) == *c) {
            return Some(key.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_matches_sony_ilce7m4() {
        let index = ProfileIndex::embedded();
        let meta = ImageMeta {
            path: "/x.ARW".into(),
            kind: crate::raw::ImageKind::Raw,
            format: "ARW".into(),
            bit_depth: 0,
            camera_make: "SONY".into(),
            camera_model: "ILCE-7M4".into(),
            lens: None,
            iso: None,
            shutter: None,
            aperture: None,
            focal_mm: None,
            captured_at: None,
            width: 100,
            height: 100,
            orientation: "Normal".into(),
            as_shot_wb: [1.0, 1.0, 1.0],
            estimated_cct: Some(5500.0),
            camera_profile: None,
            available_profiles: Vec::new(),
            available_profile_files: Vec::new(),
            demosaic: String::new(),
            available_demosaic: Vec::new(),
            gps_lat: None,
            gps_lon: None,
            input_color_space: None,
        };
        let profiles = resolve_profiles(&meta, &index);
        assert!(
            !profiles.is_empty(),
            "expected ILCE-7M4 profiles in index"
        );
        assert!(default_profile(&profiles).is_some());
    }

    #[test]
    fn display_name_from_filename_not_adobe_tag() {
        assert_eq!(
            profile_display_name("Sony ILCE-7M4 MeraRAW Standard.dcp", "Sony ILCE-7M4"),
            "MeraRAW Standard"
        );
    }
}
