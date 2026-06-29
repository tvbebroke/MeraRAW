// Lightroom-style local folder tree for the develop left rail.
import { useCallback, useEffect, useState } from "react";
import { browseRoots, listDir, pickFolder } from "../../ipc/commands";
import type { BrowseRoot, DirEntry } from "../../ipc/types";

const IMAGE_EXT = new Set([
  "arw",
  "nef",
  "cr2",
  "cr3",
  "dng",
  "raf",
  "orf",
  "rw2",
  "jpg",
  "jpeg",
  "png",
  "tif",
  "tiff",
  "heic",
  "heif",
]);

function isImageFile(name: string): boolean {
  const dot = name.lastIndexOf(".");
  if (dot < 0) return false;
  return IMAGE_EXT.has(name.slice(dot + 1).toLowerCase());
}

function isHidden(name: string): boolean {
  return name.startsWith(".");
}

function parentDir(path: string): string | null {
  const i = path.lastIndexOf("/");
  if (i <= 0) return null;
  return path.slice(0, i);
}

function ancestors(path: string): string[] {
  const out: string[] = [];
  let p: string | null = path;
  while (p) {
    out.push(p);
    p = parentDir(p);
  }
  return out;
}

function TreeFolder({
  name,
  path,
  depth,
  expanded,
  loading,
  currentPath,
  onToggle,
  children,
}: {
  name: string;
  path: string;
  depth: number;
  expanded: boolean;
  loading: boolean;
  currentPath: string | null;
  onToggle: () => void;
  children?: React.ReactNode;
}) {
  const active =
    currentPath === path ||
    (currentPath?.startsWith(`${path}/`) ?? false);
  return (
    <div className="tree-folder">
      <button
        type="button"
        className={`tree-row ${active ? "active" : ""}`}
        style={{ paddingLeft: `${depth * 14 + 6}px` }}
        onClick={onToggle}
        title={path}
      >
        <span className="tree-chevron">{loading ? "…" : expanded ? "▾" : "▸"}</span>
        <span className="tree-icon" aria-hidden>
          📁
        </span>
        <span className="tree-label">{name}</span>
      </button>
      {expanded ? children : null}
    </div>
  );
}

function TreeFile({
  entry,
  depth,
  currentPath,
  onOpen,
}: {
  entry: DirEntry;
  depth: number;
  currentPath: string | null;
  onOpen: (path: string) => void;
}) {
  return (
    <button
      type="button"
      className={`tree-row tree-file ${currentPath === entry.path ? "active" : ""}`}
      style={{ paddingLeft: `${depth * 14 + 22}px` }}
      onClick={() => onOpen(entry.path)}
      title={entry.path}
    >
      <span className="tree-icon" aria-hidden>
        🖼
      </span>
      <span className="tree-label">{entry.name}</span>
    </button>
  );
}

