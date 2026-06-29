# 04 — Creative Cloud Photography Plans | Common Questions

**Applies to:** Account/entitlement layer — **[BUILD: October 2018]**

## Purpose
Commercial/entitlement context (storage tiers, plan differences). Minimal feature logic.

## Implementation-relevant points
- **Cloud storage quota** is plan-bound and surfaced in the UI (used vs available). The
  app must read and display quota (see file 10, Account preferences).
- Feature parity between desktop/mobile/web is assumed within a plan.

## IMPL
- Model an `entitlement` object: `{ storageQuotaBytes, usedBytes, plan }`. Surface in
  settings; block uploads when over quota with a clear error path.
