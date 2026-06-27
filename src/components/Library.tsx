// Lighttable: browse imported folders, then photos within each folder.
// Thumbs stream in via thumb:// while import runs in the background.
import { useCallback, useEffect, useRef, useState } from "react";
import {
  getGrid,
  importFolder,
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
import type { FolderItem, GridItem, GridQuery, MetaPatch } from "../ipc/types";

const FLAG_ICON: Record<string, string> = { pick: "✓", reject: "✕", none: "" };

/** Longest import root that contains `path`. */
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

export function Library({ onOpen }: { onOpen: (path: string) => void }) {
  const [folders, setFolders] = useState<FolderItem[]>([]);
  const [selectedFolder, setSelectedFolder] = useState<string | null>(null);
  const [items, setItems] = useState<GridItem[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [query, setQuery] = useState<GridQuery>({ limit: 500 });
  const [importStatus, setImportStatus] = useState<string | null>(null);
  const queryRef = useRef(query);
  queryRef.current = query;
  const selectedFolderRef = useRef(selectedFolder);
  selectedFolderRef.current = selectedFolder;

  const refreshFolders = useCallback(async () => {
    try {
      setFolders(await listFolders());
    } catch (e) {
      console.error("folders", e);
    }
  }, []);

  const refreshGrid = useCallback(async () => {
    const folder = selectedFolderRef.current;
    if (!folder) {
      setItems([]);
      return;
    }
    try {
      setItems(
        await getGrid({ ...queryRef.current, folder, limit: queryRef.current.limit ?? 500 }),
      );
    } catch (e) {
      console.error("grid", e);
    }
  }, []);

  useEffect(() => {
    void refreshFolders();
  }, [refreshFolders]);

  useEffect(() => {
    void refreshGrid();
  }, [query, selectedFolder, refreshGrid]);

  useEffect(() => {
    const u1 = onCatalogChanged(() => {
      void refreshFolders();
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
  }, [refreshFolders, refreshGrid]);

  const activeFolder = folders.find((f) => f.root === selectedFolder);

  async function handleImport() {
    const folder = await pickFolder();
    if (!folder) return;
    setImportStatus("scanning…");
    try {
      const queued = await importFolder(folder);
      if (queued === 0) setImportStatus("nothing new");
      else setSelectedFolder(folder);
    } catch (e) {
      setImportStatus(`import failed: ${String(e)}`);
    }
  }

  async function patchSelected(patch: MetaPatch) {
    if (selected === null) return;
    await setAssetMeta([selected], patch).catch(console.error);
  }

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.target as HTMLElement)?.tagName === "INPUT") return;
      if (selectedFolder === null) {
        if (e.key === "Escape") return;
        return;
      }
      if (selected === null && items.length > 0 && (e.key === "ArrowRight" || e.key === "ArrowLeft")) {
        setSelected(items[0].id);
        return;
      }
      const idx = items.findIndex((i) => i.id === selected);
      if (e.key >= "0" && e.key <= "5") {
        void patchSelected({ rating: parseInt(e.key, 10) });
      } else if (e.key === "p") {
        void patchSelected({ flag: "pick" });
      } else if (e.key === "x") {
        void patchSelected({ flag: "reject" });
      } else if (e.key === "u") {
        void patchSelected({ flag: "none" });
      } else if (e.key === "ArrowRight" && idx >= 0 && idx < items.length - 1) {
        setSelected(items[idx + 1].id);
      } else if (e.key === "ArrowLeft" && idx > 0) {
        setSelected(items[idx - 1].id);
      } else if (e.key === "Enter" && idx >= 0 && items[idx].accessible) {
        onOpen(items[idx].path);
      } else if (e.key === "Escape") {
        setSelectedFolder(null);
        setSelected(null);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [items, selected, selectedFolder]);

  return (
    <div className="library">
      <div className="library-toolbar">
        {selectedFolder ? (
          <button
            className="library-back"
            onClick={() => {
              setSelectedFolder(null);
              setSelected(null);
            }}
          >
            ← All Folders
          </button>
        ) : null}
        <button onClick={handleImport}>📥 Import Folder…</button>
        <button
          onClick={() => {
            setImportStatus("rebuilding…");
            rebuildIndex().catch((e) => setImportStatus(String(e)));
          }}
          title="Rebuild the index from folders + sidecars"
        >
          ♻️ Rebuild
        </button>
        {selectedFolder ? (
          <>
            <input
              placeholder="search…"
              onChange={(e) =>
                setQuery((q) => ({ ...q, text: e.target.value || undefined }))
              }
            />
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
        ) : null}
        {importStatus && <span className="muted">{importStatus}</span>}
        <span className="muted" style={{ marginLeft: "auto" }}>
          {selectedFolder
            ? `${items.length} photos · 0-5 rate · P pick · X reject · ⏎ develop · Esc back`
            : `${folders.length} folders · Import a folder to begin`}
        </span>
      </div>

      {selectedFolder && activeFolder && !activeFolder.accessible ? (
        <div className="library-offline-banner">
          <strong>Folder not connected.</strong>{" "}
          <span className="muted">
            Connect the volume at{" "}
            <code>{activeFolder.root}</code> to view originals. Cached thumbnails may
            still appear but files cannot be opened.
          </span>
        </div>
      ) : null}

      {selectedFolder ? (
        <div className="library-grid">
          {items.map((it) => (
            <div
              key={it.id}
              className={`grid-cell ${selected === it.id ? "selected" : ""} ${!it.accessible ? "offline" : ""}`}
              onClick={() => setSelected(it.id)}
              onDoubleClick={() => it.accessible && onOpen(it.path)}
              title={it.filename}
            >
              {activeFolder?.accessible && it.accessible && it.hasThumb ? (
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
              <div className="cell-badges">
                {it.rating > 0 && <span>{"★".repeat(it.rating)}</span>}
                {it.flag !== "none" && <span>{FLAG_ICON[it.flag]}</span>}
                {it.hasEdits && <span title="has edits">✎</span>}
              </div>
            </div>
          ))}
          {items.length === 0 && (
            <div className="muted" style={{ padding: 24 }}>
              No photos in this folder.
            </div>
          )}
        </div>
      ) : (
        <div className="folder-list">
          {folders.map((f) => (
            <button
              key={f.root}
              type="button"
              className={`folder-row ${f.accessible ? "" : "offline"}`}
              onClick={() => {
                setSelectedFolder(f.root);
                setSelected(null);
              }}
            >
              <span className="folder-row-icon">{f.accessible ? "📁" : "📂"}</span>
              <span className="folder-row-main">
                <span className="folder-row-name">{f.name}</span>
                <span className="folder-row-path muted" title={f.root}>
                  {f.root}
                </span>
              </span>
              <span className="folder-row-meta">
                <span className={`folder-status ${f.accessible ? "ok" : "err"}`}>
                  {f.accessible ? "connected" : "offline"}
                </span>
                <span className="muted">{f.photoCount} photos</span>
              </span>
            </button>
          ))}
          {folders.length === 0 && (
            <div className="muted" style={{ padding: 24 }}>
              No folders yet — click Import Folder to add photos from a drive or folder.
            </div>
          )}
        </div>
      )}
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
