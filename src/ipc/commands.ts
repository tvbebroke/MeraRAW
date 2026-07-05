// Typed wrappers over Tauri invoke. Thin — mirror of src-tauri/src/commands.rs.
import { invoke } from "@tauri-apps/api/core";
import type {
  AppInfo,
  BrowseRoot,
  DirEntry,
  DocDelta,
  EditDocMirror,
  EngineStatus,
  ExportSettings,
  FileMeta,
  FrameInfo,
  ImageMeta,
  Op,
  ParamSpec,
  ViewParams,
} from "./types";

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

export function pingEngine(): Promise<EngineStatus> {
  return invoke<EngineStatus>("ping_engine");
}

export function pickFile(): Promise<string | null> {
  return invoke<string | null>("pick_file");
}

export function pickFolder(): Promise<string | null> {
  return invoke<string | null>("pick_folder");
}

export function readFileMeta(path: string): Promise<FileMeta> {
  return invoke<FileMeta>("read_file_meta", { path });
}

export function browseRoots(): Promise<BrowseRoot[]> {
  return invoke<BrowseRoot[]>("browse_roots");
}

export function listDir(path: string): Promise<DirEntry[]> {
  return invoke<DirEntry[]>("list_dir", { path });
}

export function openImage(path: string): Promise<ImageMeta> {
  return invoke<ImageMeta>("open_image", { path });
}

export function requestFrame(view: ViewParams): Promise<FrameInfo> {
  return invoke<FrameInfo>("request_frame", { view });
}

export function getMetadata(): Promise<ImageMeta | null> {
  return invoke<ImageMeta | null>("get_metadata");
}

export function closeImage(): Promise<void> {
  return invoke<void>("close_image");
}

// ---- Phase 2: doc ops ----

export function applyOp(op: Op, live = false): Promise<DocDelta> {
  return invoke<DocDelta>("apply_op", { op, live });
}

export function setParam(
  path: string,
  value: unknown,
  live = false,
): Promise<DocDelta> {
  return applyOp({ op: "set_param", path, value }, live);
}

export function undo(): Promise<DocDelta> {
  return invoke<DocDelta>("undo");
}

export function redo(): Promise<DocDelta> {
  return invoke<DocDelta>("redo");
}

export function getDoc(): Promise<EditDocMirror | null> {
  return invoke<EditDocMirror | null>("get_doc");
}

export function getHistory(): Promise<string[]> {
  return invoke<string[]>("get_history");
}

export function getRegistry(): Promise<ParamSpec[]> {
  return invoke<ParamSpec[]>("get_registry");
}

export function snapshot(name: string): Promise<void> {
  return invoke<void>("snapshot", { name });
}

export function listSnapshots(): Promise<string[]> {
  return invoke<string[]>("list_snapshots");
}

export function restoreSnapshot(name: string): Promise<DocDelta> {
  return invoke<DocDelta>("restore_snapshot", { name });
}

export function virtualCopy(): Promise<string> {
  return invoke<string>("virtual_copy");
}

export function switchDoc(docId: string): Promise<DocDelta> {
  return invoke<DocDelta>("switch_doc", { docId });
}

export function savePreset(modules: string[]): Promise<unknown> {
  return invoke<unknown>("save_preset", { modules });
}

export function getStats(): Promise<import("./types").FrameStats | null> {
  return invoke("get_stats");
}

/** WB eyedropper: neutralize the sampled normalized image point. */
export function wbFromPoint(x: number, y: number): Promise<DocDelta> {
  return invoke<DocDelta>("wb_from_point", { x, y });
}

// ---- Phase 5: catalog ----

export function importFolder(path: string): Promise<number> {
  return invoke<number>("import_folder", { path });
}

export function scanImportFolder(
  path: string,
): Promise<import("./types").ImportCandidate[]> {
  return invoke("scan_import_folder", { path });
}

export function importSelected(root: string, paths: string[]): Promise<number> {
  return invoke<number>("import_selected", { root, paths });
}

export function getAssetDetail(
  id: number,
): Promise<import("./types").AssetDetail | null> {
  return invoke("get_asset_detail", { id });
}

export function listAlbums(): Promise<import("./types").AlbumItem[]> {
  return invoke("list_albums");
}

export function createAlbum(name: string): Promise<number> {
  return invoke<number>("create_album", { name });
}

