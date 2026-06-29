// Event names mirrored from src-tauri/src/events.rs. Keep in sync.
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export const EVENTS = {
  engineReady: "engine-ready",
  log: "log",
  fileOpened: "file-opened",
  folderOpened: "folder-opened",
  previewReady: "preview-ready",
  imageReady: "image-ready",
  frameReady: "frame-ready",
  decodeError: "decode-error",
  docUpdated: "doc-updated",
  maskReady: "mask-ready",
  exportProgress: "export-progress",
  engineCrashed: "engine-crashed",
  exportRequested: "export-requested",
  importRequested: "import-requested",
} as const;

export function onMaskReady(cb: (id: string) => void): Promise<UnlistenFn> {
  return listen<string>(EVENTS.maskReady, (e) => cb(e.payload));
}

export function onImportProgress(
  cb: (p: { done: number; total: number }) => void,
): Promise<UnlistenFn> {
  return listen<{ done: number; total: number }>("import-progress", (e) =>
    cb(e.payload),
  );
}

export function onImportDone(cb: (total: number) => void): Promise<UnlistenFn> {
  return listen<number>("import-done", (e) => cb(e.payload));
}

export function onCatalogChanged(cb: () => void): Promise<UnlistenFn> {
  return listen<null>("catalog-changed", () => cb());
}

export function onPreviewReady(cb: (version: number) => void): Promise<UnlistenFn> {
  return listen<number>(EVENTS.previewReady, (e) => cb(e.payload));
}

export function onImageReady(cb: (version: number) => void): Promise<UnlistenFn> {
  return listen<number>(EVENTS.imageReady, (e) => cb(e.payload));
}

export function onFrameReady(cb: (version: number) => void): Promise<UnlistenFn> {
  return listen<number>(EVENTS.frameReady, (e) => cb(e.payload));
}

export function onDecodeError(cb: (message: string) => void): Promise<UnlistenFn> {
  return listen<string>(EVENTS.decodeError, (e) => cb(e.payload));
}

export function onExportProgress(
  cb: (p: import("./types").ExportProgress) => void,
): Promise<UnlistenFn> {
  return listen<import("./types").ExportProgress>(EVENTS.exportProgress, (e) =>
    cb(e.payload),
  );
}

export function onEngineCrashed(cb: (message: string) => void): Promise<UnlistenFn> {
  return listen<string>(EVENTS.engineCrashed, (e) => cb(e.payload));
}

export function onDocUpdated(
  cb: (delta: import("./types").DocDelta) => void,
): Promise<UnlistenFn> {
  return listen<import("./types").DocDelta>(EVENTS.docUpdated, (e) =>
    cb(e.payload),
  );
}

export interface EngineReadyPayload {
  adapter: string | null;
  gpuReady: boolean;
}

export function onEngineReady(
  cb: (p: EngineReadyPayload) => void,
): Promise<UnlistenFn> {
  return listen<EngineReadyPayload>(EVENTS.engineReady, (e) => cb(e.payload));
}

export function onFileOpened(cb: (path: string) => void): Promise<UnlistenFn> {
  return listen<string>(EVENTS.fileOpened, (e) => cb(e.payload));
}

export function onFolderOpened(
  cb: (path: string) => void,
): Promise<UnlistenFn> {
  return listen<string>(EVENTS.folderOpened, (e) => cb(e.payload));
}

export function onImportRequested(cb: () => void): Promise<UnlistenFn> {
  return listen<null>(EVENTS.importRequested, () => cb());
}

export function onExportRequested(cb: () => void): Promise<UnlistenFn> {
  return listen<null>(EVENTS.exportRequested, () => cb());
}