function FolderContents({
  path,
  depth,
  expanded,
  cache,
  setCache,
  expandedDirs,
  toggleDir,
  loadingDirs,
  setLoadingDirs,
  currentPath,
  onOpen,
}: {
  path: string;
  depth: number;
  expanded: boolean;
  cache: Map<string, DirEntry[]>;
  setCache: React.Dispatch<React.SetStateAction<Map<string, DirEntry[]>>>;
  expandedDirs: Set<string>;
  toggleDir: (path: string) => void;
  loadingDirs: Set<string>;
  setLoadingDirs: React.Dispatch<React.SetStateAction<Set<string>>>;
  currentPath: string | null;
  onOpen: (path: string) => void;
}) {
  const entries = cache.get(path);

  useEffect(() => {
    if (!expanded || entries) return;
    let cancelled = false;
    setLoadingDirs((s) => new Set(s).add(path));
    listDir(path)
      .then((list) => {
        if (cancelled) return;
        const filtered = list.filter((e) => !isHidden(e.name));
        setCache((c) => new Map(c).set(path, filtered));
      })
      .catch(() => {
        if (!cancelled) setCache((c) => new Map(c).set(path, []));
      })
      .finally(() => {
        if (cancelled) return;
        setLoadingDirs((s) => {
          const n = new Set(s);
          n.delete(path);
          return n;
        });
      });
    return () => {
      cancelled = true;
    };
  }, [expanded, path, entries, setCache, setLoadingDirs]);

  if (!expanded || !entries) return null;

  return (
    <>
      {entries.map((entry) =>
        entry.isDir ? (
          <TreeFolder
            key={entry.path}
            name={entry.name}
            path={entry.path}
            depth={depth + 1}
            expanded={expandedDirs.has(entry.path)}
            loading={loadingDirs.has(entry.path)}
            currentPath={currentPath}
            onToggle={() => toggleDir(entry.path)}
          >
            <FolderContents
              path={entry.path}
              depth={depth + 1}
              expanded={expandedDirs.has(entry.path)}
              cache={cache}
              setCache={setCache}
              expandedDirs={expandedDirs}
              toggleDir={toggleDir}
              loadingDirs={loadingDirs}
              setLoadingDirs={setLoadingDirs}
              currentPath={currentPath}
              onOpen={onOpen}
            />
          </TreeFolder>
        ) : isImageFile(entry.name) ? (
          <TreeFile
            key={entry.path}
            entry={entry}
            depth={depth + 1}
            currentPath={currentPath}
            onOpen={onOpen}
          />
        ) : null,
      )}
    </>
  );
}

export function LocalBrowser({
  currentPath,
  onOpen,
}: {
  currentPath: string | null;
  onOpen: (path: string) => void;
}) {
  const [roots, setRoots] = useState<BrowseRoot[]>([]);
  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set());
  const [cache, setCache] = useState<Map<string, DirEntry[]>>(new Map());
  const [loadingDirs, setLoadingDirs] = useState<Set<string>>(new Set());

  useEffect(() => {
    browseRoots().then(setRoots).catch(console.error);
  }, []);

  // Expand tree to reveal the currently open file.
  useEffect(() => {
    if (!currentPath) return;
    setExpandedDirs((prev) => {
      const next = new Set(prev);
      for (const p of ancestors(currentPath)) next.add(p);
      for (const p of ancestors(parentDir(currentPath) ?? currentPath)) next.add(p);
      return next;
    });
  }, [currentPath]);

  const toggleDir = useCallback((path: string) => {
    setExpandedDirs((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

  async function browseOther() {
    const folder = await pickFolder();
    if (!folder) return;
    setExpandedDirs((prev) => new Set(prev).add(folder));
    setCache(new Map());
  }

  return (
    <div className="local-browser">
      <div className="local-browser-head">
        <span className="local-browser-tab active">Browse</span>
        <button type="button" className="lr-mini-btn" title="Choose folder…" onClick={() => void browseOther()}>
          …
        </button>
      </div>
      <div className="local-browser-tree">
        {roots.map((root) => (
          <TreeFolder
            key={root.path}
            name={root.name}
            path={root.path}
            depth={0}
            expanded={expandedDirs.has(root.path)}
            loading={loadingDirs.has(root.path)}
            currentPath={currentPath}
            onToggle={() => toggleDir(root.path)}
          >
            <FolderContents
              path={root.path}
              depth={0}
              expanded={expandedDirs.has(root.path)}
              cache={cache}
              setCache={setCache}
              expandedDirs={expandedDirs}
              toggleDir={toggleDir}
              loadingDirs={loadingDirs}
              setLoadingDirs={setLoadingDirs}
              currentPath={currentPath}
              onOpen={onOpen}
            />
          </TreeFolder>
        ))}
        {roots.length === 0 && (
          <p className="muted sm" style={{ padding: 8 }}>
            Loading folders…
          </p>
        )}
      </div>
    </div>
  );
}
