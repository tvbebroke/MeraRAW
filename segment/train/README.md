# MeraSubject — train better subject masks

MeraRAW’s live subject tool uses bundled **u2netp** (saliency) plus local refine.
That model is small and often fails on wildlife / low-contrast scenes (birds on dark water).

This folder trains a stronger subject/saliency network on public datasets, then exports ONNX for the app.

## Datasets (public)

| Dataset | What | License notes |
|---------|------|----------------|
| [DUTS](http://saliencydetection.net/duts/) | 10k train / 5k test saliency | Research use — check site |
| [ECSSD](https://www.cse.cuhk.edu.hk/leojia/projects/hsaliency/dataset.html) | 1k high-quality | Research |
| [DIS5K](https://xuebinqin.github.io/dis/index.html) | High-res dichotomous masks | Apache-2.0 weights / check data |

Prefer **DIS5K** + **DUTS** for Lightroom-class silhouettes (thin structures, animals).

## Quick start (Apple Silicon)

```bash
# From repo root
./scripts/train-subject-local.sh
```

Or manually:

```bash
cd segment/train
python3 -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
python download_datasets.py --out data --duts --ecssd
python train.py --config configs/merasubject_m4_local.yaml --device mps
python export_onnx.py --checkpoint runs/merasubject_m4_local/best.pt \
  --output "$HOME/Library/Application Support/MeraRAW/models/subject/merasubject-v1.onnx"
```

Then point the engine at the new weights (env override, same pattern as denoise):

```bash
export MERARAW_SUBJECT_MODEL="$HOME/Library/Application Support/MeraRAW/models/subject/merasubject-v1.onnx"
```

## Scope

- **Phase A (this scaffold):** fine-tune a compact U²-Net-style head on DUTS/ECSSD; export ONNX.
- **Phase B:** DIS5K + wildlife fine-tune (your Alaska birds) for beak/wing edges.
- **Phase C (optional):** click-conditioned SAM 2 / HQ-SAM for outline pick — separate runtime.

Local M4 is fine for experiments. Matching Lightroom Subject needs Phase B/C and usually cloud GPU.
