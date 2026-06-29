import type { MetaPatch } from "../ipc/types";
import type { RightToolTab } from "../components/lr/RightRail";

/** App-level callbacks registered by App.tsx */
export interface AppKeyboardContext {
  setMode: (mode: "library" | "develop") => void;
  openPath: (path: string) => void | Promise<void>;
  openFilePicker: () => void | Promise<void>;
  triggerImport: () => void | Promise<void>;
  triggerExport: () => void;
  triggerExportPrevious: () => void;
  getMode: () => "library" | "develop";
  hasImage: () => boolean;
}

/** Library callbacks registered while Library is mounted */
export interface LibraryKeyboardContext {
  patchSelected: (patch: MetaPatch) => Promise<void>;
  advanceSelection: (delta: number) => void;
  extendSelection: (delta: number) => void;
  selectAll: () => void;
  deselectAll: () => void;
  selectOnlyActive: () => void;
  openActive: () => void;
  toggleGridMode: () => void;
  focusSearch: () => void;
  toggleFilterBar: () => void;
  toggleFilters: () => void;
  getSelectedCount: () => number;
  getItemCount: () => number;
  isReviewOpen: () => boolean;
  adjustThumbnailSize: (delta: number) => void;
  jumpGrid: (where: "home" | "end") => void;
  exportSelection: () => void;
  bumpRating: (delta: number) => void;
}

let appCtx: AppKeyboardContext | null = null;
let libraryCtx: LibraryKeyboardContext | null = null;

export function setAppKeyboardContext(ctx: AppKeyboardContext | null) {
  appCtx = ctx;
}

export function getAppKeyboardContext() {
  return appCtx;
}

export function setLibraryKeyboardContext(ctx: LibraryKeyboardContext | null) {
  libraryCtx = ctx;
}

export function getLibraryKeyboardContext() {
  return libraryCtx;
}

export interface FilmstripKeyboardContext {
  advance: (delta: number) => void;
}

let filmstripCtx: FilmstripKeyboardContext | null = null;

export function setFilmstripKeyboardContext(ctx: FilmstripKeyboardContext | null) {
  filmstripCtx = ctx;
}

export function getFilmstripKeyboardContext() {
  return filmstripCtx;
}

export type { RightToolTab };
