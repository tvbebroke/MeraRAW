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

copy_if_exists "$HOME/Downloads/NAKID PRESETS" "$ROOT/preset-sources/NAKID"
copy_if_exists "$HOME/Downloads/Cuba Gallery Lightroom Presets - Pack 8" "$ROOT/preset-sources/Cuba-Gallery"
copy_if_exists "$HOME/Downloads/450+ Lightroom Presets and Photoshop Actions - [CrackzSoft]" "$ROOT/preset-sources/CrackzSoft"

cd "$ROOT"
npm run import-presets
