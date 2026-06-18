// UI-only state. Never holds pixels or canonical edit state.
import { create } from "zustand";

interface UiState {
  engineReady: boolean;
  gpuAdapter: string | null;
  statusMessage: string;
  lastOpenedPath: string | null;
  imageOpen: boolean;
  imageDims: { w: number; h: number } | null;
  decodeState: "idle" | "preview" | "ready" | "error";
  zoomLabel: string;
  tool: "pan" | "wb" | "brush";
  setTool: (t: "pan" | "wb" | "brush") => void;
  selectedMask: string | null;
  setSelectedMask: (id: string | null) => void;
  brushRadius: number;
  setBrushRadius: (r: number) => void;
  // view-command channel (toolbar/navigator → viewport). nonce drives effect.
  viewCmd: "fit" | "oneToOne" | "zoomIn" | "zoomOut" | null;
  viewCmdNonce: number;
  sendViewCmd: (c: "fit" | "oneToOne" | "zoomIn" | "zoomOut") => void;
  beforeAfter: boolean;
  setBeforeAfter: (b: boolean) => void;
  setEngineReady: (adapter: string | null) => void;
  setStatus: (msg: string) => void;
  setLastOpenedPath: (p: string | null) => void;
  setImage: (dims: { w: number; h: number } | null) => void;
  setDecodeState: (s: UiState["decodeState"]) => void;
  setZoomLabel: (z: string) => void;
}

export const useUiStore = create<UiState>((set) => ({
  engineReady: false,
  gpuAdapter: null,
  statusMessage: "starting…",
  lastOpenedPath: null,
  imageOpen: false,
  imageDims: null,
  decodeState: "idle",
  zoomLabel: "—",
  tool: "pan",
  setTool: (t) => set({ tool: t }),
  selectedMask: null,
  setSelectedMask: (id) => set({ selectedMask: id }),
  brushRadius: 0.04,
  setBrushRadius: (r) => set({ brushRadius: Math.min(0.2, Math.max(0.01, r)) }),
  viewCmd: null,
  viewCmdNonce: 0,
  sendViewCmd: (c) => set((s) => ({ viewCmd: c, viewCmdNonce: s.viewCmdNonce + 1 })),
  beforeAfter: false,
  setBeforeAfter: (b) => set({ beforeAfter: b }),
  setEngineReady: (adapter) =>
    set({ engineReady: true, gpuAdapter: adapter, statusMessage: "engine ready" }),
  setStatus: (msg) => set({ statusMessage: msg }),
  setLastOpenedPath: (p) => set({ lastOpenedPath: p }),
  setImage: (dims) => set({ imageDims: dims, imageOpen: dims !== null }),
  setDecodeState: (s) => set({ decodeState: s }),
  setZoomLabel: (z) => set({ zoomLabel: z }),
}));
