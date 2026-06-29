# 15 — Find & Organize Photos of People (People View)

**Applies to:** Desktop v2.0 / Mobile v4.0 — **[BUILD: October 2018]**

## Purpose
Face-detection + clustering feature that auto-groups photos by person.

## Behavior
- **People View** runs **face detection** across the library and **clusters** detected
  faces into per-person groups.
- User confirms/names clusters; can **merge** clusters (same person) and **assign names**.
- Named people become a **search/filter facet** (find all photos of a person).
- Processing is ongoing/background as new photos arrive; clustering happens server/cloud-side
  in the CC model so it's consistent across devices.

## IMPL
```
Face { id, photoId, bbox, embedding }
PersonCluster { id, name?, faceIds:[], confirmed:bool }
PeopleIndex { detect(photo) -> [Face]; cluster([Face]) -> [PersonCluster] }
```
- Pipeline: **detect** (bounding boxes + embeddings) → **cluster** (group by embedding
  distance) → **user curation** (name/merge/split) → expose as search facet.
- In a cloud model, run detection/clustering server-side and sync `PersonCluster` results;
  clients render and curate.
- Naming + merges must be **idempotent and reversible**; store user decisions separately
  from the raw clustering so re-clustering doesn't lose names.
- Privacy: face embeddings are sensitive — keep them account-scoped; never expose in
  shared galleries unless the user opts in.

## Notes
- This is the most ML-heavy chapter; the document describes behavior, not algorithms.
  Implementation will require a face-detection + embedding model and a clustering step
  (e.g. agglomerative over cosine distance) — chosen independently of Adobe's internals.
