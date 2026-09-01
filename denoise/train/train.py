#!/usr/bin/env python3
"""Train MeraNoise v1 on composite public datasets."""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

import torch
import torch.nn.functional as F
import yaml
from torch.utils.data import DataLoader
from tqdm import tqdm

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from datasets.composite import MeraNoiseDataset
from models.meranoise import build_model
from utils.metrics import psnr, ssim_gray


def charbonnier(x: torch.Tensor, y: torch.Tensor, eps: float = 1e-3) -> torch.Tensor:
    return torch.sqrt((x - y) ** 2 + eps * eps).mean()


def ssim_loss(pred: torch.Tensor, target: torch.Tensor) -> torch.Tensor:
    # Lightweight multi-scale proxy on luminance.
    p = 0.299 * pred[:, 0] + 0.587 * pred[:, 1] + 0.114 * pred[:, 2]
    t = 0.299 * target[:, 0] + 0.587 * target[:, 1] + 0.114 * target[:, 2]
    losses = []
    for scale in (1, 2):
        if scale > 1:
            p_s = F.avg_pool2d(p.unsqueeze(1), scale).squeeze(1)
            t_s = F.avg_pool2d(t.unsqueeze(1), scale).squeeze(1)
        else:
            p_s, t_s = p, t
        c1, c2 = 0.01**2, 0.03**2
        mu_p, mu_t = p_s.mean(dim=(-2, -1)), t_s.mean(dim=(-2, -1))
        var_p = ((p_s - mu_p[:, None, None]) ** 2).mean(dim=(-2, -1))
        var_t = ((t_s - mu_t[:, None, None]) ** 2).mean(dim=(-2, -1))
        cov = ((p_s - mu_p[:, None, None]) * (t_s - mu_t[:, None, None])).mean(dim=(-2, -1))
        ssim = ((2 * mu_p * mu_t + c1) * (2 * cov + c2)) / (
            (mu_p**2 + mu_t**2 + c1) * (var_p + var_t + c2) + 1e-8
        )
        losses.append(1.0 - ssim.mean())
    return sum(losses) / len(losses)


def edge_loss(pred: torch.Tensor, target: torch.Tensor) -> torch.Tensor:
    kx = torch.tensor([[-1, 0, 1], [-2, 0, 2], [-1, 0, 1]], device=pred.device, dtype=pred.dtype)
    kx = kx.view(1, 1, 3, 3)
    ky = kx.transpose(-1, -2)
    p = pred.mean(dim=1, keepdim=True)
    t = target.mean(dim=1, keepdim=True)
    gx_p, gy_p = F.conv2d(p, kx, padding=1), F.conv2d(p, ky, padding=1)
    gx_t, gy_t = F.conv2d(t, kx, padding=1), F.conv2d(t, ky, padding=1)
    return F.l1_loss(torch.sqrt(gx_p**2 + gy_p**2 + 1e-6), torch.sqrt(gx_t**2 + gy_t**2 + 1e-6))


@torch.no_grad()
def validate(model: torch.nn.Module, loader: DataLoader, device: torch.device) -> dict[str, float]:
    model.eval()
    psnrs, ssims = [], []
    for batch in loader:
        x = batch["input"].to(device)
        y = batch["target"].to(device)
        pred = model(x).clamp(0.0, 1.0)
        for i in range(pred.shape[0]):
            p = pred[i].cpu().numpy().transpose(1, 2, 0)
            t = y[i].cpu().numpy().transpose(1, 2, 0)
            psnrs.append(psnr(t, p))
            lum_p = 0.299 * p[..., 0] + 0.587 * p[..., 1] + 0.114 * p[..., 2]
            lum_t = 0.299 * t[..., 0] + 0.587 * t[..., 1] + 0.114 * t[..., 2]
            ssims.append(ssim_gray(lum_t, lum_p))
    return {
        "psnr": float(sum(psnrs) / max(len(psnrs), 1)),
        "ssim": float(sum(ssims) / max(len(ssims), 1)),
    }


