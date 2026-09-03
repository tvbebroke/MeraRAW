#!/bin/bash
# Run this once in Terminal (has access to ~/Downloads) to import preset packs.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$ROOT/preset-sources"

copy_if_exists() {
  local src="$1" dest="$2"
  if [[ -d "$src" ]]; then
    echo "Copying $(basename "$src")…"
    rm -rf "$dest"
    cp -R "$src" "$dest"
  else
    echo "Skip (not found): $src"
  fi
}

# Only copy packs you already downloaded (photographer freebies / packs you own).
# Add more copy_if_exists lines for free packs you grab from blogs / Gumroad $0 / etc.
copy_if_exists "$HOME/Downloads/NAKID PRESETS" "$ROOT/preset-sources/NAKID"
copy_if_exists "$HOME/Downloads/Cuba Gallery Lightroom Presets - Pack 8" "$ROOT/preset-sources/Cuba-Gallery"

# Auto-pick common freebie zip extracts under Downloads (*preset* / *xmp* dirs)
shopt -s nullglob
for d in "$HOME/Downloads"/*[Pp]reset* "$HOME/Downloads"/*[Xx][Mm][Pp]*; do
  [[ -d "$d" ]] || continue
  base="$(basename "$d")"
  # skip known cracked dump names
  if [[ "$base" == *CrackzSoft* ]]; then continue; fi
  dest="$ROOT/preset-sources/${base}"
  copy_if_exists "$d" "$dest"
done
shopt -u nullglob

cd "$ROOT"
npm run generate-recommended-presets
# Prefer public redistributable + your Downloads copies
npm run import-presets
npm run tag-presets
