#!/usr/bin/env bash
# Build, sign, notarize, and optionally upload the MeraRAW Beta DMG.
#
# Required for notarization (export before running; one line each, no trailing comments):
#   export APPLE_ID="you@example.com"
#   export APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"
#   export APPLE_TEAM_ID="ZJP5CXC3FS"
# APPLE_PASSWORD is an app-specific password from appleid.apple.com — not your Apple ID password.
#
# Or store credentials once:
#   xcrun notarytool store-credentials "meraraw-notary"
#   export NOTARY_KEYCHAIN_PROFILE="meraraw-notary"
#
# Optional R2 upload (private bucket):
#   export R2_ENDPOINT="https://<account_id>.r2.cloudflarestorage.com"
#   export R2_ACCESS_KEY_ID="..."
#   export R2_SECRET_ACCESS_KEY="..."
#   export R2_BUCKET_NAME="meraraw-releases"
#   export R2_OBJECT_KEY="MeraRAW Beta 0.1.1.dmg"
#
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ -f .env ]]; then
  set -a
  # shellcheck disable=SC1091
  source .env
  set +a
fi

VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
DMG_NAME="MeraRAW Beta ${VERSION}.dmg"
RELEASE_DIR="release"

have_notary_creds=false
if [[ -n "${NOTARY_KEYCHAIN_PROFILE:-}" ]]; then
  have_notary_creds=true
elif [[ -n "${APPLE_API_KEY:-}" && -n "${APPLE_API_ISSUER:-}" && -n "${APPLE_API_KEY_PATH:-}" ]]; then
  have_notary_creds=true
elif [[ -n "${APPLE_ID:-}" && -n "${APPLE_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" ]]; then
  have_notary_creds=true
fi

if [[ "${NOTARIZE_ONLY:-}" == "1" ]]; then
  DMG_PATH="${RELEASE_DIR}/${DMG_NAME}"
  if [[ ! -f "${DMG_PATH}" ]]; then
    echo "error: ${DMG_PATH} not found — run a full release build first." >&2
    exit 1
  fi
  if [[ "${have_notary_creds}" != true ]]; then
    echo "error: notarization credentials required for NOTARIZE_ONLY=1" >&2
    exit 1
  fi
  bash scripts/notarize-dmg.sh "${DMG_PATH}"
  exit 0
fi

if [[ "${have_notary_creds}" != true ]]; then
  echo "⚠ Notarization credentials not set — build will sign but skip notarization."
  echo "  After build: NOTARIZE_ONLY=1 npm run notarize  (or export APPLE_* and re-run release)"
else
  echo "→ Apple notarization credentials detected."
fi

echo "→ Building MeraRAW Beta ${VERSION} (signed DMG)…"
npm run build
# Re-link frontend into the Tauri binary when dist is newer than the last release build.
RELEASE_BIN="src-tauri/target/release/meratech-editor"
if [[ -f "${RELEASE_BIN}" && dist/index.html -nt "${RELEASE_BIN}" ]]; then
  echo "→ dist/ newer than release binary — forcing Tauri relink"
  rm -f "${RELEASE_BIN}"
fi
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

if [[ "${have_notary_creds}" == true ]]; then
  bash scripts/notarize-dmg.sh "${RELEASE_DIR}/${DMG_NAME}"
else
  echo "→ DMG is signed but not notarized. Run: NOTARIZE_ONLY=1 npm run notarize"
fi

if [[ -n "${R2_ENDPOINT:-}" && -n "${R2_ACCESS_KEY_ID:-}" && -n "${R2_SECRET_ACCESS_KEY:-}" && -n "${R2_BUCKET_NAME:-}" ]]; then
  KEY="${R2_OBJECT_KEY:-${DMG_NAME}}"
  ENDPOINT="${R2_ENDPOINT%/}"
  if [[ "${ENDPOINT}" == */"${R2_BUCKET_NAME}" ]]; then
    ENDPOINT="${ENDPOINT%/"${R2_BUCKET_NAME}"}"
  fi
  echo "→ Uploading to R2: s3://${R2_BUCKET_NAME}/${KEY}"
  if command -v aws >/dev/null 2>&1; then
    AWS_ACCESS_KEY_ID="${R2_ACCESS_KEY_ID}" \
    AWS_SECRET_ACCESS_KEY="${R2_SECRET_ACCESS_KEY}" \
    aws s3 cp "${RELEASE_DIR}/${DMG_NAME}" "s3://${R2_BUCKET_NAME}/${KEY}" \
      --endpoint-url "${ENDPOINT}"
  elif [[ -x ".venv-r2/bin/python" && -f scripts/r2-upload.py ]]; then
    R2_ENDPOINT="${ENDPOINT}" .venv-r2/bin/python scripts/r2-upload.py "${RELEASE_DIR}/${DMG_NAME}" "${KEY}"
  else
    echo "error: aws CLI unavailable — run: python3 -m venv .venv-r2 && .venv-r2/bin/pip install boto3" >&2
    exit 1
  fi
  echo "→ Uploaded. Set Supabase secret R2_OBJECT_KEY=${KEY}"
else
  echo "→ Skipping R2 upload (R2_* env vars not set)."
  echo "  Upload manually, then set Supabase R2_OBJECT_KEY to: ${DMG_NAME}"
fi

echo "→ Done. Tag: v${VERSION}"
