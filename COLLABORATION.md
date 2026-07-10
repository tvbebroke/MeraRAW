# Collaboration & sharing guide

This repo (`kaimai-apex/MeraRAW`) is the **full, private** MeraRAW product: the
editor UI, the proprietary Rust image engine, and the release/infra tooling.
This document says **what is safe to share and what is not**, so parts can be
handed to outside collaborators (e.g. a front-end intern) without leaking the
image algorithms or any secrets.

> **TL;DR for the intern:** don't give them access to *this* repo. Give them the
> separate **`meraraw-ui`** repo (see the bottom of this file), which contains
> only the front-end and a browser mock of the backend.

## Sensitivity map

| Area | What it is | Share? | Why |
|---|---|---|---|
| `src/` | React/TS **editor UI** (components, viewport, panels, keyboard, state, IPC contracts) | ✅ **Shareable** | Pure front-end. No algorithms, no secrets. This is what the intern works on. |
| `src/license/`, `src/services/`, `src/analytics/` | Client-side license/purchase/analytics (Supabase **anon** key only) | ✅ Shareable | Uses the public publishable key; no private credentials. Lives inside `src/`. |
| `keybinds/keymap.json` | Keyboard map imported by `src/keyboard` | ✅ Shareable | Data file the UI needs. |
| `public/`, `assets/`, `index.html` | Icons, logo, HTML entry | ✅ Shareable | Static front-end assets. |
| `src-tauri/core/` | **Rust image engine** — GPU render graph, demosaic engines, RAW pipeline, profiles | 🔴 **Private** | The proprietary "special sauce." Never share. |
| `src-tauri/src/` | Tauri shell + ISP/tone/grading/color modules, assistant, license verification | 🔴 Private | Backend algorithms + license signing logic. |
| `scripts/` | Release, notarization, R2 upload, **license key generation** | 🔴 Private | Infra + `license-private.pem` lives here. |
| `.env` | Anthropic key, Apple notarization creds, Cloudflare R2 keys | ⛔ **Secret** | Never commit, never share. Already gitignored. |
| `scripts/license-private.pem` | License **signing private key** | ⛔ Secret | Never commit, never share. Already gitignored. |
| `src-tauri/license-public.pem` | License **public** key | ✅ (technically shareable) | Public half; not needed by the UI though. |
| `meraraw-nextbuild/`, `docs/`, `*-spec.md`, `*-project-description.md` | Design/spec docs | 🟡 Internal | Not secret, but internal planning — don't include in the shared UI repo. |
| `meraraw-derivatives/`, `presets/`, `preset-sources/`, `dist/`, `release/`, `dist-local/` | Generated data / build output | ⬜ N/A | Not source; gitignored or irrelevant to sharing. |

### Secret-hygiene facts (verified)
- `.env` and `scripts/license-private.pem` were **never committed** — they've
  been gitignored from the start, so a clone of this repo contains no hard
  secrets.
- The only key in tracked source is the Supabase **publishable/anon** key
  (`src/config.ts`), which is designed to be public.
- **Caveat:** those two secret files DO exist in the working directory. Never
  hand someone a *zip/copy of the folder* — only ever grant **git** access, and
  only to a repo that is meant to be shared.

## The shared front-end repo: `meraraw-ui`

Rather than give collaborators access to this repo, the front-end has been
extracted into a standalone repo at **`../meraraw-ui`** (sibling of this one):

- Contains only the ✅ rows above (`src/`, `public/`, `assets/`,
  `keybinds/keymap.json`, `index.html`) plus a small `mock/` backend.
- The `mock/` folder replaces the Rust engine with an in-memory stand-in
  (aliased in via `vite.config.ts`), so the intern can `npm install && npm run
  dev` and get a working editor UI **in a plain browser** — no Tauri, no engine,
  no secrets.
- It has its **own fresh git history** (no commits from this repo are carried
  over), so nothing from here leaks through `git log`.

To create the collaborator's remote: push `../meraraw-ui` to a new private
GitHub repo and add the intern to *that* repo only.

### Keeping the UI repo in sync

When the UI changes here, push the source into the shared repo with:

```bash
bash scripts/sync-ui.sh
```

This one-way-mirrors the shareable source (`src/`, `public/`, `assets/`,
`keybinds/`, `index.html`) into `../meraraw-ui`. It deliberately does **not**
touch that repo's `mock/`, `vite.config.ts`, `package.json`, or `README.md` —
those are UI-repo-specific. Review + commit in `../meraraw-ui` afterward.

## Housekeeping (optional, not done automatically)

These untracked items are sitting in the working tree and are safe to remove or
add to `.gitignore` — flagged, not touched:
`claude-skills-main/`, `dist-local/`, `.venv-r2/`, `.self-eval-scores.jsonl`.
