// Catalog-backed browsing state: current folder listing feeds the
// filmstrip + file browser; clicking a photo opens it in the engine.
import { atom, computed } from "nanostores";
import {
  getGrid,
  importFolder,
  importSelected,
  listFolders,
  pickFiles,
  pickFolder,
  setAssetMeta,
  forgetFolder,
} from "../ipc/commands";
import { onCatalogChanged, onImportDone } from "../ipc/events";
import type { FolderItem, GridItem, GridQuery, MetaPatch } from "../ipc/types";
import { customSchemeUrl } from "../lib/engine/customScheme";
import { currentFolder, lastOpenedDocId, lastOpenedPath, openingPreviewUrl } from "./app";
import { workspace } from "./workspace";
import { extOf, isVideoPath } from "../lib/media";

export const folders = atom<FolderItem[]>([]);
export const photos = atom<GridItem[]>([]);
export const browseBusy = atom(false);

/** Darktable-style collection filters. Catalog `get_grid` is the source of truth. */
export type LibraryFlag = "any" | "pick" | "reject" | "none";

export type LibraryFilters = {
  ratingMin: number;
  flag: LibraryFlag;
  hasEdits: boolean | null;
  blurryOnly: boolean;
  dupesOnly: boolean;
  text: string;
};

export const DEFAULT_LIBRARY_FILTERS: LibraryFilters = {
  ratingMin: 0,
  flag: "any",
  hasEdits: null,
  blurryOnly: false,
  dupesOnly: false,
  text: "",
};

export const libraryFilters = atom<LibraryFilters>({ ...DEFAULT_LIBRARY_FILTERS });

export function libraryFiltersActive(
  f: LibraryFilters = libraryFilters.get(),
): boolean {
  return (
    f.ratingMin > 0 ||
    f.flag !== "any" ||
    f.hasEdits !== null ||
    f.blurryOnly ||
    f.dupesOnly ||
    f.text.trim().length > 0
  );
}

/** Pure mapper: session filters → catalog `GridQuery`. */
export function gridQueryFromState(
  folder: string | null,
  filters: LibraryFilters,
  opts?: { limit?: number; sort?: string },
): GridQuery {
  const text = filters.text.trim();
  return {
    folder: folder ?? undefined,
    limit: opts?.limit ?? 2000,
    sort: opts?.sort ?? "captured",
    ratingMin: filters.ratingMin > 0 ? filters.ratingMin : undefined,
    flag: filters.flag !== "any" ? filters.flag : undefined,
    hasEdits: filters.hasEdits ?? undefined,
    blurryOnly: filters.blurryOnly || undefined,
    dupesOnly: filters.dupesOnly || undefined,
    text: text.length > 0 ? text : undefined,
  };
}

export function setLibraryFilters(patch: Partial<LibraryFilters>): void {
  libraryFilters.set({ ...libraryFilters.get(), ...patch });
}

export async function applyLibraryFilters(
  patch: Partial<LibraryFilters>,
): Promise<void> {
  setLibraryFilters(patch);
  await loadFolder(currentFolder.get());
}

export async function clearLibraryFilters(): Promise<void> {
  libraryFilters.set({ ...DEFAULT_LIBRARY_FILTERS });
  await loadFolder(currentFolder.get());
}

export type FolderMediaKind = "auto" | "photo" | "video";

const FOLDER_KIND_KEY = "meraraw.folderKind";

