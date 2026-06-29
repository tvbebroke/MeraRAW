# 10 — Set Preferences

**Applies to:** Desktop v2.0 — **[BUILD: October 2018]**

## Purpose
Settings model: Account, Local Storage, and editing/interface preferences.

## 10.1 Account preferences
- Shows Adobe ID account info + **Manage Account** link.
- **Cloud Storage** readout: space **used by backed-up photos** vs **available** quota.

## 10.2 Local storage preferences
- CC **intelligently manages local disk**: keeps working-set photos local, evicts others
  to cloud-only, so the drive never fills. All photos remain viewable via proxies even when
  the original isn't local.
- **Offline availability is per-album** (`Store album locally`) `[BUILD: v1.5 Aug 2018]`
  — the earlier **per-photo** offline option was **removed** at v1.5.

## 10.3 Other preferences (interface/editing)
- Update preferences (Auto-update toggle), interface options, and editing defaults.

## IMPL
```
Preferences {
  account: { adobeId, manageUrl }
  storage: { quotaBytes, usedBytes, cacheSizeLimit, evictionPolicy:lru }
  offlineAlbums: [albumId]          // per-album, not per-photo
  autoUpdate: bool
}
```
- Implement a **cache manager** with an eviction policy (LRU over local originals; never
  evict Smart Previews needed for offline albums).
- Quota display must reflect server truth; reconcile on sync.
