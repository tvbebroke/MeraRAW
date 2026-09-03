# Bundled develop presets

Ships with the app and merges with user-saved presets in the Presets panel.

## Recommended (built-in)

Original MeraRAW starter looks:

```bash
npm run generate-recommended-presets
```

## Photographer freebies (you download)

Yes — many photographers give away free Adobe `.xmp` packs (email gates, Gumroad $0, blog freebies, etc.). Those are real and useful.

**Typical license:** free for *your* editing; **not** free for apps to re-host or ship. So MeraRAW does not scrape those sites into the repo. You download → we import.

1. Download the free pack (unzip if needed)
2. Copy the folder into `preset-sources/` (or keep it in `~/Downloads`)
3. Import:

```bash
# From folders listed in scripts/import-presets-from-downloads.sh:
bash scripts/import-presets-from-downloads.sh

# Or point at any folder of .xmp / .lrtemplate / .zip:
npm run import-presets -- "/path/to/FreePortraitPresets"
npm run tag-presets
```

Imported JSON lands in `presets/bundled/`. Rebuild the app if you want them inside the `.app` bundle.

Only add a freebie to `presets/public-catalog.json` if the author **explicitly** allows redistribution (MIT, CC0, Unlicense, “free to redistribute”, etc.).

## Public free/open packs (curated redistributable)

`presets/public-catalog.json` lists SPDX-licensed GitHub packs we *may* fetch and ship:

```bash
npm run sync-public-presets
```

Review more MIT GitHub candidates (manual add to catalog):

```bash
node scripts/fetch-public-presets.mjs --discover
```

Darktable `.dtstyle` packs are catalogued but off by default (`--include-darktable`) until a lossless mapper exists.

## Color-chart ranking

See `looks/train/`.
