#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRIVATE="$ROOT/scripts/license-private.pem"
PUBLIC="$ROOT/src-tauri/license-public.pem"

openssl ecparam -genkey -name prime256v1 -noout \
  | openssl pkcs8 -topk8 -nocrypt -out "$PRIVATE"
openssl ec -in "$PRIVATE" -pubout -out "$PUBLIC"

echo "Wrote:"
echo "  $PRIVATE  (Supabase secret: LICENSE_SIGNING_PRIVATE_KEY — do NOT commit)"
echo "  $PUBLIC   (embedded in app binary for offline verification)"
