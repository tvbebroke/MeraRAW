#!/usr/bin/env bash
# Build MeraRAW on macOS via Tauri, ad-hoc sign, copy to ./release-app/
# Must run on macOS (Darwin). Cloud/Linux agents cannot produce .app bundles.
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: macOS .app builds require macOS. This machine is $(uname -s)." >&2
  echo "Run this script on your Mac (local Cursor terminal or Terminal.app)." >&2
  exit 1
fi

cd "$(dirname "$0")/.."

VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
PRODUCT="$(node -p "require('./src-tauri/tauri.conf.json').productName")"
OUT_DIR="release-app"
BUNDLE_DIR="src-tauri/target/release/bundle/macos"

echo "→ Installing frontend dependencies…"
if [[ -f package-lock.json ]]; then npm ci; else npm install; fi

if [[ -z "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  export APPLE_SIGNING_IDENTITY="-"
  echo "→ Using ad-hoc signing (APPLE_SIGNING_IDENTITY=-)"
fi

echo "→ Tauri production build (macOS .app bundle)…"
npm run tauri build -- --bundles app

APP_SRC="$(find "${BUNDLE_DIR}" -maxdepth 1 -name '*.app' -type d 2>/dev/null | head -1)"
if [[ -z "${APP_SRC}" ]]; then
  echo "error: no .app under ${BUNDLE_DIR}" >&2
  exit 1
fi

DEST="${OUT_DIR}/${PRODUCT} ${VERSION}.app"
mkdir -p "${OUT_DIR}"
rm -rf "${DEST}"
cp -R "${APP_SRC}" "${DEST}"

echo "→ Ad-hoc codesign…"
codesign --force --deep --sign - "${DEST}"

echo ""
echo "✓ Built and copied:"
echo "  ${DEST}"
echo "  (source bundle: ${APP_SRC})"
ls -la "${OUT_DIR}"