function readFolderKinds(): Record<string, FolderMediaKind> {
  try {
    const raw = localStorage.getItem(FOLDER_KIND_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const out: Record<string, FolderMediaKind> = {};
    for (const [k, v] of Object.entries(parsed)) {
      if (v === "photo" || v === "video" || v === "auto") out[k] = v;
    }
    return out;
  } catch {
    return {};
  }
}

export const folderKinds = atom<Record<string, FolderMediaKind>>(readFolderKinds());

export function setFolderKind(root: string, kind: FolderMediaKind): void {
  const next = { ...folderKinds.get() };
  for (const k of Object.keys(next)) {
    if (sameFolderPath(k, root)) delete next[k];
  }
  if (kind !== "auto") next[root] = kind;
  folderKinds.set(next);
  try {
    localStorage.setItem(FOLDER_KIND_KEY, JSON.stringify(next));
  } catch {
    /* ignore */
  }
}

const STILL_LIBRARY_EXTS = new Set([
  "raf",
  "arw",
  "cr2",
  "cr3",
  "nef",
  "nrw",
  "dng",
  "orf",
  "rw2",
  "pef",
  "srw",
  "jpg",
  "jpeg",
  "png",
  "tif",
  "tiff",
  "heic",
  "heif",
  "webp",
  "bmp",
]);

/** True when a sidebar item is one imported file (not a folder of siblings). */
export function isLibraryFileRoot(item: Pick<FolderItem, "root" | "isFile">): boolean {
  if (item.isFile) return true;
  const ext = extOf(item.root);
  return Boolean(ext) && (isVideoPath(item.root) || STILL_LIBRARY_EXTS.has(ext));
}

/** Which library sections a folder belongs in. Mixed auto-folders appear in both. */
export function folderSections(
  f: FolderItem,
  kind: FolderMediaKind | undefined,
): ("photo" | "video")[] {
  if (kind === "photo") return ["photo"];
  if (kind === "video") return ["video"];
  const stills = (f.photoCount ?? 0) > 0;
  const clips = (f.videoCount ?? 0) > 0;
  if (stills && clips) return ["photo", "video"];
  if (clips && !stills) return ["video"];
  return ["photo"];
}

/** Catalog rows for the active workspace. Photo never lists clips; video never lists stills. */
export const libraryItems = computed([photos, workspace], (items, ws) =>
  items.filter((i) => {
    const vid = isVideoPath(i.path) || isVideoPath(i.filename);
    return ws === "video" ? vid : !vid;
  }),
);

/** Alias used by shell chrome that still says "folder". */
export const folder = currentFolder;

export function thumbUrl(id: number, tier: "t" | "p" = "t"): string {
  return customSchemeUrl("thumb", `${id}?tier=${tier}`);
}

/** Stable key for a catalog row that may be a virtual copy of the same path. */
export function gridKey(item: Pick<GridItem, "path"> & { docId?: string | null }): string {
  return item.docId ? `${item.path}::${item.docId}` : item.path;
}

/** Grid item matching the currently open image (if visible in this workspace). */
export const activePhoto = computed(
  [libraryItems, lastOpenedPath, lastOpenedDocId],
  (items, path, docId) => {
    if (!path) return null;
    if (docId) {
      const exact = items.find((i) => i.path === path && i.docId === docId);
      if (exact) return exact;
    }
    return items.find((i) => i.path === path) ?? null;
  },
);

function normPath(p: string): string {
  return p
    .replace(/\\/g, "/")
    .replace(/^\/\/\?\//, "")
    .replace(/\/+$/, "")
    .toLowerCase();
}

/** True when two folder roots are the same location (slash/prefix variants). */
export function sameFolderPath(a: string, b: string): boolean {
  const na = normPath(a);
  const nb = normPath(b);
  if (na === nb) return true;
  // macOS /tmp vs /private/tmp, and catalog roots stored with an extra prefix.
  const aTail = na.replace(/^\/private/, "");
  const bTail = nb.replace(/^\/private/, "");
  if (aTail === bTail && aTail.length > 0) return true;
  return na.endsWith("/" + nb) || nb.endsWith("/" + na);
}

export function folderKindFor(
  root: string,
  map: Record<string, FolderMediaKind> = folderKinds.get(),
): FolderMediaKind {
  if (map[root]) return map[root];
  for (const [k, v] of Object.entries(map)) {
    if (sameFolderPath(k, root)) return v;
  }
  return "auto";
}

/** True when `path` is this folder or a file/folder inside it. */
export function isPathInFolder(path: string, root: string): boolean {
  if (sameFolderPath(path, root)) return true;
  const a = normPath(path);
  const b = normPath(root);
  return b.length > 0 && (a.startsWith(b + "/") || a.startsWith(b + "\\"));
}

/** Prefer the catalog's remembered root when the picker path differs only by prefix. */
function resolveCatalogRoot(picked: string, listed: FolderItem[]): string {
  return listed.find((f) => sameFolderPath(f.root, picked))?.root ?? picked;
}

/** Attach ImportDone listener first, then return a promise that settles on the event. */
async function importDoneLatch(timeoutMs = 180_000): Promise<{
  done: Promise<number>;
  cancel: () => void;
}> {
  let settled = false;
  let unlisten: (() => void) | undefined;
  let resolveDone!: (n: number) => void;
  let rejectDone!: (e: Error) => void;
  const done = new Promise<number>((resolve, reject) => {
    resolveDone = resolve;
    rejectDone = reject;
  });
  const timer = window.setTimeout(() => {
    if (settled) return;
    settled = true;
    unlisten?.();
    rejectDone(new Error("import timed out"));
  }, timeoutMs);

  unlisten = await onImportDone((total) => {
    if (settled) return;
    settled = true;
    window.clearTimeout(timer);
    unlisten?.();
    resolveDone(total);
  });

  return {
    done,
    cancel: () => {
      if (settled) return;
      settled = true;
      window.clearTimeout(timer);
      unlisten?.();
    },
  };
}

export async function refreshFolders(): Promise<void> {
  try {
    folders.set(await listFolders());
  } catch {
    folders.set([]);
  }
}

export async function loadFolder(root: string | null): Promise<void> {
  currentFolder.set(root);
  browseBusy.set(true);
  try {
    const grid = await getGrid(gridQueryFromState(root, libraryFilters.get()));
    // Keep inaccessible rows visible — filtering them hid Windows imports
    // when Path::exists() disagreed with the stored path form.
    photos.set(grid);
  } catch {
    photos.set([]);
  } finally {
    browseBusy.set(false);
  }
}

async function waitForImport(root: string, total: number, latch: {
  done: Promise<number>;
  cancel: () => void;
}): Promise<void> {
  if (total <= 0) {
    latch.cancel();
    return;
  }
  await Promise.race([
    latch.done.catch(() => undefined),
    (async () => {
      for (let i = 0; i < 120; i++) {
        await new Promise((r) => setTimeout(r, 250));
        const grid = await getGrid({ folder: root, limit: 1, sort: "captured" });
        if (grid.length > 0) return;
      }
    })(),
  ]);
}

/** Import a folder into the catalog, wait for workers, then browse it. */
export async function importAndBrowse(root: string): Promise<void> {
  browseBusy.set(true);
  const latch = await importDoneLatch();
  try {
    const total = await importFolder(root);
    await waitForImport(root, total, latch);
    await refreshFolders();
    const catalogRoot = resolveCatalogRoot(root, folders.get());
    await loadFolder(catalogRoot);
  } finally {
    latch.cancel();
    browseBusy.set(false);
  }
}

/** Remember a File → Open photo as its own sidebar row (filename only). */
export async function pinOpenedFile(path: string): Promise<void> {
  const listed = folders.get();
  if (listed.some((f) => sameFolderPath(f.root, path))) return;
  if (listed.some((f) => !isLibraryFileRoot(f) && isPathInFolder(path, f.root))) return;
  try {
    const latch = await importDoneLatch();
    try {
      const total = await importSelected(path, [path]);
      await waitForImport(path, total, latch);
    } finally {
      latch.cancel();
    }
    await refreshFolders();
  } catch {
    /* opening the file still succeeded */
  }
}

/** Import specific files as their own library items (filename on the sidebar). */
export async function importAndBrowseSelected(paths: string[]): Promise<string | null> {
  if (paths.length === 0) return null;
  browseBusy.set(true);
  try {
    for (const file of paths) {
      const latch = await importDoneLatch();
      try {
        const total = await importSelected(file, [file]);
        await waitForImport(file, total, latch);
      } finally {
        latch.cancel();
      }
    }
    await refreshFolders();
    const last = paths[paths.length - 1];
    const catalogRoot = resolveCatalogRoot(last, folders.get());
    await loadFolder(catalogRoot);
    if (paths.length === 1) {
      await openLibraryFile(paths[0]);
    }
    return catalogRoot;
  } finally {
    browseBusy.set(false);
  }
}

/** Native folder picker → import → browse. */
export async function pickAndImportFolder(): Promise<string | null> {
  const dir = await pickFolder();
  if (!dir) return null;
  await importAndBrowse(dir);
  return dir;
}

/** Native file picker (one or more photos/clips) → import → browse. */
export async function pickAndImportPhotos(): Promise<string | null> {
  const kind = workspace.get() === "video" ? "video" : "photo";
  const files = await pickFiles(kind);
  if (!files?.length) return null;
  return importAndBrowseSelected(files);
}

/** Keep the open folder's grid live as import workers upsert assets. */
export function initBrowseBridge(): () => void {
  let cancelled = false;
  let un: (() => void) | undefined;
  void onCatalogChanged(() => {
    if (cancelled) return;
    void refreshFolders().then(() => {
      if (cancelled) return;
      const root = currentFolder.get();
      if (root) return loadFolder(root);
    });
  }).then((u) => {
    if (cancelled) u();
    else un = u;
  });
  return () => {
    cancelled = true;
    un?.();
  };
}

/** Apply rating/flag/label/keywords and refresh the open folder grid. */
export async function patchPhotoMeta(
  ids: number[],
  patch: MetaPatch,
): Promise<void> {
  await setAssetMeta(ids, patch);
  const root = currentFolder.get();
  if (root) await loadFolder(root);
  else await refreshFolders();
}

/** Open a catalog photo in the develop engine. */
export async function openPhoto(item: GridItem): Promise<void> {
  const fastThumb = item.hasThumb ? thumbUrl(item.id, "t") : null;
  const catalogPreview = item.hasThumb ? thumbUrl(item.id, "p") : null;
  openingPreviewUrl.set(fastThumb);

  // The filmstrip thumbnail is normally already in WebKit's cache, so it
  // appears immediately. Upgrade it to the larger catalog preview in the
  // background while the RAW decoder starts.
  if (fastThumb && catalogPreview) {
    const probe = new Image();
    probe.onload = () => {
      if (openingPreviewUrl.get() === fastThumb) openingPreviewUrl.set(catalogPreview);
    };
    probe.src = catalogPreview;
  }

  await openLibraryFile(item.path, item.docId, fastThumb);
}

export async function openLibraryFile(
  path: string,
  docId?: string | null,
  previewUrl?: string | null,
): Promise<void> {
  const { openPath } = await import("../lib/engine/boot");
  await openPath(path, docId, previewUrl);
}

/** Pin a discovered folder as a permanent sidebar shortcut and index it. */
export async function addFolderShortcut(root: string): Promise<void> {
  await importAndBrowse(root);
}

/** Pin several folders, wait for the first batch to land, then browse. */
export async function addFolderShortcuts(roots: string[]): Promise<void> {
  if (!roots.length) return;
  browseBusy.set(true);
  const latch = await importDoneLatch();
  try {
    for (const root of roots) {
      await importFolder(root);
    }
    await Promise.race([
      latch.done.catch(() => undefined),
      (async () => {
        for (let i = 0; i < 120; i++) {
          await new Promise((r) => setTimeout(r, 250));
          const listed = await listFolders();
          if (listed.some((f) => roots.some((root) => sameFolderPath(f.root, root)))) return;
        }
      })(),
    ]);
    await refreshFolders();
    if (!currentFolder.get()) {
      const listed = folders.get();
      const first = roots
        .map((root) => listed.find((f) => sameFolderPath(f.root, root)))
        .find(Boolean);
      await loadFolder(first?.root ?? listed[0]?.root ?? null);
    }
  } finally {
    latch.cancel();
    browseBusy.set(false);
  }
}

/** Remove a sidebar shortcut. Files on disk are not deleted. */
export async function removeFolderShortcut(root: string): Promise<void> {
  const current = currentFolder.get();
  const leaving = Boolean(current && isPathInFolder(current, root));
  if (leaving) currentFolder.set(null);
  await forgetFolder(root);
  setFolderKind(root, "auto");
  await refreshFolders();
  if (leaving) {
    const next = folders.get()[0]?.root ?? null;
    await loadFolder(next);
  }
}
