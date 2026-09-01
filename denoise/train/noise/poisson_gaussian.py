"""Poisson–Gaussian noise model (matches meratech-core denoise/profile.rs heuristics)."""

from __future__ import annotations

import numpy as np


def profile_from_iso(iso: float) -> tuple[float, float]:
    """Return (a, b) variance coefficients: var ≈ a * signal + b."""
    iso = max(50.0, float(iso))
    # Calibrated to match Rust NoiseProfile::from_iso rough curve.
    a = 2.5e-5 * (iso / 100.0) ** 1.15
    b = 1.2e-6 * (iso / 100.0) ** 1.8
    return a, b


def add_poisson_gaussian(
    clean: np.ndarray,
    iso: float,
    seed: int | None = None,
) -> np.ndarray:
    """
    Add signal-dependent noise to linear RGB in [0, 1].
    clean: H×W×3 float32
    """
    rng = np.random.default_rng(seed)
    a, b = profile_from_iso(iso)
    out = clean.astype(np.float32, copy=True)
    # Per-channel independent noise (camera-like).
    signal = np.clip(out, 0.0, 1.0)
    var = np.clip(a * signal + b, 1e-8, None)
    sigma = np.sqrt(var)
    gauss = rng.normal(0.0, 1.0, size=out.shape).astype(np.float32) * sigma
    # Poisson shot component via Gaussian approximation (fast, stable).
    shot = rng.normal(0.0, 1.0, size=out.shape).astype(np.float32) * np.sqrt(
        np.clip(a * signal, 0.0, None)
    )
    noisy = signal + gauss * 0.35 + shot * 0.65
    return np.clip(noisy, 0.0, 1.0)


def noise_level_map(iso: float, shape: tuple[int, ...]) -> np.ndarray:
    """Normalized log-ISO map (channel 4 for MeraNoise v1)."""
    log_iso = np.log2(max(50.0, iso) / 100.0)
    # Map ISO 100..25600 → ~0..1
    norm = np.clip((log_iso + 0.0) / 8.0, 0.0, 1.0).astype(np.float32)
    return np.full(shape, norm, dtype=np.float32)
