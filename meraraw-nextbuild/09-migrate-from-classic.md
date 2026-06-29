# 09 — Migrate Photos & Videos from Lightroom Classic CC → Lightroom CC

**Applies to:** Desktop v2.0 — **[BUILD: October 2018]**

## Purpose
One-way catalog importer. Most relevant chapter for understanding the **data-model delta**
between Classic (local catalog) and CC (cloud).

## Source vs target model
- **Classic catalog** = `.lrcat` database. Each record holds: a **reference** to the
  photo's on-disk location, processing instructions (adjustments), and metadata
  (keywords, ratings).
- **CC** = originals stored **full-resolution in the cloud**, synced to devices on demand.

## Migration behavior
- Migration **uploads each cataloged photo** to the cloud at full resolution.
- **Collections → Albums** automatically.
- After migration, originals can be deleted from local disk to reclaim space (cloud is
  source of truth).
- **Not all Classic data survives migration** — the document flags that certain catalog
  data won't carry over (e.g. some develop/organizational constructs without a CC
  equivalent). Treat unmapped fields as lossy.
- Sync requires CC or **latest** Classic; legacy (Lightroom 6 / 2015) no longer syncs.

## IMPL
```
ClassicMigration {
  read(.lrcat)                       // SQLite catalog
  for each photo: resolveOriginal() -> upload(fullRes) -> createCloudAsset()
  map: collections -> albums
  carryOver: adjustments(where mappable), keywords, ratings
  report: unmappedData[]             // surface lossy fields to user
}
```
- Classic `.lrcat` is **SQLite** — read-only parse; never write back.
- Build an explicit **field-mapping table** Classic→CC; log anything dropped.
- Migration is **idempotent-ish**: re-running should not duplicate already-uploaded assets
  (dedupe by content hash).
