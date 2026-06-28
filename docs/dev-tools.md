# Dev & diagnostic tools

These are **not** part of the acceptance bar. Use when debugging locally.

## Diagnostic examples

```bash
cd src-tauri

# Catalog import smoke (folder scan + import_one)
cargo run -p meratech-core --example import_check -- /path/to/folder

# DCP header probe
cargo run -p meratech-core --example probe_dcp -- /path/to/profile.dcp

# Rebuild profile_index.json from meraraw-derivatives
MERARAW_PROFILES_DIR=/path/to/meraraw-derivatives \
  cargo run -p meratech-core --example index_profiles

# Compact embedded profile index (file paths only)
cargo run -p meratech-core --example compact_profile_index
```

## Frontend harnesses (env-gated)

| Env | Harness |
|-----|---------|
| `MERATECH_SELFTEST=1` | Full in-app integration (`src/selftest.ts`) |
| `MERATECH_LIVE_ASSISTANT=1` | Real Claude export loop (`src/liveassistant.ts`) |
| `MERATECH_VERIFY_SLIDER=1` | Slider regression (`src/verifyslider.ts`) |

All dispatch through `src/devHarness.ts` after `image-ready`.
