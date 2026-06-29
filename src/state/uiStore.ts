// UI-only state. Never holds pixels or canonical edit state.
import { create } from "zustand";
import type { RightToolTab } from "../components/lr/RightRail";

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
  viewCmd: "fit" | "oneToOne" | "zoomIn" | "zoomOut" | null;
  viewCmdNonce: number;
  sendViewCmd: (c: "fit" | "oneToOne" | "zoomIn" | "zoomOut") => void;
  beforeAfter: boolean;
  setBeforeAfter: (b: boolean) => void;
  /** Develop right-rail tool tab (Presets / Edit / Crop / …). */
  rightRailTab: RightToolTab;
  setRightRailTab: (t: RightToolTab) => void;
  /** Panel chrome visibility (F6–F8, Tab, Shift+Tab). */
  showLeftPanel: boolean;
  showRightPanel: boolean;
  showToolbar: boolean;
  showFilmstrip: boolean;
  showTopbar: boolean;
  toggleLeftPanel: () => void;
  toggleRightPanel: () => void;
  toggleSidePanels: () => void;
  toggleToolbar: () => void;
  toggleFilmstrip: () => void;
  toggleAllPanels: () => void;
  /** Ctrl+/ shortcut help overlay. */
  helpOverlay: boolean;
  setHelpOverlay: (on: boolean) => void;
  /** Develop clipping overlay (J). */
  clippingVisible: boolean;
  toggleClipping: () => void;
  /** On-image info overlay cycle (I / Cmd+I). */
  infoOverlay: number;
  cycleInfoOverlay: () => void;
  toggleInfoOverlay: () => void;
  toggleFullscreen: () => void;
  setEngineReady: (adapter: string | null) => void;
  setStatus: (msg: string) => void;
  setLastOpenedPath: (p: string | null) => void;
  setImage: (dims: { w: number; h: number } | null) => void;
  setDecodeState: (s: UiState["decodeState"]) => void;
  setZoomLabel: (z: string) => void;
}

export const useUiStore = create<UiState>((set, get) => ({
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
  rightRailTab: "edit",
  setRightRailTab: (t) => set({ rightRailTab: t }),
  showLeftPanel: true,
  showRightPanel: true,
  showToolbar: true,
  showFilmstrip: true,
  showTopbar: true,
  toggleLeftPanel: () => set((s) => ({ showLeftPanel: !s.showLeftPanel })),
  toggleRightPanel: () => set((s) => ({ showRightPanel: !s.showRightPanel })),
  toggleSidePanels: () =>
    set((s) => ({
      showLeftPanel: !(s.showLeftPanel && s.showRightPanel),
      showRightPanel: !(s.showLeftPanel && s.showRightPanel),
    })),
  toggleToolbar: () => set((s) => ({ showToolbar: !s.showToolbar })),
  toggleFilmstrip: () => set((s) => ({ showFilmstrip: !s.showFilmstrip })),
  toggleAllPanels: () => {
    const any =
      get().showLeftPanel ||
      get().showRightPanel ||
      get().showToolbar ||
      get().showFilmstrip ||
      get().showTopbar;
    set({
      showLeftPanel: !any,
      showRightPanel: !any,
      showToolbar: !any,
      showFilmstrip: !any,
      showTopbar: !any,
    });
  },
  helpOverlay: false,
  setHelpOverlay: (on) => set({ helpOverlay: on }),
  clippingVisible: false,
  toggleClipping: () => set((s) => ({ clippingVisible: !s.clippingVisible })),
  infoOverlay: 0,
  cycleInfoOverlay: () => set((s) => ({ infoOverlay: (s.infoOverlay + 1) % 3 })),
  toggleInfoOverlay: () =>
    set((s) => ({ infoOverlay: s.infoOverlay === 0 ? 1 : 0 })),
  toggleFullscreen: () => {
    if (!document.fullscreenElement) {
      void document.documentElement.requestFullscreen?.();
    } else {
      void document.exitFullscreen?.();
    }
  },
  setEngineReady: (adapter) =>
    set({ engineReady: true, gpuAdapter: adapter, statusMessage: "engine ready" }),
  setStatus: (msg) => set({ statusMessage: msg }),
  setLastOpenedPath: (p) => set({ lastOpenedPath: p }),
  setImage: (dims) => set({ imageDims: dims, imageOpen: dims !== null }),
  setDecodeState: (s) => set({ decodeState: s }),
  setZoomLabel: (z) => set({ zoomLabel: z }),
}));
