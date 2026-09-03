"""Export MeraSubject checkpoint to ONNX (FP32, opset 13) for tract."""

from __future__ import annotations

import argparse
from pathlib import Path

import torch

from models.merasubject import MeraSubjectNet


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--checkpoint", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--size", type=int, default=320)
    ap.add_argument("--base-channels", type=int, default=32)
    args = ap.parse_args()

    ckpt = torch.load(args.checkpoint, map_location="cpu", weights_only=False)
    base = args.base_channels
    if isinstance(ckpt.get("cfg"), dict):
        base = int(ckpt["cfg"].get("base_channels", base))
    model = MeraSubjectNet(base=base)
    model.load_state_dict(ckpt["model"])
    model.eval()

    dummy = torch.randn(1, 3, args.size, args.size)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    torch.onnx.export(
        model,
        dummy,
        str(args.output),
        input_names=["input"],
        output_names=["mask"],
        opset_version=13,
        dynamo=False,
    )
    print(f"✓ wrote {args.output}")


if __name__ == "__main__":
    main()
