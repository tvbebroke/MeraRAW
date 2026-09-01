"""Unified training dataset — composites public sources."""

from __future__ import annotations

import os
import random
from pathlib import Path
from typing import Any

import numpy as np
from PIL import Image
from torch.utils.data import Dataset

from noise.poisson_gaussian import add_poisson_gaussian, noise_level_map
from utils.gamma import linear_to_display

try:
    from datasets.procedural import procedural_clean
except ImportError:
    from procedural import procedural_clean  # type: ignore

IMAGE_EXT = {".png", ".jpg", ".jpeg", ".tif", ".tiff", ".bmp"}


def data_root() -> Path:
    root = os.environ.get("MERANOISE_DATA_ROOT")
    if root:
        return Path(root)
    return Path(__file__).resolve().parents[1] / "data"


def _load_rgb(path: Path) -> np.ndarray:
    img = Image.open(path).convert("RGB")
    arr = np.asarray(img, dtype=np.float32) / 255.0
    return arr


def _random_crop_pair(
    clean: np.ndarray, noisy: np.ndarray, size: int, rng: random.Random
) -> tuple[np.ndarray, np.ndarray]:
    h, w = clean.shape[:2]
    if h < size or w < size:
        scale = max(size / h, size / w) * 1.05
        nh, nw = int(h * scale), int(w * scale)
        clean_img = Image.fromarray((clean * 255).astype(np.uint8)).resize((nw, nh), Image.BICUBIC)
        noisy_img = Image.fromarray((noisy * 255).astype(np.uint8)).resize((nw, nh), Image.BICUBIC)
        clean = np.asarray(clean_img, dtype=np.float32) / 255.0
        noisy = np.asarray(noisy_img, dtype=np.float32) / 255.0
        h, w = clean.shape[:2]
    y = rng.randint(0, h - size)
    x = rng.randint(0, w - size)
    return (
        clean[y : y + size, x : x + size],
        noisy[y : y + size, x : x + size],
    )


class SampleIndex:
    def __init__(self, source: str, path_a: Path, path_b: Path | None, iso: float | None):
        self.source = source
        self.path_a = path_a
        self.path_b = path_b
        self.iso = iso


def _scan_nind(root: Path) -> list[SampleIndex]:
    """NIND: paired files often share base name with _low/_high or in subfolders."""
    out: list[SampleIndex] = []
    base = root / "nind"
    if not base.is_dir():
        return out
    # Common layout from nind-denoise repo exports
    for clean_dir_name in ("clean", "low_iso", "reference"):
        noisy_dir_name = {"clean": "noisy", "low_iso": "high_iso", "reference": "noisy"}.get(
            clean_dir_name, "noisy"
        )
        clean_dir, noisy_dir = base / clean_dir_name, base / noisy_dir_name
        if clean_dir.is_dir() and noisy_dir.is_dir():
            for p in sorted(clean_dir.iterdir()):
                if p.suffix.lower() not in IMAGE_EXT:
                    continue
                q = noisy_dir / p.name
                if q.is_file():
                    out.append(SampleIndex("nind", p, q, None))
    # Flat paired naming: foo_clean.jpg / foo_noisy.jpg
    if not out:
        files = [p for p in base.rglob("*") if p.suffix.lower() in IMAGE_EXT]
        by_stem: dict[str, list[Path]] = {}
        for p in files:
            stem = p.stem.replace("_clean", "").replace("_noisy", "").replace("_GT", "")
            by_stem.setdefault(stem, []).append(p)
        for paths in by_stem.values():
            if len(paths) < 2:
                continue
            paths = sorted(paths, key=lambda x: x.name)
            out.append(SampleIndex("nind", paths[0], paths[-1], None))
    return out


