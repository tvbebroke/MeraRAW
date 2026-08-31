#!/usr/bin/env bash
# Download the MIT-licensed U²-Net sky segmentation ONNX (xiongzhu666 / voyagerfromeast).
# ~84 MB — trained on sky masks, distinguishes sky from blue water unlike heuristics.
set -euo pipefail
cd "$(dirname "$0")/.."

DEST="src-tauri/core/models/skyseg.onnx"
URL="https://huggingface.co/voyagerfromeast/skyseg/resolve/main/skyseg_fp16.onnx"

if [[ -f "$DEST" ]]; then
  echo "✓ sky model already present: $DEST ($(du -h "$DEST" | cut -f1))"
  exit 0
fi

mkdir -p "$(dirname "$DEST")"
echo "→ Downloading sky segmentation model…"
curl -L --fail --progress-bar -o "$DEST" "$URL"
echo "✓ Saved $DEST ($(du -h "$DEST" | cut -f1))"
