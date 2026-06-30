// Shared types mirrored from Rust (src-tauri/core + commands). Keep in sync.

export interface AppInfo {
  version: string;
  build: string;
  gpuAdapter: string | null;
  gpuBackend: string | null;
}

export interface EngineStatus {
  alive: boolean;
  gpuReady: boolean;
  adapter: string | null;
}

export interface FileMeta {
  path: string;
  exists: boolean;
  isDir: boolean;
  size: number;
  modifiedMs: number | null;
  ext: string | null;
}

export interface DirEntry {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
}

export interface BrowseRoot {
  name: string;
  path: string;
}

export interface ImageMeta {
  path: string;
  cameraMake: string;
  cameraModel: string;
  lens: string | null;
  iso: number | null;
  shutter: string | null;
  aperture: number | null;
  focalMm: number | null;
  capturedAt: string | null;
  width: number;
  height: number;
  orientation: string;
  asShotWb: [number, number, number];
  estimatedCct: number | null;
  cameraProfile: string | null;
  availableProfiles: string[];
  availableProfileFiles: string[];
}

export interface ViewParams {
  outW: number;
  outH: number;
  /** Output px per image px. null = fit. */
  scale: number | null;
  centerX: number;
  centerY: number;
  /** When true, render full master for crop-tool overlay editing. */
  cropPreview?: boolean;
}

export interface FrameInfo {
  version: number;
  width: number;
  height: number;
}

/** Mirror of Rust ParamSpec (registry — contract A2). */
export interface ParamSpec {
  path: string;
  ty: "f32" | "bool" | "enum" | "curve" | "color";
  min: number;
  max: number;
  default: unknown;
  enumValues?: string[];
  ui: { label: string; step: number; scale: string; group: string };
}

export interface MaskMirror {
  id: string;
  kind: string;
  opacity: number;
  invert: boolean;
  feather: number;
  source: { type: string } & Record<string, unknown>;
  modules?: Record<string, Record<string, unknown>>;
}

/** Mirror of the canonical EditDoc (loosely typed; engine is master). */
export interface EditDocMirror {
  schema_version: number;
  doc_id: string;
  source_ref: { path: string };
  modules?: Record<string, Record<string, unknown>>;
  masks?: MaskMirror[];
  meta?: Record<string, unknown>;
}

export interface DocDelta {
  doc: EditDocMirror;
  label: string;
  undoDepth: number;
  redoDepth: number;
  newMaskId?: string;
}

export type Op =
  | { op: "set_param"; path: string; value: unknown }
  | { op: "add_mask"; kind: string; source: unknown }
  | { op: "remove_mask"; id: string }
  | {
      op: "refine_mask";
      id: string;
      opacity?: number;
      feather?: number;
      invert?: boolean;
    }
  | { op: "set_mask_source"; id: string; source: unknown }
  | { op: "reset_module"; module: string }
  | { op: "reset_all" }
  | { op: "apply_preset"; preset: { modules: Record<string, Record<string, unknown>> } };

export interface GridItem {
  id: number;
  path: string;
  filename: string;
  width: number;
  height: number;
  rating: number;
  flag: string;
  label: string | null;
  hasEdits: boolean;
  capturedAt: string | null;
  cameraModel: string | null;
  blurScore: number | null;
  hasThumb: boolean;
  accessible: boolean;
}

export interface FolderItem {
  root: string;
  name: string;
  photoCount: number;
  accessible: boolean;
}

export interface GridQuery {
  text?: string;
  ratingMin?: number;
  flag?: string;
  label?: string;
  camera?: string;
  hasEdits?: boolean;
  folder?: string;
  albumId?: number;
  blurryOnly?: boolean;
  dupesOnly?: boolean;
  sort?: string;
  offset?: number;
  limit?: number;
}

export interface ImportCandidate {
  path: string;
  filename: string;
  isNew: boolean;
}

export interface AssetDetail {
  id: number;
  path: string;
  filename: string;
  width: number;
  height: number;
  rating: number;
  flag: string;
  label: string | null;
  hasEdits: boolean;
  capturedAt: string | null;
  importedAt: string | null;
  cameraMake: string | null;
  cameraModel: string | null;
  lens: string | null;
  iso: number | null;
  shutter: string | null;
  aperture: number | null;
  focalMm: number | null;
  keywords: string[];
  albums: string[];
  accessible: boolean;
}

export interface AlbumItem {
  id: number;
  name: string;
  photoCount: number;
}

export interface MetaPatch {
  rating?: number;
  flag?: string;
  label?: string | null;
  addKeyword?: string;
  removeKeyword?: string;
}

export interface FrameStats {
  bins: number;
  r: number[];
  g: number[];
  b: number[];
  luma: number[];
  clipHighPct: number;
  clipLowPct: number;
}

export interface ExportSettings {
  format: "jpeg" | "png" | "tiff16" | "heic";
  target: "srgb" | "display-p3" | "adobe-rgb" | "prophoto";
  quality: number;
  maxDim: number | null;
  sharpen: number;
  destDir: string;
  stripMetadata?: boolean;
  copyright?: string | null;
  watermarkText?: string | null;
}

export interface ExportProgress {
  phase: "render" | "encode" | string;
  done: number;
  total: number;
}

/** Serialized form of Rust AppError: #[serde(tag = "kind", content = "message")] */
export interface AppError {
  kind:
    | "Io"
    | "NotFound"
    | "Engine"
    | "Gpu"
    | "Decode"
    | "InvalidOp"
    | "Internal";
  message: string;
}

export function isAppError(e: unknown): e is AppError {
  return (
    typeof e === "object" &&
    e !== null &&
    "kind" in e &&
    "message" in e
  );
}

/** Human-readable Tauri invoke error (handles AppError + legacy tagged shapes). */
export function formatAppError(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  if (isAppError(e)) return `${e.kind}: ${e.message}`;
  if (e && typeof e === "object") {
    const o = e as Record<string, unknown>;
    if (typeof o.message === "string") return o.message;
    const kind = Object.keys(o)[0];
    if (kind && typeof o[kind] === "string") return `${kind}: ${o[kind]}`;
    try {
      return JSON.stringify(e);
    } catch {
      /* fall through */
    }
  }
  return String(e);
}
