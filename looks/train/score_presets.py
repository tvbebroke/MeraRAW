#!/usr/bin/env python3
"""
Rank MeraRAW preset JSON files for a photo using ColorChecker-aware heuristics.

Without a detected chart, falls back to mild aesthetic priors on the preset
parameters alone (safe defaults). With --chart-json patch RGB samples, scores
ΔE toward sRGB ColorChecker Classic reference means.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

# Approximate sRGB ColorChecker Classic patch means (D65-ish display referred).
# Order: dark skin … black (24 patches). Used when --chart-json provides samples.
CC_REF_SRGB = [
    (115, 82, 68),
    (194, 150, 130),
    (98, 122, 157),
    (87, 108, 67),
    (133, 128, 177),
    (103, 189, 170),
    (214, 126, 44),
    (80, 91, 166),
    (193, 90, 99),
    (94, 60, 108),
    (157, 188, 64),
    (224, 163, 46),
    (56, 61, 150),
    (70, 148, 73),
    (175, 54, 60),
    (231, 199, 31),
    (187, 86, 149),
    (8, 133, 161),
    (243, 243, 242),
    (200, 200, 200),
    (160, 160, 160),
    (122, 122, 121),
    (85, 85, 85),
    (52, 52, 52),
]


def load_preset(path: Path) -> dict:
    raw = json.loads(path.read_text())
    return {
        "id": path.stem,
        "label": raw.get("label") or path.stem,
        "tags": raw.get("tags") or [],
        "modules": raw.get("modules") or {},
    }


def mget(modules: dict, mod: str, key: str, default: float = 0.0) -> float:
    block = modules.get(mod) or {}
    v = block.get(key, default)
    return float(v) if isinstance(v, (int, float)) else default


def taste_score(modules: dict) -> float:
    """Higher is better. Penalize extreme / crushed looks for general 'recommended'."""
    score = 50.0
    exp = mget(modules, "exposure", "stops")
    contrast = mget(modules, "tone_curve", "contrast")
    sat = mget(modules, "color_grade", "global_chroma")
    vib = mget(modules, "color_grade", "perceptual_sat")
    temp = mget(modules, "white_balance", "temp", 5500)

    score -= abs(exp) * 8
    score -= max(0.0, abs(contrast) - 35) * 0.6
    score -= max(0.0, abs(sat) - 40) * 0.5
    score -= max(0.0, abs(vib) - 45) * 0.4
    if temp:
        score -= abs(temp - 5500) / 250.0
    # Mild lift is often preferred for starters
    shadows = mget(modules, "tone_curve", "shadows")
    if 5 <= shadows <= 35:
        score += 4
    return score


def rgb_to_lab(r: float, g: float, b: float) -> tuple[float, float, float]:
    """Rough sRGB→Lab (D65) for ΔE scoring — good enough for ranking."""
    def lin(c: float) -> float:
        c = c / 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    r, g, b = lin(r), lin(g), lin(b)
    x = r * 0.4124 + g * 0.3576 + b * 0.1805
    y = r * 0.2126 + g * 0.7152 + b * 0.0722
    z = r * 0.0193 + g * 0.1192 + b * 0.9505
    xn, yn, zn = 0.95047, 1.0, 1.08883

    def f(t: float) -> float:
        return t ** (1 / 3) if t > 0.008856 else (7.787 * t + 16 / 116)

    fx, fy, fz = f(x / xn), f(y / yn), f(z / zn)
    L = 116 * fy - 16
    a = 500 * (fx - fy)
    bb = 200 * (fy - fz)
    return L, a, bb


def delta_e76(a: tuple[float, float, float], b: tuple[float, float, float]) -> float:
    return math.sqrt(sum((x - y) ** 2 for x, y in zip(a, b)))


def chart_score(samples: list[list[float]]) -> float:
    """samples: list of [R,G,B] length ≤ 24. Higher = closer to reference."""
    n = min(len(samples), len(CC_REF_SRGB))
    if n == 0:
        return 0.0
    errs = []
    for i in range(n):
        s = samples[i]
        if len(s) < 3:
            continue
        lab_s = rgb_to_lab(s[0], s[1], s[2])
        lab_r = rgb_to_lab(*CC_REF_SRGB[i])
        errs.append(delta_e76(lab_s, lab_r))
    if not errs:
        return 0.0
    mean_e = sum(errs) / len(errs)
    # Map ΔE ~0 → 40 pts, ΔE 20 → 0
    return max(0.0, 40.0 - mean_e * 2.0)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--presets-dir", type=Path, default=Path("../../presets/bundled"))
    ap.add_argument(
        "--chart-json",
        type=Path,
        help="Optional 24 [R,G,B] patches for ONE look (prints ΔE fidelity; does not rank all presets)",
    )
    ap.add_argument(
        "--chart-dir",
        type=Path,
        help="Dir of {preset_id}.json patch lists — per-preset chart fidelity for ranking",
    )
    ap.add_argument("--out", type=Path, default=Path("ranked_presets.json"))
    ap.add_argument("--top", type=int, default=10)
    args = ap.parse_args()

    if args.chart_json and args.chart_json.is_file():
        samples = json.loads(args.chart_json.read_text())
        print(f"Chart fidelity score: {chart_score(samples):.1f} (higher = closer to CC ref)")

    presets = []
    for p in sorted(args.presets_dir.glob("*.json")):
        presets.append(load_preset(p))

    ranked = []
    for p in presets:
        t = taste_score(p["modules"])
        c = 0.0
        if args.chart_dir:
            cf = args.chart_dir / f"{p['id']}.json"
            if cf.is_file():
                c = chart_score(json.loads(cf.read_text()))
        total = t + c
        ranked.append(
            {
                "id": p["id"],
                "label": p["label"],
                "tags": p["tags"],
                "taste": round(t, 2),
                "chart": round(c, 2),
                "score": round(total, 2),
            }
        )
    ranked.sort(key=lambda x: x["score"], reverse=True)
    args.out.write_text(json.dumps({"ranked": ranked[: args.top], "all": ranked}, indent=2))
    print(f"Top {args.top}:")
    for row in ranked[: args.top]:
        print(f"  {row['score']:5.1f}  {row['label']}")
    print(f"Wrote {args.out}")


if __name__ == "__main__":
    main()
