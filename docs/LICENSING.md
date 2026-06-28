# MeraRAW licensing setup

Two independent locks: **gated download** (website) and **in-app activation** (DMG).

## 1. Cloudflare R2 (private DMG storage)

1. Create bucket `meraraw-releases` — **keep private**
2. Upload `MeraRAW_0.1.0_aarch64.dmg` (or your release filename)
3. Create R2 API token scoped to the bucket

## 2. Supabase Edge Function secrets

| Secret | Purpose |
|---|---|
| `R2_ENDPOINT` | `https://<account_id>.r2.cloudflarestorage.com` |
| `R2_ACCESS_KEY_ID` | R2 API token |
| `R2_SECRET_ACCESS_KEY` | R2 API token |
| `R2_BUCKET_NAME` | e.g. `meraraw-releases` |
| `R2_OBJECT_KEY` | e.g. `MeraRAW Beta 0.1.0.dmg` |
| `R2_SIGNED_URL_TTL` | Optional, default `300` (5 min) |
| `LICENSE_SIGNING_PRIVATE_KEY` | ES256 private key PEM (see below) |

Deploy Edge Functions from `meraraw-landing/supabase/functions/`:

- `get-download-url` — licensed users only, returns 5‑min signed R2 URL
- `verify-license` — licensed users only, returns signed JWT for offline app use
- `create-checkout-session`, `stripe-webhook` — existing purchase flow

## 3. License signing keys (app offline activation)

```bash
./scripts/generate-license-keys.sh
```

- **`scripts/license-private.pem`** → paste into Supabase as `LICENSE_SIGNING_PRIVATE_KEY` (full PEM, including headers). Never commit.
- **`src-tauri/license-public.pem`** → committed; baked into the app for Rust-side JWT verification.

If you regenerate keys, redeploy `verify-license` and rebuild the app so the public key matches.

## 4. Website

Download button calls `get-download-url` (no static DMG URL). See `meraraw-landing/README.md`.

## 5. App

Copy `.env.example` → `.env`:

```
VITE_SUPABASE_URL=https://xxxx.supabase.co
VITE_SUPABASE_ANON_KEY=sb_publishable_...
```

**Launch flow:**

**MeraRAW Beta 0.1.0** ships without the in-app license gate — download and open, no sign-in.

For the future full release:

1. Rust checks saved JWT in Application Support (`license_check_local`)
2. Valid signature → unlock (offline OK)
3. Missing/invalid → login screen → `verify-license` → save token → unlock

**Dev skip (debug builds only):**

```bash
MERARAW_SKIP_LICENSE=1 npm run tauri dev
```

## 6. Test checklist

- [ ] Unlicensed logged-in user: `get-download-url` → 403
- [ ] Licensed user: download starts; signed URL expires ~5 min
- [ ] Fresh app: login → activates → quit → reopen offline → still unlocked
- [ ] Tampered `license.jwt` → login screen again

## Open decisions (not implemented)

- Periodic online re-validation (refunds)
- Hardware-bound activation (one Mac per license)
- Multiple DMG versions in R2
