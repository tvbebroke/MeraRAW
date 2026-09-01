#!/usr/bin/env bash
# Train MeraNoise on this Mac (MPS), export ONNX, install for MeraRAW.
set -euo pipefail
cd "$(dirname "$0")/../denoise/train"

LOG="runs/meranoise_m4_local/train.log"
mkdir -p runs/meranoise_m4_local

if [[ ! -d .venv ]]; then
  python3 -m venv .venv
  source .venv/bin/activate
  pip install -q -r requirements.txt
else
  source .venv/bin/activate
fi

echo "=== MeraNoise local train $(date) ===" | tee -a "$LOG"
python train.py --config configs/meranoise_m4_local.yaml --device auto 2>&1 | tee -a "$LOG"

echo "=== Export ONNX $(date) ===" | tee -a "$LOG"
OUT="$HOME/Library/Application Support/MeraRAW/models/denoise"
mkdir -p "$OUT"
python export_onnx.py \
  --checkpoint runs/meranoise_m4_local/best.pt \
  --output "$OUT/meranoise-v1.onnx" \
  --fp16 2>&1 | tee -a "$LOG"

echo "=== Done $(date) ===" | tee -a "$LOG"
echo "Model installed: $OUT/meranoise-v1.onnx"
echo "Set MERARAW_DENOISE_ALLOW_UNPINNED=1 or restart MeraRAW after export."
