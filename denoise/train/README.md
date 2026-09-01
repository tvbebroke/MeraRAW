# MeraNoise Training Pipeline

Train **MeraNoise v1** — an ISO-conditioned residual UNet aimed at Lightroom-class AI denoise on linear RGB (post-demosaic), using every major **public** paired-noise dataset we can legally combine.

## Goal

| Tier | Target | Status |
|------|--------|--------|
| **A** | RGB UNet, multi-dataset, ONNX in app | This pipeline |
| **B** | Noise-map conditioning (ISO → 4th channel) | Built into v1 |
| **C** | Joint Bayer demosaic+denoise (Lightroom parity) | Future — needs RAW mosaic training |

Lightroom’s edge is **mosaic-domain joint demosaic+denoise** on proprietary data. We close the gap in two steps: (1) ship a strong RGB model trained on all public real-noise pairs + synthetic RAW-linear noise; (2) Phase C mosaic model on SID + synthetic Bayer.

## Quick start

```bash
# 1. Stage datasets (DIV2K auto; SIDD/NIND/SID manual — see script output)
bash scripts/download-denoise-datasets.sh

# 2. Python env
cd denoise/train
python3 -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt

# 3. Smoke test (synthetic patches only, ~2 min CPU)
python train.py --smoke

# 4. Full train (GPU recommended, multi-day for convergence)
export MERANOISE_DATA_ROOT=/path/to/data   # default: denoise/train/data
python train.py --config configs/meranoise_v1.yaml

# 5. Export ONNX (576² — matches src-tauri/core/src/denoise/ai/tiler.rs)
python export_onnx.py --checkpoint runs/meranoise_v1/best.pt \
  --output meranoise-v1.onnx --fp16

# 6. Install in MeraRAW
mkdir -p ~/Library/Application\ Support/MeraRAW/models/denoise
cp meranoise-v1.onnx ~/Library/Application\ Support/MeraRAW/models/denoise/
# Or: export MERARAW_DENOISE_MODELS_DIR=.../denoise
```

## Datasets (public)

| ID | Type | Role |
|----|------|------|
| **NIND** | Real low/high ISO pairs | Core real sensor noise |
| **SIDD** | Smartphone real pairs | Cross-device generalization |
| **SID** | Sony/Fuji RAW short/long | High-ISO RAW texture |
| **DND** | Benchmark real noise | Val / regression |
| **PolyU** | Real-world noisy | Extra real noise diversity |
| **DIV2K / Flickr2K** | Clean | Poisson–Gaussian synthesis (ISO 100–25600) |

Registry + licenses: `datasets/registry.py`.

## Model

- **Architecture:** 4-level UNet, 48 base channels, residual on RGB
- **Input:** display-gamma RGB (γ=2.2) + normalized log-ISO map (matches `run_onnx` encode path)
- **Output:** display-gamma clean RGB
- **Loss:** Charbonnier + SSIM + edge (texture preservation)
- **Export:** fixed 576×576 NCHW fp16 ONNX for tract

## Beating Lightroom (realistic plan)

1. **Train v1** on NIND+SIDD+SID+DIV2K (this repo) — expect strong results ISO 1600–12800 on RGB.
2. **Benchmark** against classical + nind-UNet using `cargo run -p meratech-core --example denoise_bench` and blind A/B on SID validation crops.
3. **Phase C:** Bayer-packed 4-plane input, train on SID RAW + synthetic mosaic noise (requires `run_onnx` mosaic path — see `denoise/05-ai-denoising.md`).
4. **Runtime:** migrate tract → ONNX Runtime + CoreML EP for ANE speed (spec’d, not yet wired).

## Compute

| Hardware | ~Time (100 epochs, full data) |
|----------|-------------------------------|
| 1× RTX 4090 | 2–4 days |
| 1× M2 Max | 5–8 days |
| CPU only | Not recommended |

Use `--smoke` to verify the pipeline before committing GPU time.
