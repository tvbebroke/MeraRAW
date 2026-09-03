from __future__ import annotations

import argparse
from pathlib import Path

import torch
import yaml
from torch.utils.data import DataLoader
from tqdm import tqdm

from datasets.saliency import SaliencyPairDataset
from models.merasubject import MeraSubjectNet, soft_dice_bce


def pick_device(name: str) -> torch.device:
    if name == "cpu":
        return torch.device("cpu")
    if name == "mps" and torch.backends.mps.is_available():
        return torch.device("mps")
    if name == "cuda" and torch.cuda.is_available():
        return torch.device("cuda")
    if name == "auto":
        if torch.backends.mps.is_available():
            return torch.device("mps")
        if torch.cuda.is_available():
            return torch.device("cuda")
    return torch.device("cpu")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", type=Path, required=True)
    ap.add_argument("--device", default="auto")
    args = ap.parse_args()
    cfg = yaml.safe_load(args.config.read_text())
    device = pick_device(args.device)
    print(f"device={device}")

    size = int(cfg.get("size", 320))
    ds = SaliencyPairDataset(cfg["datasets"], size=size, augment=True)
    loader = DataLoader(
        ds,
        batch_size=int(cfg.get("batch_size", 8)),
        shuffle=True,
        num_workers=int(cfg.get("workers", 2)),
        drop_last=True,
    )

    model = MeraSubjectNet(base=int(cfg.get("base_channels", 32))).to(device)
    opt = torch.optim.AdamW(model.parameters(), lr=float(cfg.get("lr", 1e-4)), weight_decay=1e-4)
    epochs = int(cfg.get("epochs", 12))
    out = Path(cfg.get("out_dir", "runs/merasubject"))
    out.mkdir(parents=True, exist_ok=True)

    best = float("inf")
    for epoch in range(1, epochs + 1):
        model.train()
        total = 0.0
        n = 0
        for x, y in tqdm(loader, desc=f"epoch {epoch}/{epochs}"):
            x, y = x.to(device), y.to(device)
            pred = model(x)
            loss = soft_dice_bce(pred, y)
            opt.zero_grad(set_to_none=True)
            loss.backward()
            opt.step()
            total += float(loss.item())
            n += 1
        avg = total / max(n, 1)
        print(f"epoch {epoch}: loss={avg:.4f}")
        ckpt = {
            "model": model.state_dict(),
            "epoch": epoch,
            "loss": avg,
            "cfg": cfg,
        }
        torch.save(ckpt, out / "last.pt")
        if avg < best:
            best = avg
            torch.save(ckpt, out / "best.pt")
            print(f"  ✓ best → {out / 'best.pt'}")


if __name__ == "__main__":
    main()
