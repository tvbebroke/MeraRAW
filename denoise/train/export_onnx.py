#!/usr/bin/env python3
"""Export MeraNoise checkpoint → ONNX for tract runtime (576², NCHW)."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import torch
import yaml

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from models.meranoise import build_model


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--checkpoint", required=True, help="Path to best.pt or epoch_*.pt")
    ap.add_argument("--output", default="meranoise-v1.onnx")
    ap.add_argument("--tile-size", type=int, default=576)
    ap.add_argument("--opset", type=int, default=17)
    ap.add_argument("--fp16", action="store_true")
    args = ap.parse_args()

    ckpt = torch.load(args.checkpoint, map_location="cpu", weights_only=False)
    cfg = ckpt["cfg"]
    model = build_model(cfg)
    model.load_state_dict(ckpt["model"])
    model.eval()

    in_ch = int(cfg.get("model", {}).get("in_channels", 4))
    dummy = torch.randn(1, in_ch, args.tile_size, args.tile_size)
    if args.fp16:
        model = model.half()
        dummy = dummy.half()

    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    torch.onnx.export(
        model,
        dummy,
        str(out_path),
        input_names=["input"],
        output_names=["output"],
        opset_version=args.opset,
        dynamic_axes=None,
        do_constant_folding=True,
    )
    print(f"Exported {out_path} ({out_path.stat().st_size / 1e6:.1f} MB)")
    print("Install: cp", out_path, "~/Library/Application\\ Support/MeraRAW/models/denoise/")
    print("Or set MERARAW_DENOISE_MODELS_DIR to the containing folder.")


if __name__ == "__main__":
    main()
