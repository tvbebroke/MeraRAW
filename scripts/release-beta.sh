#!/usr/bin/env bash
# Build, sign, notarize, and optionally upload the MeraRAW Beta DMG.
#
# Required for notarization (export before running; one line each, no trailing comments):
#   export APPLE_ID="you@example.com"
#   export APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"
#   export APPLE_TEAM_ID="ZJP5CXC3FS"
# APPLE_PASSWORD is an app-specific password from appleid.apple.com — not your Apple ID password.
#
# Optional R2 upload (private bucket):
#   export R2_ENDPOINT="https://<account_id>.r2.cloudflarestorage.com"
#   export R2_ACCESS_KEY_ID="..."
#   export R2_SECRET_ACCESS_KEY="..."
#   export R2_BUCKET_NAME="meraraw-releases"
#   export R2_OBJECT_KEY="MeraRAW Beta 0.1.0.dmg"
#
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="0.1.1"
DMG_NAME="MeraRAW Beta ${VERSION}.dmg"
BUNDLE_DIR="src-tauri/target/release/bundle/dmg"
RELEASE_DIR="release"

echo "→ Building signed DMG (notarizes when APPLE_* env vars are set)…"
npm run tauri build

BUILT=""
for root in "src-tauri/target" "${CARGO_TARGET_DIR:-}"; do
  [[ -z "${root}" || ! -d "${root}" ]] && continue
  BUILT="$(find "${root}" -path '*/bundle/dmg/*.dmg' -type f 2>/dev/null | head -1)"
  [[ -n "${BUILT}" ]] && break
done
if [[ -z "${BUILT}" ]]; then
  echo "error: no DMG found under src-tauri/target (or CARGO_TARGET_DIR)" >&2
  exit 1
fi

mkdir -p "${RELEASE_DIR}"
cp "${BUILT}" "${RELEASE_DIR}/${DMG_NAME}"
echo "→ Release artifact: ${RELEASE_DIR}/${DMG_NAME}"
ls -lh "${RELEASE_DIR}/${DMG_NAME}"

if [[ -n "${R2_ENDPOINT:-}" && -n "${R2_ACCESS_KEY_ID:-}" && -n "${R2_SECRET_ACCESS_KEY:-}" && -n "${R2_BUCKET_NAME:-}" ]]; then
  KEY="${R2_OBJECT_KEY:-${DMG_NAME}}"
  echo "→ Uploading to R2: s3://${R2_BUCKET_NAME}/${KEY}"
  AWS_ACCESS_KEY_ID="${R2_ACCESS_KEY_ID}" \
  AWS_SECRET_ACCESS_KEY="${R2_SECRET_ACCESS_KEY}" \
  aws s3 cp "${RELEASE_DIR}/${DMG_NAME}" "s3://${R2_BUCKET_NAME}/${KEY}" \
    --endpoint-url "${R2_ENDPOINT}"
  echo "→ Uploaded. Set Supabase secret R2_OBJECT_KEY=${KEY}"
else
  echo "→ Skipping R2 upload (R2_* env vars not set)."
  echo "  Upload manually, then set Supabase R2_OBJECT_KEY to: ${DMG_NAME}"
fi
