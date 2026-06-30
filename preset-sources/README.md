# Preset import sources

Drop Lightroom preset packs here (optional), then run:

```bash
npm run import-presets
```

The importer also reads these default paths automatically:

- `~/Downloads/NAKID PRESETS`
- `~/Downloads/Cuba Gallery Lightroom Presets - Pack 8`
- `~/Downloads/450+ Lightroom Presets and Photoshop Actions - [CrackzSoft]`

Output goes to `presets/bundled/` with **look-descriptive names** (author/pack branding stripped).

If Cursor cannot read Downloads (macOS privacy), copy the three folders here first:

```bash
mkdir -p preset-sources
cp -R ~/Downloads/NAKID\ PRESETS preset-sources/NAKID
cp -R ~/Downloads/Cuba\ Gallery\ Lightroom\ Presets\ -\ Pack\ 8 preset-sources/Cuba-Gallery
cp -R ~/Downloads/450+\ Lightroom\ Presets\ and\ Photoshop\ Actions\ -\ [CrackzSoft] preset-sources/CrackzSoft
npm run import-presets
```