def pick_device(requested: str) -> torch.device:
    if requested != "auto":
        return torch.device(requested)
    if torch.cuda.is_available():
        return torch.device("cuda")
    if getattr(torch.backends, "mps", None) and torch.backends.mps.is_available():
        return torch.device("mps")
    return torch.device("cpu")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", default="configs/meranoise_v1.yaml")
    ap.add_argument("--epochs", type=int, default=None)
    ap.add_argument("--device", default="auto")
    ap.add_argument("--smoke", action="store_true", help="Tiny synthetic-only smoke run")
    args = ap.parse_args()

    cfg_path = ROOT / args.config
    with open(cfg_path) as f:
        cfg = yaml.safe_load(f)

    if args.smoke:
        os.environ.setdefault("MERANOISE_DATA_ROOT", str(ROOT / "data_smoke"))
        smoke_dir = Path(os.environ["MERANOISE_DATA_ROOT"]) / "div2k" / "DIV2K_train_HR"
        smoke_dir.mkdir(parents=True, exist_ok=True)
        from PIL import Image

        for i in range(4):
            arr = (torch.rand(512, 512, 3).numpy() * 255).astype("uint8")
            Image.fromarray(arr).save(smoke_dir / f"smoke_{i:02d}.png")
        cfg["data"]["datasets"] = [{"name": "synthetic_div2k", "weight": 1.0, "split": "train"}]
        cfg["data"]["val_datasets"] = [{"name": "synthetic_div2k", "split": "train"}]
        cfg["train"]["epochs"] = 2
        cfg["data"]["batch_size"] = 2

    device = pick_device(args.device)
    print(f"device: {device}")
    patch = int(cfg["data"]["patch_size"])
    iso_range = tuple(cfg["noise"]["iso_range"])
    virtual = int(cfg["data"].get("virtual_samples", 0))

    train_ds = MeraNoiseDataset(
        cfg["data"]["datasets"], "train", patch, iso_range, virtual_samples=virtual
    )
    val_specs = cfg["data"].get("val_datasets", cfg["data"]["datasets"])
    val_ds = MeraNoiseDataset(val_specs, "val", patch, iso_range, virtual_samples=virtual)

    train_loader = DataLoader(
        train_ds,
        batch_size=int(cfg["data"]["batch_size"]),
        shuffle=True,
        num_workers=int(cfg["data"].get("num_workers", 0)),
        pin_memory=device.type == "cuda",
    )
    val_loader = DataLoader(val_ds, batch_size=2, shuffle=False, num_workers=0)

    model = build_model(cfg).to(device)
    opt = torch.optim.AdamW(
        model.parameters(),
        lr=float(cfg["train"]["lr"]),
        weight_decay=float(cfg["train"]["weight_decay"]),
    )
    use_amp = bool(cfg["train"].get("amp", False)) and device.type == "cuda"
    scaler = torch.cuda.amp.GradScaler(enabled=use_amp)

    out_dir = ROOT / cfg["train"]["out_dir"]
    out_dir.mkdir(parents=True, exist_ok=True)
    epochs = args.epochs or int(cfg["train"]["epochs"])
    lw = cfg["loss"]

    best_psnr = 0.0
    for epoch in range(1, epochs + 1):
        model.train()
        losses = []
        for batch in tqdm(train_loader, desc=f"epoch {epoch}/{epochs}"):
            x = batch["input"].to(device)
            y = batch["target"].to(device)
            opt.zero_grad(set_to_none=True)
            with torch.cuda.amp.autocast(enabled=use_amp):
                pred = model(x)
                loss = (
                    float(lw["charbonnier"]) * charbonnier(pred, y)
                    + float(lw["ssim"]) * ssim_loss(pred, y)
                    + float(lw["edge"]) * edge_loss(pred, y)
                )
            if use_amp:
                scaler.scale(loss).backward()
                if cfg["train"].get("grad_clip"):
                    scaler.unscale_(opt)
                    torch.nn.utils.clip_grad_norm_(model.parameters(), float(cfg["train"]["grad_clip"]))
                scaler.step(opt)
                scaler.update()
            else:
                loss.backward()
                if cfg["train"].get("grad_clip"):
                    torch.nn.utils.clip_grad_norm_(model.parameters(), float(cfg["train"]["grad_clip"]))
                opt.step()
            losses.append(float(loss.detach().cpu()))

        metrics = validate(model, val_loader, device)
        avg_loss = sum(losses) / max(len(losses), 1)
        print(
            f"epoch {epoch}: loss={avg_loss:.4f} val_psnr={metrics['psnr']:.2f} "
            f"val_ssim={metrics['ssim']:.4f}"
        )

        ckpt = out_dir / f"epoch_{epoch:03d}.pt"
        torch.save({"epoch": epoch, "model": model.state_dict(), "cfg": cfg, "metrics": metrics}, ckpt)
        torch.save({"epoch": epoch, "model": model.state_dict(), "cfg": cfg, "metrics": metrics}, out_dir / "last.pt")
        # Keep disk usage low — drop older numbered checkpoints.
        save_every = int(cfg["train"].get("save_every", 10))
        if epoch % save_every != 0 and ckpt.exists():
            ckpt.unlink(missing_ok=True)
        elif epoch > save_every:
            old = out_dir / f"epoch_{epoch - save_every:03d}.pt"
            old.unlink(missing_ok=True)
        if metrics["psnr"] > best_psnr:
            best_psnr = metrics["psnr"]
            torch.save({"epoch": epoch, "model": model.state_dict(), "cfg": cfg, "metrics": metrics}, out_dir / "best.pt")

    print(f"Training complete. Best val PSNR: {best_psnr:.2f} dB. Checkpoints in {out_dir}")


if __name__ == "__main__":
    main()
