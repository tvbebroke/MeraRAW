# MeraRAW — Tauri audit follow-ups

**Date:** 2026-07-13  
**Branch:** `MeraRAW-0.1.4`  
**Context:** Full pass against [dchuk/claude-code-tauri-skills](https://github.com/dchuk/claude-code-tauri-skills/tree/main/tauri) (39 skills). Hardening landed in the commit that added this file.

Interactive scorecard (local):  
`~/.cursor/projects/Users-kaimai-Desktop-meratech-editor/canvases/tauri-skills-audit.canvas.tsx`

---

## What already shipped in this commit

| Area | Change |
|------|--------|
| CSP / XSS | Restrictive CSP + `freezePrototype` + security headers |
| ACL | `src-tauri/permissions/` + AppManifest command enrollment; tighter capabilities |
| Paths | Canonicalize / deny sensitive paths on file IPC (`src-tauri/src/paths.rs`) |
| URLs | `open_external_url` is **https-only**; no `window.open` fallback |
| License | Webview → Rust only (removed unused `@supabase/supabase-js`) |
| Packaging | `mainBinaryName`, `icon.ico`, `Info.plist`, `removeUnusedCommands`, size-oriented release profile |
| Vite | Tauri `build.target` / `envPrefix` / bind `127.0.0.1` / headers |
| CI | `.github/workflows/ci.yml` (audit + check); release workflow audits first |
| Deps | `cargo audit` clean of vulns (`plist`/`quick-xml`, `crossbeam-epoch`, `quinn-proto`) |

---

## Exact next steps (do these in order)

### 1. Smoke-test the hardened app locally

```bash
cd ~/Desktop/meratech-editor
npx tauri dev
```

Verify:

- [ ] App window opens (CSP did not break React styles / scripts)
- [ ] Open a RAW / JPEG (path validation works)
- [ ] Library thumbs load (`thumb://`)
- [ ] Develop preview loads (`frame://` fetch)
- [ ] Sign-in / license still works (Rust HTTPS path)
- [ ] Early Supporter / purchase opens **https** in the system browser
- [ ] Import folder + export still work

If the webview is blank or images fail, check DevTools console for CSP violations and adjust `app.security.csp` in `src-tauri/tauri.conf.json` (usually `img-src` / `connect-src` for `frame:` / `thumb:`).

### 2. Push the branch (when ready)

```bash
git push -u origin HEAD
```

Confirm GitHub Actions **CI** runs green on the push (npm audit + cargo audit + `cargo check` + `tsc`).

### 3. Windows Authenticode signing (blocks SmartScreen)

You still need a **real OV/EV code-signing certificate**. Until then, Windows installers from CI stay unsigned.

1. Purchase / receive an OV or EV Authenticode cert (PFX).
2. Export the cert thumbprint (SHA1), e.g. from Keychain / `certmgr` / PowerShell.
3. In GitHub repo **Settings → Secrets and variables → Actions**, add:
   - `WINDOWS_CERTIFICATE` — base64 of the `.pfx`  
     `base64 -i cert.pfx | pbcopy`
   - `WINDOWS_CERTIFICATE_PASSWORD` — PFX password
4. Edit `src-tauri/tauri.conf.json` → `bundle.windows` and add:

```json
"certificateThumbprint": "YOUR_THUMBPRINT_HEX_NO_SPACES",
"digestAlgorithm": "sha256",
"timestampUrl": "http://timestamp.digicert.com"
```

5. Tag a release (or run the Release workflow) and confirm the `.exe` is signed:

```powershell
Get-AuthenticodeSignature ".\MeraRAW Win x.y.z.exe"
```

### 4. Optional: macOS build in CI

Today Mac DMGs are built + notarized **locally** (`npm run release` / `npm run notarize`) because `signingIdentity` is hard-coded.

To move Mac into Actions later:

1. Add secrets: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID` / API key (`APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_KEY_PATH`), `KEYCHAIN_PASSWORD`.
2. Add a `macos-latest` (and optionally `macos-14` Intel) job that imports the cert into a temporary keychain before `npm run tauri build`.
3. Run notarization in CI or keep local staple for the first few releases.

### 5. Optional polish (not blockers)

| Item | Why | Action |
|------|-----|--------|
| Isolation pattern | Extra XSS containment | Tauri `app.security.pattern.use: "isolation"` + isolation hook |
| Vitest + `mockIPC` | Frontend IPC unit tests | Add Vitest; mock `@tauri-apps/api` |
| Wire selftest into CI | Catch regressions | Linux job with `MERATECH_SELFTEST=1` (headless-friendly subset) |
| Linux GPG / AppImage signing | Trust on Linux | Optional; document unsigned for beta |
| CrabNebula / Tauri updater | Auto-update | Only if you want in-app updates beyond R2 + meratech.co |
| Single version-bump script | Avoid drift | One script updating `package.json` + both `Cargo.toml` + `tauri.conf.json` |

### 6. Resume product work

Audit hardening is done. Suggested feature tracks (specs already in repo):

1. **AI denoise** — `meraraw-denoise-project-description.md`
2. **Face / AI masking** — `meraraw-masking-face-ai-project-description.md`
3. Next beta cut (`0.1.5`) after smoke-test + (ideally) Windows signing

---

## Commands cheat sheet

```bash
# Dev
npx tauri dev

# Frontend typecheck
npx tsc --noEmit

# Rust check + path tests
cd src-tauri && cargo check && cargo test --bin meratech-editor paths::

# Audits
npm audit
cd src-tauri && cargo audit

# Local macOS beta DMG + notarize
npm run release
npm run notarize
```

---

## Do not commit / ignore

These were left untracked on purpose:

- `claude-skills-main/` — vendor dump, not product source
- `dist-local/` — local build output
- `.self-eval-scores.jsonl` — local eval noise
