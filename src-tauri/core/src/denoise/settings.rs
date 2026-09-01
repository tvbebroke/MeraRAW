//! Sidecar-facing denoise settings ↔ resolved `ChainParams`.
//! Params live under the `detail` module (noise slot) for pipeline order
//! compatibility; AI fields share the same module.

use super::cpu::ChainParams;
use super::profile::NoiseProfile;
use crate::doc::EditDoc;
use crate::registry::effective_f32 as eff;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DenoiseMode {
    Off,
    Classical,
    Ai,
    AiPlusClassical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LumaEngine {
    WaveletAuto,
    Wavelet,
    Nlm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecoveryMode {
    Conservative,
    Aggressive,
}

/// Declarative denoise settings (doc 03 §4). Read from / written to EditDoc
/// `detail.*` params so the existing op/sidecar path just works.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DenoiseSettings {
    pub enabled: bool,
    pub mode: DenoiseMode,
    pub ai_amount: f32,
    pub ai_model: String,
    pub luminance: f32,
    pub luma_detail: f32,
    pub chrominance: f32,
    pub chroma_auto: bool,
    pub strength: f32,
    pub engine: LumaEngine,
    pub wavelet_luma_curve: [f32; 6],
    pub wavelet_chroma_curve: [f32; 6],
    pub nlm_patch: u8,
    pub nlm_search: u8,
    pub nlm_center_weight: f32,
    pub impulse: f32,
    pub hot_pixels: bool,
    pub recovery_mode: RecoveryMode,
}

impl Default for DenoiseSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: DenoiseMode::Classical,
            ai_amount: 50.0,
            ai_model: "meranoise-v1".into(),
            luminance: 0.0,
            luma_detail: 50.0,
            chrominance: 0.0,
            chroma_auto: true,
            strength: 1.0,
            engine: LumaEngine::WaveletAuto,
            wavelet_luma_curve: [1.0; 6],
            wavelet_chroma_curve: [1.0; 6],
            nlm_patch: 1,
            nlm_search: 5,
            nlm_center_weight: 30.0,
            impulse: 0.0,
            hot_pixels: true,
            recovery_mode: RecoveryMode::Conservative,
        }
    }
}

impl DenoiseSettings {
    /// Pull live values from the edit doc. Missing params → registry defaults
    /// via `effective_f32`.
    pub fn from_doc(doc: &EditDoc, profile: &NoiseProfile) -> Self {
        let mut s = Self::default();
        let luma = eff(doc, "detail", "noise_luma");
        let chroma = eff(doc, "detail", "noise_chroma");
        let detail = eff(doc, "detail", "detail_preserve");
        s.luminance = luma;
        s.luma_detail = detail;
        s.chrominance = chroma;
        s.chroma_auto = eff(doc, "detail", "chroma_auto") >= 0.5;
        s.strength = eff(doc, "detail", "nr_strength").clamp(0.25, 4.0);
        s.impulse = eff(doc, "detail", "impulse");
        s.hot_pixels = eff(doc, "detail", "hot_pixels") >= 0.5;
        s.ai_amount = eff(doc, "detail", "ai_amount");
        let eng = eff(doc, "detail", "nr_engine");
        s.engine = if eng >= 1.5 {
            LumaEngine::Nlm
        } else if eng >= 0.5 {
            LumaEngine::Wavelet
        } else {
            LumaEngine::WaveletAuto
        };
        s.recovery_mode = if eff(doc, "detail", "nr_aggressive") >= 0.5 {
            RecoveryMode::Aggressive
        } else {
            RecoveryMode::Conservative
        };
        s.nlm_patch = eff(doc, "detail", "nlm_patch").round().clamp(1.0, 3.0) as u8;
        s.nlm_search = eff(doc, "detail", "nlm_search").round().clamp(2.0, 10.0) as u8;
        s.nlm_center_weight = eff(doc, "detail", "nlm_center");

        // Auto chroma (RT-style): when the checkbox is on AND the user has
        // engaged NR (luma/impulse > 0), fill chrominance from the profile.
        // Leaving everything at registry defaults keeps the graph identity
        // (noise node skipped) — chroma-only-by-default would break that.
        let engaged = luma > 0.0 || s.impulse > 0.0 || chroma > 0.0;
        if s.chroma_auto && engaged {
            s.chrominance = auto_chroma(profile).max(chroma);
        }

        // Mode: AI enabled when ai_amount > 0 and ai_enabled flag set.
        let ai_on = eff(doc, "detail", "ai_enabled") >= 0.5;
        let classical_on = luma > 0.0 || s.chrominance > 0.0 || s.impulse > 0.0;
        s.mode = match (ai_on, classical_on) {
            (false, false) => DenoiseMode::Off,
            (true, false) => DenoiseMode::Ai,
            (false, true) => DenoiseMode::Classical,
            (true, true) => DenoiseMode::AiPlusClassical,
        };
        s.enabled = s.mode != DenoiseMode::Off || s.hot_pixels;
        s
    }

    /// Resolve into CPU/GPU chain parameters. `sigma_scale` ≤ 1 for preview.
    pub fn to_chain_params(&self, profile: &NoiseProfile, sigma_scale: f32) -> ChainParams {
        let use_nlm = match self.engine {
            LumaEngine::Nlm => true,
            LumaEngine::Wavelet => false,
            LumaEngine::WaveletAuto => {
                // Auto-pick NLM at very high noise (ISO-ish proxy).
                profile.relative_at_midgray() > 0.04
            }
        };
        ChainParams::from_sliders(
            profile,
            self.strength,
            self.luminance,
            self.chrominance,
            self.luma_detail,
            self.impulse,
            &self.wavelet_luma_curve,
            &self.wavelet_chroma_curve,
            use_nlm,
            self.nlm_patch as i32,
            self.nlm_search as i32,
            self.nlm_center_weight,
            self.recovery_mode == RecoveryMode::Aggressive,
            sigma_scale,
        )
    }

    /// Classical path should run (GPU noise node).
    pub fn classical_active(&self) -> bool {
        matches!(
            self.mode,
            DenoiseMode::Classical | DenoiseMode::AiPlusClassical
        ) && (self.luminance > 0.0 || self.chrominance > 0.0 || self.impulse > 0.0)
    }

    pub fn ai_active(&self) -> bool {
        matches!(self.mode, DenoiseMode::Ai | DenoiseMode::AiPlusClassical) && self.ai_amount > 0.0
    }
}

/// RT-style auto chroma from relative mid-gray noise.
/// ~0 at base ISO, ~40 at ISO 6400, capped at 80.
pub fn auto_chroma(profile: &NoiseProfile) -> f32 {
    let r = profile.relative_at_midgray();
    (r * 800.0).clamp(0.0, 80.0)
}

/// Suggested import defaults from ISO (doc 06 §3).
pub fn import_defaults_for_iso(iso: u32) -> (f32, f32, bool) {
    // (luma, chroma, suggest_ai)
    if iso < 800 {
        (0.0, 0.0, false)
    } else if iso < 3200 {
        (0.0, auto_chroma(&NoiseProfile::from_iso(iso)), false)
    } else if iso < 12800 {
        (15.0, auto_chroma(&NoiseProfile::from_iso(iso)), false)
    } else {
        (20.0, auto_chroma(&NoiseProfile::from_iso(iso)), true)
    }
}
