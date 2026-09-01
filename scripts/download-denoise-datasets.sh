#!/usr/bin/env bash
# Download / stage public denoise datasets for MeraNoise training.
# Many datasets require manual registration — this script fetches what is automatable
# and prints instructions for the rest.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DATA="${MERANOISE_DATA_ROOT:-$ROOT/denoise/train/data}"
mkdir -p "$DATA"

echo "→ MeraNoise data root: $DATA"
echo ""

# DIV2K train HR (clean, for synthetic noise) — direct zip from ETH
DIV2K_ZIP="$DATA/div2k/DIV2K_train_HR.zip"
if [[ ! -d "$DATA/div2k/DIV2K_train_HR" ]]; then
  echo "→ Downloading DIV2K train HR (~3.2 GB)…"
  mkdir -p "$DATA/div2k"
  if command -v curl >/dev/null; then
    curl -L --fail -o "$DIV2K_ZIP" \
      "https://data.vision.ee.ethz.ch/cvl/DIV2K/DIV2K_train_HR.zip"
    unzip -q "$DIV2K_ZIP" -d "$DATA/div2k"
    rm -f "$DIV2K_ZIP"
  else
    echo "  curl not found — download manually from https://data.vision.ee.ethz.ch/cvl/DIV2K/"
  fi
else
  echo "✓ DIV2K train HR present"
fi

# NIND via nind-denoise sample (if repo provides links)
NIND_DIR="$DATA/nind"
if [[ ! -d "$NIND_DIR" ]] || [[ -z "$(ls -A "$NIND_DIR" 2>/dev/null || true)" ]]; then
  echo ""
  echo "NIND (Natural Image Noise Dataset):"
  echo "  1. Clone https://github.com/m-tassano/nind-denoise"
  echo "  2. Follow their README to obtain NIND image pairs"
  echo "  3. Place clean/noisy pairs under: $NIND_DIR/clean and $NIND_DIR/noisy"
  echo "     (or flat paired *_clean.jpg / *_noisy.jpg)"
fi

echo ""
echo "SIDD:"
echo "  Register at https://www.eecs.yorku.ca/~kamel/sidd/dataset.php"
echo "  Extract sRGB pairs to: $DATA/sidd/SIDD_Small_sRGB_Only/{GT,NOISY}/"

echo ""
echo "SID (See-in-the-Dark RAW pairs):"
echo "  Follow https://github.com/cchen156/Learning-to-See-in-the-Dark"
echo "  Place under: $DATA/sid/Sony/{short,long} and $DATA/sid/Fuji/{short,long}/"

echo ""
echo "DND / PolyU / FiveK — see denoise/train/datasets/registry.py for URLs."
echo ""
echo "When ready:"
echo "  cd denoise/train && pip install -r requirements.txt"
echo "  python train.py --config configs/meranoise_v1.yaml"
echo "  python export_onnx.py --checkpoint runs/meranoise_v1/best.pt --output meranoise-v1.onnx --fp16"
