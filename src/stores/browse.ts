// Catalog-backed browsing state: current folder listing feeds the
// filmstrip + file browser; clicking a photo opens it in the engine.
import { atom, computed } from "nanostores";
import { getGrid, importFolder, listFolders, pickFolder } from "../ipc/commands";
import type { FolderItem, GridItem } from "../ipc/types";
import { customSchemeUrl } from "../lib/engine/customScheme";
import { currentFolder, lastOpenedPath } from "./app";

export const folders = atom<FolderItem[]>([]);
export const photos = atom<GridItem[]>([]);
export const browseBusy = atom(false);

/** Alias used by shell chrome that still says "folder". */
export const folder = currentFolder;

export function thumbUrl(id: number, tier: "t" | "p" = "t"): string {
  return customSchemeUrl("thumb", `${id}?tier=${tier}`);
}

/** Grid item matching the currently open image (if visible in the strip). */
export const activePhoto = computed(
  [photos, lastOpenedPath],
  (items, path) => items.find((i) => i.path === path) ?? null,
);

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
    photos.set(grid.filter((i) => i.accessible));
  } catch {
    photos.set([]);
  } finally {
    browseBusy.set(false);
  }
}

/** Import a folder into the catalog, then browse it. */
export async function importAndBrowse(root: string): Promise<void> {
  browseBusy.set(true);
  try {
    await importFolder(root);
    await refreshFolders();
    await loadFolder(root);
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
