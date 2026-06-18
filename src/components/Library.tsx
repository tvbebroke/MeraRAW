// Lighttable: grid + filters + keyboard culling (P5). Thumbs stream in
// via the thumb:// protocol while import runs in the background.
import { useCallback, useEffect, useRef, useState } from "react";
import {
  getGrid,
  importFolder,
  pickFolder,
  rebuildIndex,
  setAssetMeta,
} from "../ipc/commands";
import {
  onCatalogChanged,
  onImportDone,
  onImportProgress,
} from "../ipc/events";
import type { GridItem, GridQuery, MetaPatch } from "../ipc/types";

const FLAG_ICON: Record<string, string> = { pick: "✓", reject: "✕", none: "" };

export function Library({ onOpen }: { onOpen: (path: string) => void }) {
  const [items, setItems] = useState<GridItem[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [query, setQuery] = useState<GridQuery>({ limit: 500 });
  const [importStatus, setImportStatus] = useState<string | null>(null);
  const queryRef = useRef(query);
  queryRef.current = query;

  const refresh = useCallback(async () => {
    try {
      setItems(await getGrid(queryRef.current));
    } catch (e) {
      console.error("grid", e);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [query, refresh]);

  useEffect(() => {
    const u1 = onCatalogChanged(() => void refresh());
    const u2 = onImportProgress((p) =>
      setImportStatus(`importing ${p.done}/${p.total}…`),
    );
    const u3 = onImportDone((total) => {
      setImportStatus(total > 0 ? `imported ${total}` : null);
      void refresh();
      setTimeout(() => setImportStatus(null), 4000);
    });
    return () => {
      [u1, u2, u3].forEach((u) => u.then((f) => f()));
    };
  }, [refresh]);

  async function handleImport() {
    const folder = await pickFolder();
    if (!folder) return;
    setImportStatus("scanning…");
    try {
      const queued = await importFolder(folder);
      if (queued === 0) setImportStatus("nothing new");
    } catch (e) {
      setImportStatus(`import failed: ${String(e)}`);
    }
  }

  async function patchSelected(patch: MetaPatch) {
    if (selected === null) return;
    await setAssetMeta([selected], patch).catch(console.error);
  }

  // keyboard culling: 0-5 rate, p pick, x reject, u unflag, arrows move
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.target as HTMLElement)?.tagName === "INPUT") return;
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
      } else if (e.key === "Enter" && idx >= 0) {
        onOpen(items[idx].path);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [items, selected]);

  return (
    <div className="library">
      <div className="library-toolbar">
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
        {importStatus && <span className="muted">{importStatus}</span>}
        <span className="muted" style={{ marginLeft: "auto" }}>
          {items.length} photos · 0-5 rate · P pick · X reject · ⏎ develop
        </span>
      </div>
      <div className="library-grid">
        {items.map((it) => (
          <div
            key={it.id}
            className={`grid-cell ${selected === it.id ? "selected" : ""}`}
            onClick={() => setSelected(it.id)}
            onDoubleClick={() => onOpen(it.path)}
            title={it.filename}
          >
            {it.hasThumb ? (
              <img
                src={`thumb://localhost/${it.id}?tier=t`}
                loading="lazy"
                alt={it.filename}
              />
            ) : (
              <div className="thumb-placeholder">{it.filename}</div>
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
            No photos yet — Import a folder.
          </div>
        )}
      </div>
    </div>
  );
}

/** Slim horizontal strip for develop view. */
export function Filmstrip({
  currentPath,
  onOpen,
}: {
  currentPath: string | null;
  onOpen: (path: string) => void;
}) {
  const [items, setItems] = useState<GridItem[]>([]);
  useEffect(() => {
    getGrid({ limit: 200 }).then(setItems).catch(() => {});
    const un = onCatalogChanged(() =>
      getGrid({ limit: 200 }).then(setItems).catch(() => {}),
    );
    return () => {
      un.then((f) => f());
    };
  }, []);
  if (items.length === 0) return null;
  return (
    <div className="filmstrip">
      {items.map((it) => (
        <img
          key={it.id}
          src={`thumb://localhost/${it.id}?tier=t`}
          className={currentPath === it.path ? "current" : ""}
          onClick={() => onOpen(it.path)}
          loading="lazy"
          alt={it.filename}
          title={it.filename}
        />
      ))}
    </div>
  );
}
