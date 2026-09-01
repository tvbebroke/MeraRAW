"""Training / eval metrics."""

from __future__ import annotations

import numpy as np


def psnr(clean: np.ndarray, den: np.ndarray, peak: float = 1.0) -> float:
    mse = float(np.mean((clean - den) ** 2))
    if mse < 1e-12:
        return 99.0
    return 10.0 * np.log10((peak * peak) / mse)


def ssim_gray(a: np.ndarray, b: np.ndarray) -> float:
    """Simplified SSIM on luminance channel."""
    a = a.astype(np.float64)
    b = b.astype(np.float64)
    c1, c2 = 0.01**2, 0.03**2
    mu_a, mu_b = a.mean(), b.mean()
    var_a, var_b = a.var(), b.var()
    cov = ((a - mu_a) * (b - mu_b)).mean()
    num = (2 * mu_a * mu_b + c1) * (2 * cov + c2)
    den = (mu_a**2 + mu_b**2 + c1) * (var_a + var_b + c2)
    return float(num / (den + 1e-12))
