# Look quality / recommended presets

MeraRAW cannot legally scrape Adobe Premium presets or cracked “450+ Lightroom”
packs from the internet and ship them. Use:

1. **Bundled Recommended** — original starter looks (`npm run generate-recommended-presets`)
2. **Your licensed packs** — drop `.xmp` / `.lrtemplate` into `preset-sources/` then `npm run import-presets`
3. **This folder** — train a small scorer that prefers color-accurate, pleasant global edits

## Public free/open packs

Curated SPDX-licensed sources live in `presets/public-catalog.json`. Sync with:

```bash
npm run sync-public-presets
```

Add new packs only after checking the author’s license allows redistribution.

Goal: given a RAW/JPG + optional ColorChecker patch, rank develop presets by:
- **Fidelity** — ΔE2000 on chart patches vs reference
- **Taste** — mild priors (avoid crushing, extreme WB, neon sat)

```bash
cd looks/train
python3 -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
# Rank by taste priors on slider values:
python score_presets.py --presets-dir ../../presets/bundled
# Or after rendering each look on a chart, put 24 [R,G,B] JSONs in chart_samples/:
python score_presets.py --presets-dir ../../presets/bundled --chart-dir ./chart_samples
```

Outputs ranked JSON you can use to tag presets or drive a future “Suggest look” button.

## Phase B (later)

- Extract ColorChecker automatically (or user-click corners)
- Learn a lightweight ranker on paired before/after + chart scores
- Wire `suggest_preset` into the assistant / Presets panel
