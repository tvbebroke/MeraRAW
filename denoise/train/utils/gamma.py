"""Display gamma — matches run_onnx encode/decode in ai/mod.rs."""

from __future__ import annotations

import numpy as np

GAMMA = 2.2


def linear_to_display(x: np.ndarray) -> np.ndarray:
    return np.power(np.clip(x, 0.0, None), 1.0 / GAMMA)


def display_to_linear(x: np.ndarray) -> np.ndarray:
    return np.power(np.clip(x, 0.0, None), GAMMA)
