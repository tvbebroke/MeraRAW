#!/usr/bin/env bash
# Install the repo pre-commit gate. Run once after clone:
#   bash scripts/install-hooks.sh
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
hook="$root/.git/hooks/pre-commit"
mkdir -p "$root/.git/hooks"
cp "$root/scripts/pre-commit" "$hook"
chmod +x "$hook" "$root/scripts/pre-commit"
echo "installed $hook"
