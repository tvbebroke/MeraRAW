// Lighttable: browse All Photos / folders / albums, cull, search, info panel.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  addToAlbum,
  createAlbum,
  getAssetDetail,
  getGrid,
  listAlbums,
  listFolders,
  pickFolder,
  rebuildIndex,
  setAssetMeta,
} from "../ipc/commands";
import {
  onCatalogChanged,
  onImportDone,
  onImportProgress,
} from "../ipc/events";
import { setFilmstripKeyboardContext } from "../keyboard/context";
import { ImportReview } from "./ImportReview";
import { setLibraryKeyboardContext } from "../keyboard/context";
import {
  formatAppError,
  type AlbumItem,
  type AssetDetail,
  type FolderItem,
  type GridItem,
  type GridQuery,
  type MetaPatch,
} from "../ipc/types";

const FLAG_ICON: Record<string, string> = { pick: "✓", reject: "✕", none: "" };

type LibraryView =
  | { kind: "all" }
  | { kind: "folder"; root: string }
  | { kind: "album"; id: number; name: string };

type GridMode = "photo" | "square";

function extLabel(filename: string): string {
  const i = filename.lastIndexOf(".");
  return i >= 0 ? filename.slice(i + 1).toUpperCase() : "";
}

/** Parse facet chips from search text for display. */
function parseFacets(text: string | undefined): string[] {
  if (!text) return [];
  return text
    .split(/\s+/)
    .filter((t) => /^(\w+):/.test(t));
}

export function importRootForPath(
  path: string,
  folders: FolderItem[],
): string | undefined {
  let best: FolderItem | undefined;
  for (const f of folders) {
    if (path === f.root || path.startsWith(`${f.root}/`)) {
      if (!best || f.root.length > best.root.length) best = f;
    }
  }
  return best?.root;
}

