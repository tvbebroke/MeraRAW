#!/usr/bin/env bash
# Train MeraSubject on this Mac (MPS), export ONNX, install for MeraRAW.
set -euo pipefail
cd "$(dirname "$0")/../segment/train"

LOG="runs/merasubject_m4_local/train.log"
mkdir -p runs/merasubject_m4_local

if [[ ! -d .venv ]]; then
  python3 -m venv .venv
  source .venv/bin/activate
  pip install -q -r requirements.txt
else
  source .venv/bin/activate
fi

if [[ ! -d data/DUTS-TR ]] && [[ ! -d data/ECSSD ]]; then
  echo "=== Download datasets $(date) ===" | tee -a "$LOG"
  python download_datasets.py --out data --duts --ecssd 2>&1 | tee -a "$LOG"
fi

echo "=== MeraSubject local train $(date) ===" | tee -a "$LOG"
python train.py --config configs/merasubject_m4_local.yaml --device auto 2>&1 | tee -a "$LOG"

echo "=== Export ONNX $(date) ===" | tee -a "$LOG"
OUT="$HOME/Library/Application Support/MeraRAW/models/subject"
mkdir -p "$OUT"
python export_onnx.py \
  --checkpoint runs/merasubject_m4_local/best.pt \
  --output "$OUT/merasubject-v1.onnx" \
  2>&1 | tee -a "$LOG"

echo "=== Done $(date) ===" | tee -a "$LOG"
echo "Model installed: $OUT/merasubject-v1.onnx"
echo "Restart MeraRAW (auto-loads that path) or set MERARAW_SUBJECT_MODEL=$OUT/merasubject-v1.onnx"
