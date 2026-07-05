#!/usr/bin/env bash
# Notarize and staple an existing signed DMG (no rebuild).
#
# Option A — Apple ID (app-specific password from appleid.apple.com):
#   export APPLE_ID="you@example.com"
#   export APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"
#   export APPLE_TEAM_ID="ZJP5CXC3FS"
#
# Option B — App Store Connect API key:
#   export APPLE_API_KEY="..."
#   export APPLE_API_ISSUER="..."
#   export APPLE_API_KEY_PATH="/path/to/AuthKey_XXXX.p8"
#
# Option C — Keychain profile from: xcrun notarytool store-credentials "meraraw-notary"
#   export NOTARY_KEYCHAIN_PROFILE="meraraw-notary"
#
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ -f .env ]]; then
  set -a
  # shellcheck disable=SC1091
  source .env
  set +a
fi

DMG="${1:-}"
if [[ -z "${DMG}" ]]; then
  VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
  DMG="release/MeraRAW Beta ${VERSION}.dmg"
fi

if [[ ! -f "${DMG}" ]]; then
  echo "error: DMG not found: ${DMG}" >&2
  exit 1
fi

SUBMIT_ARGS=()
if [[ -n "${APPLE_API_KEY:-}" && -n "${APPLE_API_ISSUER:-}" && -n "${APPLE_API_KEY_PATH:-}" ]]; then
  SUBMIT_ARGS=(--key "${APPLE_API_KEY_PATH}" --key-id "${APPLE_API_KEY}" --issuer "${APPLE_API_ISSUER}")
elif [[ -n "${APPLE_ID:-}" && -n "${APPLE_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" ]]; then
  SUBMIT_ARGS=(--apple-id "${APPLE_ID}" --password "${APPLE_PASSWORD}" --team-id "${APPLE_TEAM_ID}")
elif [[ -n "${NOTARY_KEYCHAIN_PROFILE:-}" ]]; then
  SUBMIT_ARGS=(--keychain-profile "${NOTARY_KEYCHAIN_PROFILE}")
else
  echo "error: notarization credentials not found." >&2
  echo "" >&2
  echo "Add ONE of these to: $(pwd)/.env" >&2
  echo "" >&2
  echo "  APPLE_ID=you@example.com" >&2
  echo "  APPLE_PASSWORD=xxxx-xxxx-xxxx-xxxx" >&2
  echo "  APPLE_TEAM_ID=ZJP5CXC3FS" >&2
  echo "" >&2
  echo "Or store a keychain profile:" >&2
  echo "  xcrun notarytool store-credentials \"meraraw-notary\"" >&2
  echo "  NOTARY_KEYCHAIN_PROFILE=meraraw-notary" >&2
  if [[ -f .env ]]; then
    echo "" >&2
    echo "Current .env keys (values hidden):" >&2
    awk -F= '/^[^#[:space:]]/ {print "  - "$1}' .env >&2
  else
    echo "" >&2
    echo "No .env file found — copy .env.example and add Apple vars." >&2
  fi
  exit 1
fi

echo "→ Submitting for notarization: ${DMG}"
xcrun notarytool submit "${DMG}" "${SUBMIT_ARGS[@]}" --wait

echo "→ Stapling notarization ticket…"
xcrun stapler staple "${DMG}"

echo "→ Gatekeeper check…"
if spctl -a -vv -t install "${DMG}" 2>&1; then
  echo "→ Notarization verified."
else
  echo "⚠ spctl reported an issue — check notarytool log if users see Gatekeeper blocks." >&2
fi