export function Library({
  onOpen,
  onExportSelection,
  pendingReviewRoot,
  onReviewClosed,
}: {
  onOpen: (path: string) => void;
  onExportSelection?: (paths: string[]) => void;
  pendingReviewRoot?: string | null;
  onReviewClosed?: () => void;
}) {
  const [folders, setFolders] = useState<FolderItem[]>([]);
  const [albums, setAlbums] = useState<AlbumItem[]>([]);
  const [view, setView] = useState<LibraryView>({ kind: "all" });
  const [items, setItems] = useState<GridItem[]>([]);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [anchor, setAnchor] = useState<number | null>(null);
  const [query, setQuery] = useState<GridQuery>({ limit: 500, sort: "captured" });
  const [searchInput, setSearchInput] = useState("");
  const [gridMode, setGridMode] = useState<GridMode>("photo");
  const [showFilterBar, setShowFilterBar] = useState(true);
  const [thumbScale, setThumbScale] = useState(1);
  const [importStatus, setImportStatus] = useState<string | null>(null);
  const [reviewRoot, setReviewRoot] = useState<string | null>(null);

  useEffect(() => {
    if (pendingReviewRoot) setReviewRoot(pendingReviewRoot);
  }, [pendingReviewRoot]);
  const [detail, setDetail] = useState<AssetDetail | null>(null);
  const [keywordDraft, setKeywordDraft] = useState("");
  const queryRef = useRef(query);
  queryRef.current = query;
  const viewRef = useRef(view);
  viewRef.current = view;
  const searchRef = useRef<HTMLInputElement>(null);
  const itemsRef = useRef(items);
  itemsRef.current = items;
  const selectedRef = useRef(selected);
  selectedRef.current = selected;
  const reviewRootRef = useRef(reviewRoot);
  reviewRootRef.current = reviewRoot;

  const refreshFolders = useCallback(async () => {
    try {
      setFolders(await listFolders());
    } catch (e) {
      console.error("folders", e);
    }
  }, []);

  const refreshAlbums = useCallback(async () => {
    try {
      setAlbums(await listAlbums());
    } catch (e) {
      console.error("albums", e);
    }
  }, []);

  const gridQuery = useCallback((): GridQuery => {
    const q = { ...queryRef.current, limit: queryRef.current.limit ?? 500 };
    const v = viewRef.current;
    if (v.kind === "folder") q.folder = v.root;
    else {
      delete q.folder;
    }
    if (v.kind === "album") q.albumId = v.id;
    else {
      delete q.albumId;
    }
    if (searchInput.trim()) q.text = searchInput.trim();
    else delete q.text;
    return q;
  }, [searchInput]);

  const refreshGrid = useCallback(async () => {
    try {
      setItems(await getGrid(gridQuery()));
    } catch (e) {
      console.error("grid", e);
    }
  }, [gridQuery]);

  const refreshDetail = useCallback(async (id: number | null) => {
    if (id === null) {
      setDetail(null);
      return;
    }
    try {
      setDetail(await getAssetDetail(id));
    } catch {
      setDetail(null);
    }
  }, []);

  useEffect(() => {
    void refreshFolders();
    void refreshAlbums();
  }, [refreshFolders, refreshAlbums]);

  useEffect(() => {
    void refreshGrid();
  }, [query, view, searchInput, refreshGrid]);

  useEffect(() => {
    const primary = selected.size === 1 ? [...selected][0] : null;
    void refreshDetail(primary ?? null);
  }, [selected, refreshDetail]);

  useEffect(() => {
    const u1 = onCatalogChanged(() => {
      void refreshFolders();
      void refreshAlbums();
      void refreshGrid();
    });
    const u2 = onImportProgress((p) =>
      setImportStatus(`importing ${p.done}/${p.total}…`),
    );
    const u3 = onImportDone((total) => {
      setImportStatus(total > 0 ? `imported ${total}` : null);
      void refreshFolders();
      void refreshGrid();
      setTimeout(() => setImportStatus(null), 4000);
    });
    return () => {
      [u1, u2, u3].forEach((u) => u.then((f) => f()));
    };
  }, [refreshFolders, refreshAlbums, refreshGrid]);

  async function beginImportBrowse() {
    const folder = await pickFolder();
    if (folder) setReviewRoot(folder);
  }

  async function patchSelected(patch: MetaPatch) {
    if (selected.size === 0) return;
    await setAssetMeta([...selected], patch).catch(console.error);
    void refreshGrid();
    if (selected.size === 1) void refreshDetail([...selected][0]);
  }

  function toggleSelect(id: number, extend: boolean) {
    const idx = items.findIndex((i) => i.id === id);
    setSelected((prev) => {
      const next = new Set(prev);
      if (extend && anchor !== null) {
        const aIdx = items.findIndex((i) => i.id === anchor);
        if (aIdx >= 0 && idx >= 0) {
          const lo = Math.min(aIdx, idx);
          const hi = Math.max(aIdx, idx);
          for (let i = lo; i <= hi; i++) next.add(items[i].id);
          return next;
        }
      }
      if (next.has(id) && next.size === 1) next.delete(id);
      else if (next.has(id) && !extend) next.delete(id);
      else {
        next.clear();
        next.add(id);
      }
      return next;
    });
    setAnchor(id);
  }

  async function handleCreateAlbum() {
    const name = window.prompt("Album name");
    if (!name?.trim()) return;
    try {
      await createAlbum(name.trim());
      void refreshAlbums();
    } catch (e) {
      console.error(e);
    }
  }

  async function handleAddToAlbum(albumId: number) {
    if (selected.size === 0) return;
    try {
      await addToAlbum(albumId, [...selected]);
      void refreshAlbums();
      void refreshDetail(selected.size === 1 ? [...selected][0] : null);
    } catch (e) {
      console.error(e);
    }
  }

  const facetChips = useMemo(() => parseFacets(searchInput), [searchInput]);
  const activeFolder =
    view.kind === "folder" ? folders.find((f) => f.root === view.root) : undefined;
  const selectedPaths = items
    .filter((i) => selected.has(i.id) && i.accessible)
    .map((i) => i.path);

  useEffect(() => {
    setLibraryKeyboardContext({
      patchSelected: (patch) => patchSelected(patch),
      advanceSelection: (delta) => {
        if (reviewRootRef.current) return;
        const gridItems = itemsRef.current;
        const sel = selectedRef.current;
        if (gridItems.length === 0) return;
        const ids = [...sel];
        const primary = ids.length >= 1 ? ids[0] : gridItems[0]?.id;
        if (primary === undefined) return;
        const idx = gridItems.findIndex((i) => i.id === primary);
        if (idx < 0) return;
        const next = idx + delta;
        if (next < 0 || next >= gridItems.length) return;
        toggleSelect(gridItems[next].id, false);
      },
      extendSelection: (delta) => {
        const gridItems = itemsRef.current;
        const sel = selectedRef.current;
        const ids = [...sel];
        const primary = ids.length === 1 ? ids[0] : null;
        const idx = primary !== null ? gridItems.findIndex((i) => i.id === primary) : -1;
        if (idx < 0) return;
        const next = idx + delta;
        if (next < 0 || next >= gridItems.length) return;
        toggleSelect(gridItems[next].id, true);
      },
      selectAll: () => setSelected(new Set(itemsRef.current.map((i) => i.id))),
      deselectAll: () => setSelected(new Set()),
      selectOnlyActive: () => {
        const ids = [...selectedRef.current];
        if (ids.length === 0) return;
        setSelected(new Set([ids[0]]));
      },
      openActive: () => {
        const gridItems = itemsRef.current;
        const sel = selectedRef.current;
        const ids = [...sel];
        const primary = ids.length >= 1 ? ids[0] : null;
        const idx = primary !== null ? gridItems.findIndex((i) => i.id === primary) : -1;
        if (idx >= 0 && gridItems[idx].accessible) onOpen(gridItems[idx].path);
      },
      toggleGridMode: () => setGridMode((m) => (m === "photo" ? "square" : "photo")),
      focusSearch: () => searchRef.current?.focus(),
      toggleFilterBar: () => setShowFilterBar((v) => !v),
      toggleFilters: () =>
        setQuery((q) => ({ ...q, flag: q.flag ? undefined : "pick" })),
      getSelectedCount: () => selectedRef.current.size,
      getItemCount: () => itemsRef.current.length,
      isReviewOpen: () => reviewRootRef.current !== null,
      adjustThumbnailSize: (delta) =>
        setThumbScale((s) => Math.min(2, Math.max(0.5, s + delta * 0.1))),
      jumpGrid: (where) => {
        const gridItems = itemsRef.current;
        if (gridItems.length === 0) return;
        toggleSelect(
          where === "home" ? gridItems[0].id : gridItems[gridItems.length - 1].id,
          false,
        );
      },
      exportSelection: () => {
        const paths = itemsRef.current
          .filter((i) => selectedRef.current.has(i.id) && i.accessible)
          .map((i) => i.path);
        if (paths.length > 0) onExportSelection?.(paths);
      },
      bumpRating: (delta) => {
        const gridItems = itemsRef.current;
        const id = [...selectedRef.current][0];
        if (id === undefined) return;
        const item = gridItems.find((i) => i.id === id);
        if (!item) return;
        const rating = Math.max(0, Math.min(5, item.rating + delta));
        void patchSelected({ rating });
      },
    });
    return () => setLibraryKeyboardContext(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [onOpen, onExportSelection]);

  return (
    <div className="library">
    <div className="library-layout">
      <aside className="library-sidebar">
        <button
          type="button"
          className={`library-nav ${view.kind === "all" ? "active" : ""}`}
          onClick={() => {
            setView({ kind: "all" });
            setSelected(new Set());
          }}
        >
          All Photos
        </button>
        <div className="library-sidebar-head">Folders</div>
        {folders.map((f) => (
          <button
            key={f.root}
            type="button"
            className={`library-nav ${view.kind === "folder" && view.root === f.root ? "active" : ""} ${f.accessible ? "" : "offline"}`}
            onClick={() => {
              setView({ kind: "folder", root: f.root });
              setSelected(new Set());
            }}
            title={f.root}
          >
            {f.accessible ? "📁" : "📂"} {f.name}
            <span className="muted">{f.photoCount}</span>
          </button>
        ))}
        <div className="library-sidebar-head">
          Albums
          <button type="button" className="lr-mini-btn" title="New album" onClick={() => void handleCreateAlbum()}>
            +
          </button>
        </div>
        {albums.map((a) => (
          <button
            key={a.id}
            type="button"
            className={`library-nav ${view.kind === "album" && view.id === a.id ? "active" : ""}`}
            onClick={() => {
              setView({ kind: "album", id: a.id, name: a.name });
              setSelected(new Set());
            }}
          >
            📚 {a.name}
            <span className="muted">{a.photoCount}</span>
          </button>
        ))}
      </aside>

      <div className="library-main">
        <div className="library-toolbar">
          <button className="library-add-btn" onClick={() => void beginImportBrowse()} title="Add photos">
            + Add Photos…
          </button>
          <button onClick={() => void beginImportBrowse()}>Browse…</button>
          <button
            onClick={() => {
              setImportStatus("rebuilding…");
              rebuildIndex().catch((e) =>
                setImportStatus(`rebuild failed: ${formatAppError(e)}`),
              );
            }}
            title="Rebuild the index from folders + sidecars"
          >
            ♻️ Rebuild
          </button>
          <input
            ref={searchRef}
            placeholder="search… (camera:Sony rating:3 flag:pick)"
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
          />
          {showFilterBar && (
          <>
          <select
            value={query.sort ?? "captured"}
            onChange={(e) =>
              setQuery((q) => ({ ...q, sort: e.target.value || undefined }))
            }
          >
            <option value="captured">Sort: capture date</option>
            <option value="imported">Sort: import date</option>
            <option value="rating">Sort: rating</option>
          </select>
          <select
            onChange={(e) =>
              setQuery((q) => ({
                ...q,
                ratingMin: e.target.value ? parseInt(e.target.value, 10) : undefined,
              }))
            }
          >
            <option value="">any rating</option>
            {[1, 2, 3, 4, 5].map((r) => (
              <option key={r} value={r}>
                ≥ {"★".repeat(r)}
              </option>
            ))}
          </select>
          <select
            onChange={(e) =>
              setQuery((q) => ({ ...q, flag: e.target.value || undefined }))
            }
          >
            <option value="">any flag</option>
            <option value="pick">picks</option>
            <option value="reject">rejects</option>
          </select>
          <label className="muted" style={{ display: "flex", gap: 4 }}>
            <input
              type="checkbox"
              onChange={(e) =>
                setQuery((q) => ({ ...q, blurryOnly: e.target.checked }))
              }
            />
            blurry
          </label>
          <label className="muted" style={{ display: "flex", gap: 4 }}>
            <input
              type="checkbox"
              onChange={(e) =>
                setQuery((q) => ({ ...q, dupesOnly: e.target.checked }))
              }
            />
            dupes
          </label>
          </>
          )}
          <button
            type="button"
            className={gridMode === "square" ? "active" : ""}
            onClick={() => setGridMode((m) => (m === "photo" ? "square" : "photo"))}
            title="Toggle square grid (G)"
          >
            {gridMode === "square" ? "▣ Square" : "▢ Grid"}
          </button>
          {selected.size > 0 && onExportSelection && (
            <button type="button" onClick={() => onExportSelection(selectedPaths)}>
              Export {selected.size}…
            </button>
          )}
          {selected.size > 0 && albums.length > 0 && (
            <select
              defaultValue=""
              onChange={(e) => {
                const id = parseInt(e.target.value, 10);
                if (id > 0) void handleAddToAlbum(id);
                e.target.value = "";
              }}
            >
              <option value="">Add to album…</option>
              {albums.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          )}
          {importStatus && <span className="muted">{importStatus}</span>}
          <span className="muted" style={{ marginLeft: "auto" }}>
            {items.length} photos · G grid · 0-5 rate · P/X/U flags · ⏎ develop
          </span>
        </div>

        {facetChips.length > 0 && (
          <div className="search-facets">
            {facetChips.map((chip) => (
              <span key={chip} className="search-facet-chip">
                {chip}
              </span>
            ))}
          </div>
        )}

        {view.kind === "folder" && activeFolder && !activeFolder.accessible ? (
          <div className="library-offline-banner">
            <strong>Folder not connected.</strong>{" "}
            <span className="muted">
              Connect the volume at <code>{activeFolder.root}</code> to view originals.
            </span>
          </div>
        ) : null}

        <div
          className={`library-grid ${gridMode === "square" ? "square-grid" : ""}`}
          style={{ ["--thumb-scale" as string]: thumbScale }}
        >
          {items.map((it) => (
            <div
              key={it.id}
              className={`grid-cell ${selected.has(it.id) ? "selected" : ""} ${!it.accessible ? "offline" : ""}`}
              onClick={(e) => toggleSelect(it.id, e.shiftKey || e.metaKey)}
              onDoubleClick={() => it.accessible && onOpen(it.path)}
              title={it.filename}
            >
              {it.accessible && it.hasThumb ? (
                <img
                  src={`thumb://localhost/${it.id}?tier=t`}
                  loading="lazy"
                  alt={it.filename}
                />
              ) : (
                <div className="thumb-placeholder">
                  {it.accessible ? it.filename : "⚠ offline"}
                </div>
              )}
              {gridMode === "square" && (
                <span className="grid-ext-label">{extLabel(it.filename)}</span>
              )}
              <div className="cell-badges">
                {it.rating > 0 && <span>{"★".repeat(it.rating)}</span>}
                {it.flag !== "none" && <span>{FLAG_ICON[it.flag]}</span>}
                {it.hasEdits && <span title="has edits">✎</span>}
              </div>
            </div>
          ))}
          {items.length === 0 && (
            <div className="library-grid-empty muted">
              No photos match this view.
            </div>
          )}
        </div>
      </div>

      {detail && (
        <aside className="library-info">
          <h3>Info</h3>
          <div className="library-info-row">
            <span className="muted">File</span>
            <span>{detail.filename}</span>
          </div>
          <div className="library-info-row">
            <span className="muted">Camera</span>
            <span>
              {detail.cameraMake ?? "—"} {detail.cameraModel ?? ""}
            </span>
          </div>
          <div className="library-info-row">
            <span className="muted">Lens</span>
            <span>{detail.lens ?? "—"}</span>
          </div>
          <div className="library-info-row">
            <span className="muted">Exposure</span>
            <span>
              {detail.shutter ?? "—"} · ISO {detail.iso ?? "—"} ·{" "}
              {detail.aperture ? `f/${detail.aperture.toFixed(1)}` : "—"}
            </span>
          </div>
          <div className="library-info-row">
            <span className="muted">Captured</span>
            <span>{detail.capturedAt ?? "—"}</span>
          </div>
          <div className="library-info-row">
            <span className="muted">Dimensions</span>
            <span>
              {detail.width}×{detail.height}
            </span>
          </div>
          <div className="library-info-row">
            <span className="muted">Rating</span>
            <span>{detail.rating > 0 ? "★".repeat(detail.rating) : "—"}</span>
          </div>
          <div className="library-info-row">
            <span className="muted">Flag</span>
            <span>{detail.flag}</span>
          </div>
          {detail.albums.length > 0 && (
            <div className="library-info-row">
              <span className="muted">Albums</span>
              <span>{detail.albums.join(", ")}</span>
            </div>
          )}
          <div className="library-info-keywords">
            <span className="muted">Keywords</span>
            <div className="keyword-tags">
              {detail.keywords.map((kw) => (
                <button
                  key={kw}
                  type="button"
                  className="keyword-tag"
                  onClick={() =>
                    void patchSelected({ removeKeyword: kw }).then(() =>
                      refreshDetail(detail.id),
                    )
                  }
                  title="Remove keyword"
                >
                  {kw} ×
                </button>
              ))}
            </div>
            <div className="lr-add-row">
              <input
                value={keywordDraft}
                placeholder="Add keyword"
                onChange={(e) => setKeywordDraft(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && keywordDraft.trim()) {
                    void patchSelected({ addKeyword: keywordDraft.trim() }).then(() => {
                      setKeywordDraft("");
                      void refreshDetail(detail.id);
                    });
                  }
                }}
              />
            </div>
          </div>
        </aside>
      )}

      {reviewRoot && (
        <ImportReview
          root={reviewRoot}
          onClose={() => {
            setReviewRoot(null);
            onReviewClosed?.();
          }}
          onDone={(root) => {
            setView({ kind: "folder", root });
            setReviewRoot(null);
            onReviewClosed?.();
          }}
        />
      )}
    </div>
    </div>
  );
}

/** Slim horizontal strip for develop view — scoped to the current import folder. */
export function Filmstrip({
  currentPath,
  onOpen,
}: {
  currentPath: string | null;
  onOpen: (path: string) => void;
}) {
  const [items, setItems] = useState<GridItem[]>([]);
  const [folderRoot, setFolderRoot] = useState<string | undefined>();

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const folders = await listFolders();
        const root = currentPath
          ? importRootForPath(currentPath, folders)
          : undefined;
        if (cancelled) return;
        setFolderRoot(root);
        const grid = await getGrid({
          folder: root,
          limit: 200,
        });
        if (!cancelled) setItems(grid);
      } catch {
        if (!cancelled) setItems([]);
      }
    }
    void load();
    const un = onCatalogChanged(() => void load());
    return () => {
      cancelled = true;
      un.then((f) => f());
    };
  }, [currentPath]);

  useEffect(() => {
    setFilmstripKeyboardContext({
      advance: (delta) => {
        if (!currentPath || items.length === 0) return;
        const idx = items.findIndex((i) => i.path === currentPath);
        if (idx < 0) return;
        const next = idx + delta;
        if (next < 0 || next >= items.length) return;
        const it = items[next];
        if (it.accessible) onOpen(it.path);
      },
    });
    return () => setFilmstripKeyboardContext(null);
  }, [items, currentPath, onOpen]);

  if (items.length === 0) return null;
  return (
    <div className="filmstrip" title={folderRoot}>
      {items.map((it) => (
        <img
          key={it.id}
          src={
            it.accessible && it.hasThumb
              ? `thumb://localhost/${it.id}?tier=t`
              : undefined
          }
          className={`${currentPath === it.path ? "current" : ""} ${!it.accessible ? "offline" : ""}`}
          onClick={() => it.accessible && onOpen(it.path)}
          loading="lazy"
          alt={it.filename}
          title={it.accessible ? it.filename : `${it.filename} (offline)`}
        />
      ))}
    </div>
  );
}