def _scan_sidd_srgb(root: Path, split: str) -> list[SampleIndex]:
    """SIDD sRGB-only PNG pairs if present."""
    out: list[SampleIndex] = []
    base = root / "sidd"
    for sub in ("SIDD_Small_sRGB_Only", "sidd_srgb", "sidd"):
        d = base / sub
        if not d.is_dir():
            continue
        gt = d / "GT"
        noisy = d / "NOISY"
        if gt.is_dir() and noisy.is_dir():
            for p in sorted(gt.glob("*.png")):
                q = noisy / p.name
                if q.is_file():
                    out.append(SampleIndex("sidd", p, q, None))
    # Deterministic val split: last 20% by name
    if split == "val" and out:
        out = sorted(out, key=lambda s: s.path_a.name)
        n = max(1, len(out) // 5)
        return out[-n:]
    if split == "train" and out:
        out = sorted(out, key=lambda s: s.path_a.name)
        n = max(1, len(out) // 5)
        return out[:-n]
    return out


def _scan_clean_only(root: Path, rel: str) -> list[SampleIndex]:
    out: list[SampleIndex] = []
    d = root / rel
    if not d.is_dir():
        return out
    for p in sorted(d.rglob("*")):
        if p.suffix.lower() in IMAGE_EXT:
            out.append(SampleIndex("synthetic", p, None, None))
    return out


def _scan_sid(root: Path) -> list[SampleIndex]:
    """SID short/long RAW pairs — requires rawpy at train time."""
    out: list[SampleIndex] = []
    base = root / "sid"
    for cam in ("Sony", "Fuji"):
        short = base / cam / "short"
        long = base / cam / "long"
        if not short.is_dir() or not long.is_dir():
            continue
        long_by_stem = {p.stem.split(".")[0]: p for p in long.iterdir()}
        for sp in short.iterdir():
            key = sp.stem.split(".")[0]
            # SID naming: 10003_00_0.04s.ARW pairs with 10003_00_10s.ARW — match prefix
            prefix = "_".join(key.split("_")[:2])
            lp = next((v for k, v in long_by_stem.items() if k.startswith(prefix)), None)
            if lp:
                out.append(SampleIndex("sid", lp, sp, 6400.0))
    return out


def build_index(root: Path, specs: list[dict[str, Any]], split: str) -> list[SampleIndex]:
    indices: list[SampleIndex] = []
    for spec in specs:
        name = spec["name"]
        w = float(spec.get("weight", 1.0))
        if name == "nind":
            indices.extend(_scan_nind(root) * int(max(1, round(w))))
        elif name == "sidd":
            indices.extend(_scan_sidd_srgb(root, split) * int(max(1, round(w))))
        elif name == "sid":
            indices.extend(_scan_sid(root) * int(max(1, round(w))))
        elif name in ("synthetic_div2k", "div2k"):
            rel = spec.get("clean_root", "div2k/DIV2K_train_HR")
            indices.extend(_scan_clean_only(root, rel) * int(max(1, round(w * 2))))
        elif name == "flickr2k":
            indices.extend(_scan_clean_only(root, "flickr2k/Flickr2K") * int(max(1, round(w))))
        elif name == "procedural":
            n = int(spec.get("virtual_samples", 20000))
            if split == "val":
                n = min(512, n // 40)
            for i in range(n):
                indices.append(SampleIndex("procedural", Path(f"virt_{i}"), None, None))
    return indices


def _load_sid_pair(clean_path: Path, noisy_path: Path) -> tuple[np.ndarray, np.ndarray]:
    try:
        import rawpy
    except ImportError as e:
        raise RuntimeError("rawpy required for SID — pip install rawpy") from e

    def raw_to_rgb(path: Path) -> np.ndarray:
        with rawpy.imread(str(path)) as raw:
            rgb = raw.postprocess(
                use_camera_wb=True,
                no_auto_bright=True,
                output_bps=16,
                gamma=(1, 1),
                user_flip=0,
            )
        return (rgb.astype(np.float32) / 65535.0).clip(0.0, 1.0)

    return raw_to_rgb(clean_path), raw_to_rgb(noisy_path)


class MeraNoiseDataset(Dataset):
    def __init__(
        self,
        specs: list[dict[str, Any]],
        split: str = "train",
        patch_size: int = 256,
        iso_range: tuple[int, int] = (100, 25600),
        seed: int = 0,
        virtual_samples: int = 0,
    ) -> None:
        self.root = data_root()
        self.patch_size = patch_size
        self.iso_range = iso_range
        self.split = split
        self.rng = random.Random(seed + (1 if split == "val" else 0))
        # Inject virtual_samples into procedural specs when set at top level.
        specs = [dict(s) for s in specs]
        if virtual_samples > 0:
            for s in specs:
                if s.get("name") == "procedural":
                    s.setdefault("virtual_samples", virtual_samples)
        self.indices = build_index(self.root, specs, split)
        if not self.indices:
            raise FileNotFoundError(
                f"No training samples under {self.root}. "
                "Run scripts/download-denoise-datasets.sh and set MERANOISE_DATA_ROOT, "
                "or use procedural dataset in config."
            )

    def __len__(self) -> int:
        return len(self.indices)

    def __getitem__(self, idx: int) -> dict[str, Any]:
        item = self.indices[idx % len(self.indices)]
        iso = item.iso
        if item.source == "procedural":
            seed = (idx * 7919 + (17 if self.split == "val" else 3)) & 0x7FFFFFFF
            big = self.patch_size + 64
            clean = procedural_clean(big, big, seed)
            iso = float(self.rng.randint(self.iso_range[0], self.iso_range[1]))
            noisy = add_poisson_gaussian(clean, iso, seed=seed + 1)
        elif item.source == "sid" and item.path_b:
            clean, noisy = _load_sid_pair(item.path_a, item.path_b)
            iso = iso or 6400.0
        elif item.path_b:
            clean = _load_rgb(item.path_a)
            noisy = _load_rgb(item.path_b)
            iso = iso or float(self.rng.randint(self.iso_range[0], self.iso_range[1]))
        else:
            clean = _load_rgb(item.path_a)
            iso = float(self.rng.randint(self.iso_range[0], self.iso_range[1]))
            noisy = add_poisson_gaussian(clean, iso, seed=self.rng.randint(0, 2**31 - 1))

        clean, noisy = _random_crop_pair(clean, noisy, self.patch_size, self.rng)
        # Train in display gamma (matches ONNX runtime).
        clean_d = linear_to_display(clean)
        noisy_d = linear_to_display(noisy)
        nl = noise_level_map(iso, (self.patch_size, self.patch_size))
        inp = np.concatenate([noisy_d, nl[..., None]], axis=-1)  # H×W×4
        inp = np.transpose(inp, (2, 0, 1)).astype(np.float32)
        tgt = np.transpose(clean_d, (2, 0, 1)).astype(np.float32)
        return {
            "input": inp,
            "target": tgt,
            "iso": iso,
            "source": item.source,
        }
