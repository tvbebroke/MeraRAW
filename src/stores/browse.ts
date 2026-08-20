// Catalog-backed browsing state: current folder listing feeds the
// filmstrip + file browser; clicking a photo opens it in the engine.
import { atom, computed } from "nanostores";
import {
  getGrid,
  importFolder,
  listFolders,
  pickFolder,
  setAssetMeta,
} from "../ipc/commands";
import { onCatalogChanged, onImportDone } from "../ipc/events";
import type { FolderItem, GridItem, MetaPatch } from "../ipc/types";
import { customSchemeUrl } from "../lib/engine/customScheme";
import { currentFolder, lastOpenedPath } from "./app";
import { workspace } from "./workspace";
import { isVideoPath } from "../lib/media";

export const folders = atom<FolderItem[]>([]);
export const photos = atom<GridItem[]>([]);
export const browseBusy = atom(false);

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

/** Grid item matching the currently open image (if visible in this workspace). */
export const activePhoto = computed(
  [libraryItems, lastOpenedPath],
  (items, path) => items.find((i) => i.path === path) ?? null,
);

function normPath(p: string): string {
  return p.replace(/\\/g, "/").replace(/^\/\/\?\//, "").toLowerCase();
}

/** Prefer the catalog's remembered root when the picker path differs only by prefix. */
function resolveCatalogRoot(picked: string, listed: FolderItem[]): string {
  const n = normPath(picked);
  const hit = listed.find((f) => {
    const fn = normPath(f.root);
    return fn === n || fn.endsWith(n) || n.endsWith(fn);
  });
  return hit?.root ?? picked;
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
    folders.set((await listFolders()).filter((f) => f.accessible));
  } catch {
    folders.set([]);
  }
}

export async function loadFolder(root: string | null): Promise<void> {
  currentFolder.set(root);
  browseBusy.set(true);
  try {
    const grid = await getGrid({
      folder: root ?? undefined,
      limit: 2000,
      sort: "captured",
    });
    // Keep inaccessible rows visible — filtering them hid Windows imports
    // when Path::exists() disagreed with the stored path form.
    photos.set(grid);
  } catch {
    photos.set([]);
  } finally {
    browseBusy.set(false);
  }
}

/** Import a folder into the catalog, wait for workers, then browse it. */
export async function importAndBrowse(root: string): Promise<void> {
  browseBusy.set(true);
  const latch = await importDoneLatch();
  try {
    const total = await importFolder(root);
    if (total > 0) {
      // Wait for workers, but also poll — covers missed events.
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
    } else {
      latch.cancel();
    }
    await refreshFolders();
    const catalogRoot = resolveCatalogRoot(root, folders.get());
    await loadFolder(catalogRoot);
  } finally {
    latch.cancel();
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

/** Keep the open folder's grid live as import workers upsert assets. */
export function initBrowseBridge(): () => void {
  let cancelled = false;
  let un: (() => void) | undefined;
  void onCatalogChanged(() => {
    if (cancelled) return;
    const root = currentFolder.get();
    void refreshFolders().then(() => {
      if (!cancelled && root) return loadFolder(root);
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
  // Dynamic import avoids a browse ↔ boot cycle at module init.
  const { openPath } = await import("../lib/engine/boot");
  await openPath(item.path);
}

/** The grid item matching the currently open image, if visible. */
export function activeItem(items: GridItem[]): GridItem | null {
  const p = lastOpenedPath.get();
  return items.find((i) => i.path === p) ?? null;
}
