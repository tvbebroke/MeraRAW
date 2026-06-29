# 07 — Organize Photos

**Applies to:** Desktop v2.0 / Mobile v4.0 / Web — **[BUILD: October 2018]**

## Purpose
Library organization model: albums, metadata, flags/ratings, search/filter/sort, stacks,
and People.

## 7.1 Organizing model
- **Albums** are the primary user grouping (CC equivalent of Classic Collections).
  Optional **album folders** for hierarchy.
- **All Photos** is the global set; removing a photo from an album does **not** delete it
  from All Photos.
- A photo can belong to **multiple albums**; `Info panel > Albums` lists membership.
  `[BUILD: v1.5 Aug 2018 — "see which albums a photo belongs to"]`

## 7.2 Views
- **Photo Grid**, **Square Grid** (G), **Detail**. Square Grid shows file extension at
  thumbnail bottom-left. `[BUILD: v1.5]`

## 7.3 Metadata, flags, ratings
- **Info panel** shows EXIF/IPTC metadata per selected photo.
- **Flags** (pick/reject), **star ratings**, keywords — used as filter facets.

## 7.4 Search / filter / sort `[BUILD: 2019 improved]`
- **Type-ahead search** with suggestions; searches cameras, locations, metadata.
- **Scoped filters** via `facet:` syntax (e.g. `camera:`); active filters shown as chips
  in the search box.
- Sort orders over capture time, import time, modified, rating, etc.

## 7.5 Stacks
- Photos can be **stacked**; filtering inside stacks is supported `[BUILD: v1.5 fix]`.

## 7.6 People
- Face-detection-driven People grouping (full spec in file 15).

## IMPL
```
Album { id, name, parentFolderId?, localOffline:bool, photoIds:[] }
Photo { id, allPhotos:true, albumIds:[], flag, rating, keywords:[], exif, faces:[] }
SearchQuery { text, facets:{ camera, location, ... } }
```
- Membership is many-to-many (`Photo.albumIds`); deleting from album mutates the join, not
  the photo.
- Implement search as a faceted index; expose `facet:value` parsing + chip UI.
- Offline flag is **per-album** (`localOffline`) — see file 10 / file 12 (v1.5 change that
  removed per-photo offline in favor of per-album).
