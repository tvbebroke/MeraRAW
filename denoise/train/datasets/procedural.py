"""Procedural clean scenes — zero disk, infinite variety for Poisson–Gaussian training."""

from __future__ import annotations

import random

import numpy as np


def _mesh(h: int, w: int) -> tuple[np.ndarray, np.ndarray]:
    y = np.linspace(0, 1, h, dtype=np.float32)[:, None]
    x = np.linspace(0, 1, w, dtype=np.float32)[None, :]
    return y, x


def _scene_gradient(h: int, w: int, rng: random.Random) -> np.ndarray:
    y, x = _mesh(h, w)
    ax, ay = rng.random(), rng.random()
    v = (ax * x + ay * y + rng.uniform(0.05, 0.25)).astype(np.float32)
    return np.stack([v, v * rng.uniform(0.85, 1.05), v * rng.uniform(0.9, 1.1)], axis=-1)


def _scene_edges(h: int, w: int, rng: random.Random) -> np.ndarray:
    img = _scene_gradient(h, w, rng)
    n = rng.randint(2, 8)
    y, x = _mesh(h, w)
    for _ in range(n):
        cx, cy = rng.random(), rng.random()
        r = rng.uniform(0.05, 0.35)
        mask = ((x - cx) ** 2 + (y - cy) ** 2) < r * r
        col = rng.uniform(0.1, 0.9)
        for c in range(3):
            img[..., c] = np.where(mask, col * rng.uniform(0.8, 1.2), img[..., c])
    # Sharp edge band
    if rng.random() > 0.4:
        split = rng.uniform(0.2, 0.8)
        axis = rng.choice(["x", "y"])
        if axis == "x":
            mask = x[0, :] > split
            img[:, mask, :] *= rng.uniform(0.3, 1.4)
        else:
            mask = y[:, 0] > split
            img[mask, :, :] *= rng.uniform(0.3, 1.4)
    return np.clip(img, 0.0, 1.0)


def _scene_texture(h: int, w: int, rng: random.Random) -> np.ndarray:
    """Multi-frequency sinusoid texture (grass/fabric-like)."""
    y, x = _mesh(h, w)
    v = np.zeros((h, w), dtype=np.float32)
    for _ in range(rng.randint(4, 10)):
        fx = rng.uniform(3, 40)
        fy = rng.uniform(3, 40)
        ph = rng.uniform(0, 6.28)
        amp = rng.uniform(0.02, 0.12)
        v += amp * np.sin(2 * np.pi * (fx * x + fy * y) + ph)
    base = rng.uniform(0.15, 0.55)
    v = base + v
    return np.clip(
        np.stack([v, v * rng.uniform(0.92, 1.0), v * rng.uniform(0.95, 1.08)], axis=-1),
        0.0,
        1.0,
    )


def _scene_portrait_proxy(h: int, w: int, rng: random.Random) -> np.ndarray:
    """Smooth skin-tone blob + background — helps skin/hair noise handling."""
    y, x = _mesh(h, w)
    cx, cy = 0.5 + rng.uniform(-0.1, 0.1), 0.45 + rng.uniform(-0.05, 0.05)
    r = rng.uniform(0.18, 0.32)
    dist = np.sqrt((x - cx) ** 2 + (y - cy) ** 2)
    skin = np.exp(-(dist / r) ** 2 * 3.0)
    bg = rng.uniform(0.08, 0.35)
    v = bg + skin * rng.uniform(0.25, 0.55)
    warm = rng.uniform(0.95, 1.05)
    return np.clip(
        np.stack([v * warm, v * 0.92, v * 0.88], axis=-1),
        0.0,
        1.0,
    )


def procedural_clean(h: int, w: int, seed: int) -> np.ndarray:
    rng = random.Random(seed)
    kind = rng.choices(["grad", "edges", "texture", "portrait"], weights=[1, 2, 2, 1], k=1)[0]
    if kind == "grad":
        return _scene_gradient(h, w, rng)
    if kind == "edges":
        return _scene_edges(h, w, rng)
    if kind == "texture":
        return _scene_texture(h, w, rng)
    return _scene_portrait_proxy(h, w, rng)
