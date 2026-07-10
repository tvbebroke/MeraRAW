#!/usr/bin/env bash
# One-way mirror of the SHAREABLE front-end source from this repo into the
# standalone ../meraraw-ui collaborator repo. See COLLABORATION.md.
#
# It syncs only the front-end source and deliberately leaves the UI repo's
# own files alone: mock/, vite.config.ts, package.json, tsconfig.json,
# README.md, .gitignore, .env* — those are UI-repo-specific.
#
# Usage:
#   bash scripts/sync-ui.sh            # apply
#   bash scripts/sync-ui.sh --dry-run  # preview changes only
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DST="${MERARAW_UI_DIR:-$SRC/../meraraw-ui}"

if [ ! -d "$DST" ]; then
  echo "error: destination not found: $DST" >&2
  echo "       set MERARAW_UI_DIR or clone meraraw-ui next to this repo." >&2
  exit 1
fi

DRY=()
[ "${1:-}" = "--dry-run" ] && DRY=(--dry-run) && echo "(dry run — no changes written)"

echo "Syncing front-end source:"
echo "  from: $SRC"
echo "    to: $DST"

# Mirror the source directories (with --delete so removals propagate). These
# are byte-identical to the UI repo's copies, so mirroring is safe.
rsync -a --delete "${DRY[@]}" "$SRC/src/"      "$DST/src/"
rsync -a --delete "${DRY[@]}" "$SRC/public/"   "$DST/public/"
rsync -a --delete "${DRY[@]}" "$SRC/assets/"   "$DST/assets/"

# keybinds: only the keymap.json is imported by the UI.
mkdir -p "$DST/keybinds"
rsync -a "${DRY[@]}" "$SRC/keybinds/keymap.json" "$DST/keybinds/keymap.json"

# index.html entry (identical in both).
rsync -a "${DRY[@]}" "$SRC/index.html" "$DST/index.html"

echo
echo "Done. Now review + commit in the UI repo:"
echo "  cd \"$DST\" && git status && git add -A && git commit -m 'sync UI from main repo'"
