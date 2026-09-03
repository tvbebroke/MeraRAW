#!/usr/bin/env bash
# Install MeraNoise ONNX (if trained) and print a denoise test playbook.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TRAIN="$ROOT/denoise/train"
CKPT="$TRAIN/runs/meranoise_m4_local/best.pt"
OUT="$HOME/Library/Application Support/MeraRAW/models/denoise"
MODEL="$OUT/meranoise-v1.onnx"

echo "=== MeraRAW denoise test setup ==="
echo ""

# 1) Export + install ONNX when a checkpoint exists
if [[ -f "$CKPT" ]]; then
  echo "→ Found checkpoint: $CKPT"
  if [[ ! -d "$TRAIN/.venv" ]]; then
    echo "→ Creating Python venv…"
    python3 -m venv "$TRAIN/.venv"
    "$TRAIN/.venv/bin/pip" install -q -r "$TRAIN/requirements.txt"
  fi
  mkdir -p "$OUT"
  echo "→ Exporting meranoise-v1.onnx…"
  "$TRAIN/.venv/bin/python" "$TRAIN/export_onnx.py" \
    --checkpoint "$CKPT" \
    --output "$MODEL"
  echo "✓ Installed: $MODEL"
else
  echo "⚠ No checkpoint at $CKPT"
  echo "  Train first:  bash scripts/train-denoise-local.sh"
  echo "  Or smoke:     cd denoise/train && source .venv/bin/activate && python train.py --smoke"
  echo ""
  echo "  Without ONNX, AI Denoise uses the classical stand-in (wavelet CPU path)."
fi

echo ""
echo "=== Classical denoise bench (no photos needed) ==="
echo "  cd $ROOT/src-tauri"
echo "  cargo run -p meratech-core --example denoise_bench"
echo ""

echo "=== In-app test (MeraRAW Beta 0.1.8) ==="
echo "  1. File → Import folder (pick one below)"
echo "  2. Open a photo → Edit → Detail"
echo "  3. Classical: raise Luminance / Color under Noise Reduction"
echo "  4. AI: toggle AI Denoise ON (or Run), adjust Amount"
echo "  5. Watch Jobs panel for tile progress on large files"
echo "  6. Reset returns to plain decode (denoise_ai_reset)"
echo ""

echo "=== Recommended test folders on this Mac ==="
for dir in \
  "/Users/andrewliang/Downloads/MeraRaw/raw-test-local" \
  "/Users/andrewliang/Downloads/MeraRaw/RAW Test Photos/files" \
  "/Users/andrewliang/Downloads/MeraRaw/Photo files" \
  "/Users/andrewliang/Desktop/Desktop - AndrewM4Mac/Photography/Alaska"; do
  if [[ -d "$dir" ]]; then
    n=$(find "$dir" -type f \( -iname '*.RAF' -o -iname '*.SR2' -o -iname '*.NEF' -o -iname '*.ARW' -o -iname '*.CR2' -o -iname '*.CR3' -o -iname '*.DNG' -o -iname '*.jpg' -o -iname '*.JPG' \) 2>/dev/null | wc -l | tr -d ' ')
    echo "  ✓ $dir  ($n media files)"
  fi
done

echo ""
echo "=== Best files to start with ==="
echo "  Format smoke (any NR):"
echo "    raw-test-local/DSCF0266.RAF      Fuji X-T2"
echo "    raw-test-local/_DSC1477.SR2      Sony A900"
echo "    raw-test-local/_DSC0521.NEF      Nikon"
echo "    raw-test-local/IMG_1107.CR2      Canon"
echo "    raw-test-local/_MG_2231.CR3      Canon R"
echo ""
echo "  High-ISO / noisy (best for AI Denoise):"
echo "    Your own night/indoor shots at ISO 6400+"
echo "    Alaska folder — pick dark/indoor frames if any"
echo "    SIDD sample (register, free): https://www.eecs.yorku.ca/~kamel/sidd/dataset.php"
echo "      → import NOISY/GT pairs; compare MeraRAW vs reference in Affinity/Lightroom"
echo ""

if [[ -f "$MODEL" ]]; then
  echo "=== Model status ==="
  ls -lh "$MODEL"
  echo "  Restart MeraRAW if it was open before install."
else
  echo "=== Model status ==="
  echo "  meranoise-v1.onnx not installed — AI Denoise will say 'Using stand-in model weights'"
fi

echo ""
echo "Done."
