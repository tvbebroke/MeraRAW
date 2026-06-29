# 03 — Lightroom CC | Common Questions

**Applies to:** Desktop v2.0 / Mobile v4.0 — **[BUILD: October 2018]**

## Purpose
FAQ chapter. Establishes the **cloud-first mental model** that constrains the data layer.

## Load-bearing facts for implementation
- Originals are **uploaded to the cloud at full resolution**; the local machine keeps a
  cache and/or **Smart Previews** (reduced raw proxies that retain raw-edit latitude).
- Photos remain viewable even when the original isn't on the local disk (proxy-backed UI).
- There is **no user-facing catalog file**; identity/state lives in the cloud account.

## IMPL
- Data layer must support a **three-tier asset state**: `original-in-cloud-only`,
  `smart-preview-local`, `original-local`. UI must render from the best available tier.
- Edits apply to the proxy and re-render against the original on demand/download.
