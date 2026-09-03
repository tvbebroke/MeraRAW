"""Paired image/mask folders for saliency fine-tuning."""

from __future__ import annotations

import random
from pathlib import Path

from PIL import Image
from torch.utils.data import Dataset
import torchvision.transforms.functional as TF


IMG_EXT = {".jpg", ".jpeg", ".png", ".bmp", ".webp"}


def _list_pairs(img_dir: Path, mask_dir: Path) -> list[tuple[Path, Path]]:
    masks = {p.stem: p for p in mask_dir.rglob("*") if p.suffix.lower() in IMG_EXT}
    pairs = []
    for img in img_dir.rglob("*"):
        if img.suffix.lower() not in IMG_EXT:
            continue
        m = masks.get(img.stem)
        if m is not None:
            pairs.append((img, m))
    return pairs


class SaliencyPairDataset(Dataset):
    def __init__(self, roots: list[dict], size: int = 320, augment: bool = True):
        self.size = size
        self.augment = augment
        self.pairs: list[tuple[Path, Path]] = []
        for r in roots:
            img_d = Path(r["images"])
            mask_d = Path(r["masks"])
            if img_d.is_dir() and mask_d.is_dir():
                found = _list_pairs(img_d, mask_d)
                print(f"  {img_d}: {len(found)} pairs")
                self.pairs.extend(found)
        if not self.pairs:
            raise FileNotFoundError(
                "No image/mask pairs found. Run download_datasets.py or fix config roots."
            )

    def __len__(self) -> int:
        return len(self.pairs)

    def __getitem__(self, idx: int):
        ip, mp = self.pairs[idx]
        img = Image.open(ip).convert("RGB")
        mask = Image.open(mp).convert("L")
        img = TF.resize(img, [self.size, self.size], antialias=True)
        mask = TF.resize(mask, [self.size, self.size], interpolation=TF.InterpolationMode.NEAREST)
        if self.augment:
            if random.random() < 0.5:
                img = TF.hflip(img)
                mask = TF.hflip(mask)
            if random.random() < 0.3:
                img = TF.adjust_brightness(img, 0.85 + random.random() * 0.3)
        x = TF.to_tensor(img)
        # ImageNet-ish normalize to match runtime enc→mean/std path approximately.
        x = TF.normalize(x, [0.485, 0.456, 0.406], [0.229, 0.224, 0.225])
        y = TF.to_tensor(mask)
        y = (y > 0.5).float()
        return x, y