export function deleteAlbum(id: number): Promise<void> {
  return invoke<void>("delete_album", { id });
}

export function addToAlbum(albumId: number, assetIds: number[]): Promise<void> {
  return invoke<void>("add_to_album", { albumId, assetIds });
}

export function removeFromAlbum(
  albumId: number,
  assetIds: number[],
): Promise<void> {
  return invoke<void>("remove_from_album", { albumId, assetIds });
}

export function setCameraProfile(
  profileFile: string,
): Promise<ImageMeta> {
  return invoke<ImageMeta>("set_camera_profile", { profileFile });
}

/** Native picker for a 3D look LUT (.cube). */
export function pickLut(): Promise<string | null> {
  return invoke<string | null>("pick_lut");
}

/** Load (path) or clear (null) the current image's 3D look LUT. */
export function setLut(path: string | null): Promise<void> {
  return invoke<void>("set_lut", { path });
}

/** Change the demosaic algorithm and re-decode the current RAW. */
export function setDemosaic(algo: string): Promise<ImageMeta> {
  return invoke<ImageMeta>("set_demosaic", { algo });
}

export function getGrid(
  query: import("./types").GridQuery,
): Promise<import("./types").GridItem[]> {
  return invoke("get_grid", { query });
}

export function listFolders(): Promise<import("./types").FolderItem[]> {
  return invoke("list_folders");
}

export function setAssetMeta(
  ids: number[],
  patch: import("./types").MetaPatch,
): Promise<void> {
  return invoke("set_asset_meta", { ids, patch });
}

export function rebuildIndex(): Promise<number> {
  return invoke<number>("rebuild_index");
}

/** Toggle viewport mask overlay (null = off). */
export function setMaskOverlay(id: string | null): Promise<void> {
  return invoke<void>("set_mask_overlay", { id });
}

/** Before/after: render the un-edited base while on. */
export function setPreviewBypass(on: boolean): Promise<void> {
  return invoke<void>("set_preview_bypass", { on });
}

/** Display look: 0 = Neutral, 1 = Camera, 2 = Filmic (AgX). */
export function setDisplayLook(look: number): Promise<void> {
  return invoke<void>("set_display_look", { look });
}

// ---- Phase 7: export + presets ----

export function exportImage(settings: ExportSettings): Promise<string> {
  return invoke<string>("export_image", { settings });
}

export function revealInFinder(path: string): Promise<void> {
  return invoke<void>("reveal_in_finder", { path });
}

export function listPresets(): Promise<string[]> {
  return invoke<string[]>("list_presets");
}

export interface PresetCatalogEntry {
  id: string;
  label: string;
  tags: string[];
}

/** Preset style categories (searchable in Presets panel). */
export const PRESET_TAGS = [
  "Natural",
  "Bright & Airy",
  "Moody",
  "Cinematic",
  "Film",
  "Vintage",
  "Matte",
  "Warm",
  "Cool",
  "Vibrant",
  "Muted",
  "Pastel",
  "High Contrast",
  "Black & White",
  "Dark",
  "Dreamy",
  "Editorial",
  "Clean",
  "Earthy",
  "Teal & Orange",
] as const;

export function listPresetCatalog(): Promise<PresetCatalogEntry[]> {
  return invoke<PresetCatalogEntry[]>("list_preset_catalog");
}

export function applyPreset(name: string): Promise<DocDelta> {
  return invoke<DocDelta>("apply_preset", { name });
}

/** Save the listed modules' current params as a named preset (P7). */
export function savePresetNamed(name: string, modules: string[]): Promise<void> {
  return invoke<void>("save_preset", { name, modules });
}

/** Batch a set of param changes as ONE undoable history step (apply_preset). */
export function applyParamBatch(
  modules: Record<string, Record<string, number>>,
): Promise<DocDelta> {
  return applyOp({ op: "apply_preset", preset: { modules } });
}

/** Dev/test hook: returns MERATECH_OPEN env path if set. */
export function autoopenPath(): Promise<string | null> {
  return invoke<string | null>("autoopen_path");
}

/** Intentional-error probe: backend always returns AppError. DoD item 7. */
export function failOnPurpose(): Promise<void> {
  return invoke<void>("fail_on_purpose");
}

/** Frontend self-report → Rust log (lets headless test runs verify the webview booted + frame path). */
export function reportFrontendStatus(status: string): Promise<void> {
  return invoke<void>("report_frontend_status", { status });
}
