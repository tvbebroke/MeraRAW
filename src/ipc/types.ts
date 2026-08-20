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

export interface DocRef {
  docId: string;
  active: boolean;
}

export interface FileMeta {
  path: string;
  exists: boolean;
  isDir: boolean;
  size: number;
  modifiedMs: number | null;
  ext: string | null;
}

export interface ImageMeta {
  path: string;
  /** "raw" = sensor data; "rendered" = already-processed (JPEG/PNG/…). */
  kind: "raw" | "rendered" | "video";
  /** Uppercase format tag, e.g. "ARW", "JPEG", "PNG". */
  format: string;
  /** Source bit depth per channel (8/16); 0 = unknown (RAW). */
  bitDepth: number;
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
  /** Demosaic algorithm in effect (e.g. "rcd"). Empty for non-RAW. */
  demosaic: string;
  /** Algorithms currently usable (sidecar entries absent when their worker
   *  binary is missing). Omitted (empty) for non-RAW sources. */
  availableDemosaic?: string[];
  /** GPS decimal degrees when present in source EXIF. */
  gpsLat?: number | null;
  gpsLon?: number | null;
  /** Rendered-file input color space label (e.g. "Display P3", "sRGB"). */
  inputColorSpace?: string | null;
  video?: {
    fps: number;
    frameCount: number;
    durationS: number;
    frame: number;
    inFrame: number;
    outFrame: number;
    inputTransform: string;
  } | null;
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
  blend?: string;
  source: { type: string } & Record<string, unknown>;
  modules?: Record<string, Record<string, unknown>>;
}

/** Object-removal / heal spot (phase 10). */
export interface RetouchSpotMirror {
  id: string;
  enabled: boolean;
  feather: number;
  source: { type: string } & Record<string, unknown>;
}

/** Mirror of the canonical EditDoc (loosely typed; engine is master). */
export interface EditDocMirror {
  schema_version: number;
  doc_id: string;
  source_ref: { path: string };
  modules?: Record<string, Record<string, unknown>>;
  masks?: MaskMirror[];
  retouch?: RetouchSpotMirror[];
  meta?: Record<string, unknown>;
}

export interface DocDelta {
  doc: EditDocMirror;
  label: string;
  undoDepth: number;
  redoDepth: number;
  newMaskId?: string;
  newRetouchId?: string;
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
      blend?: string;
    }
  | { op: "set_mask_source"; id: string; source: unknown }
  | { op: "add_retouch_spot"; source: unknown }
  | { op: "remove_retouch_spot"; id: string }
  | { op: "set_retouch_source"; id: string; source: unknown }
  | {
      op: "refine_retouch_spot";
      id: string;
      feather?: number;
      enabled?: boolean;
    }
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
  /** Sidecar doc id; virtual copies share `path` with the master. */
  docId?: string;
}

export interface FolderItem {
  root: string;
  name: string;
  photoCount: number;
  videoCount: number;
  accessible: boolean;
}

export interface DiscoveredFolder {
  path: string;
  name: string;
  photoCount: number;
  videoCount: number;
}

export interface FolderChild {
  name: string;
  path: string;
  isDir: boolean;
  kind: "photo" | "video" | null;
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
  waveform?: number[];
  waveformW?: number;
  waveformH?: number;
  vectorscope?: number[];
  vectorscopeSize?: number;
  /** RGB parade waveform: 3 planes packed R then G then B. */
  parade?: number[];
}

export type MetadataPolicy = "preserve" | "stripGps" | "stripAll";

export interface ExportSettings {
  format: "jpeg" | "png" | "tiff16" | "heic";
  target: "srgb" | "display-p3" | "adobe-rgb" | "prophoto";
  quality: number;
  maxDim: number | null;
  sharpen: number;
  destDir: string;
  metadataPolicy?: MetadataPolicy;
  /** Legacy: when true, forces stripAll. Prefer metadataPolicy. */
  stripMetadata?: boolean;
  copyright?: string | null;
  watermarkText?: string | null;
  videoClip?: boolean;
  outputStem?: string | null;
  videoIn?: number | null;
  videoOut?: number | null;
  /** Copy source audio into the muxed clip when possible. Default true. */
  videoAudio?: boolean;
}

export interface ExportProgress {
  phase: "render" | "encode" | string;
  done: number;
  total: number;
}

export interface ExportBatchProgress {
  index: number;
  count: number;
  path: string;
  phase: "decode" | "render" | "encode" | string;
  done: number;
  total: number;
}

export interface ExportBatchDone {
  ok: string[];
  failed: { path: string; error: string }[];
  cancelled: boolean;
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
