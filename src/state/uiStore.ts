// UI-only state. Never holds pixels or canonical edit state.
import { create } from "zustand";
import type { RightToolTab } from "../components/lr/RightRail";
import { CROP_OVERLAY_CYCLE, type CropOverlayKind } from "../crop/cropConstants";
import type { CropParams } from "../crop/cropMath";
import { readCropFromDoc } from "../crop/cropMath";
import { applyCropParams } from "../crop/cropActions";
import {
  applyViewportBgAttr,
  isSolidViewportBg,
  persistViewportBg,
  readStoredViewportBg,
  syncWindowBackdrop,
  type ViewportBg,
} from "../theme/viewportBackground";
import {
  applyUiShellAttr,
  persistUiShell,
  readStoredUiShell,
  type UiShell,
} from "../theme/uiShell";
import { useDocStore } from "./docStore";

export type { ViewportBg, UiShell };

interface UiState {
  engineReady: boolean;
  gpuAdapter: string | null;
  statusMessage: string;
  lastOpenedPath: string | null;
  imageOpen: boolean;
  imageDims: { w: number; h: number } | null;
  decodeState: "idle" | "preview" | "ready" | "error";
  zoomLabel: string;
  tool: "pan" | "wb" | "brush" | "crop";
  setTool: (t: "pan" | "wb" | "brush" | "crop") => void;
  /** Crop tool session (overlay on full image). */
  cropActive: boolean;
  cropSnapshot: CropParams | null;
  cropAspectPreset: string;
  cropAspectLocked: boolean;
  cropOverlay: CropOverlayKind;
  cropOverlayVisible: boolean;
  cropOverlayVariant: number;
  cropLightsOut: boolean;
  /** Guide engine settings (shared subsystem — plan P4). */
  cropGuideMode: "auto" | "always" | "never";
  cropGuideColor: string;
  cropGuideOpacity: number;
  cropGridSize: number;
  cropAspectPreviewRatios: string[];
  /** Crop-mask (dimmed region) opacity preference (plan P5). */
  cropMaskOpacity: number;
  /** Crop-mask fill color (plan P5). */
  cropMaskColor: string;
  /** Last applied aspect ratio (w,h) for Shift+A "previous ratio". */
  cropPreviousAspect: { w: number; h: number } | null;
  /** Remembered custom ratios, most recent first (max 5 — plan P2). */
  cropCustomRatios: string[];
  /** PPI for the physical-print-size readout (plan P5); 0 = hidden. */
  cropPpi: number;
  enterCropTool: () => void;
  exitCropTool: (apply: boolean) => void;
  setCropAspectPreset: (id: string) => void;
  setCropAspectLocked: (on: boolean) => void;
  cycleCropOverlay: (reverse?: boolean) => void;
  rotateCropOverlay: () => void;
  toggleCropOverlayVisible: () => void;
  toggleCropLightsOut: () => void;
  setCropGuideMode: (m: "auto" | "always" | "never") => void;
  setCropGuideColor: (c: string) => void;
  setCropGuideOpacity: (o: number) => void;
  setCropGridSize: (n: number) => void;
  setCropAspectPreviewRatios: (r: string[]) => void;
  setCropMaskOpacity: (o: number) => void;
  setCropMaskColor: (c: string) => void;
  setCropPreviousAspect: (w: number, h: number) => void;
  rememberCustomRatio: (r: string) => void;
  setCropPpi: (ppi: number) => void;
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
  settingsOpen: boolean;
  setSettingsOpen: (on: boolean) => void;
  /** Develop preview backdrop (behind the image). */
  viewportBg: ViewportBg;
  setViewportBg: (id: ViewportBg) => void;
  /** App chrome: Modern (Figma) vs Faithful (Lightroom Classic / v0.1.4). */
  uiShell: UiShell;
  setUiShell: (shell: UiShell) => void;
  earlySupporterOpen: boolean;
  setEarlySupporterOpen: (on: boolean) => void;
  isEarlySupporter: boolean;
  setIsEarlySupporter: (v: boolean) => void;
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

// crop preferences persisted per-machine (not per-image — sidecar owns those)
function cropPref<T>(key: string, def: T): T {
  try {
    const raw = localStorage.getItem(`meraraw.crop.${key}`);
    return raw === null ? def : (JSON.parse(raw) as T);
  } catch {
    return def;
  }
}
function saveCropPref(key: string, value: unknown) {
  try {
    localStorage.setItem(`meraraw.crop.${key}`, JSON.stringify(value));
  } catch {
    // best-effort
  }
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
  cropActive: false,
  cropSnapshot: null,
  // Original is the user-expected default ratio (plan §4.5 / RapidRAW lesson)
  cropAspectPreset: "original",
  cropAspectLocked: false,
  cropOverlay: cropPref<CropOverlayKind>("overlay", "thirds"),
  cropOverlayVisible: true,
  cropOverlayVariant: 0,
  cropLightsOut: false,
  cropGuideMode: cropPref<"auto" | "always" | "never">("guideMode", "always"),
  cropGuideColor: cropPref("guideColor", "#ffffff"),
  cropGuideOpacity: cropPref("guideOpacity", 0.55),
  cropGridSize: cropPref("gridSize", 6),
  cropAspectPreviewRatios: cropPref("aspectPreview", ["1:1", "4:5", "16:9"]),
  cropMaskOpacity: cropPref("maskOpacity", 0.5),
  cropMaskColor: cropPref("maskColor", "#000000"),
  cropPreviousAspect: cropPref<{ w: number; h: number } | null>("previousAspect", null),
  cropCustomRatios: cropPref<string[]>("customRatios", []),
  cropPpi: cropPref("ppi", 0),
  enterCropTool: () => {
    const doc = useDocStore.getState().doc;
    const snap = readCropFromDoc(doc?.modules);
    set({
      cropActive: true,
      cropSnapshot: snap,
      tool: "crop",
      rightRailTab: "crop",
      cropOverlayVisible: true,
    });
  },
  exitCropTool: (apply) => {
    const snap = get().cropSnapshot;
    if (!apply && snap) {
      void applyCropParams(snap).then((delta) => useDocStore.getState().reconcile(delta));
    }
    set({
      cropActive: false,
      cropSnapshot: null,
      tool: "pan",
      cropLightsOut: false,
    });
  },
  setCropAspectPreset: (id) => set({ cropAspectPreset: id }),
  setCropAspectLocked: (on) => set({ cropAspectLocked: on }),
  cycleCropOverlay: (reverse) => {
    const kinds = CROP_OVERLAY_CYCLE;
    const cur = get().cropOverlay;
    const idx = kinds.indexOf(cur === "none" ? "thirds" : cur);
    const next =
      kinds[(idx + (reverse ? -1 : 1) + kinds.length) % kinds.length] ?? "thirds";
    saveCropPref("overlay", next);
    set({ cropOverlay: next, cropOverlayVisible: true });
  },
  rotateCropOverlay: () =>
    set((s) => ({ cropOverlayVariant: (s.cropOverlayVariant + 1) % 8 })),
  toggleCropOverlayVisible: () =>
    set((s) => ({ cropOverlayVisible: !s.cropOverlayVisible })),
  toggleCropLightsOut: () => set((s) => ({ cropLightsOut: !s.cropLightsOut })),
  setCropGuideMode: (m) => {
    saveCropPref("guideMode", m);
    set({ cropGuideMode: m });
  },
  setCropGuideColor: (c) => {
    saveCropPref("guideColor", c);
    set({ cropGuideColor: c });
  },
  setCropGuideOpacity: (o) => {
    const v = Math.min(1, Math.max(0.05, o));
    saveCropPref("guideOpacity", v);
    set({ cropGuideOpacity: v });
  },
  setCropGridSize: (n) => {
    const v = Math.min(20, Math.max(2, Math.round(n)));
    saveCropPref("gridSize", v);
    set({ cropGridSize: v });
  },
  setCropAspectPreviewRatios: (r) => {
    const v = r.slice(0, 4);
    saveCropPref("aspectPreview", v);
    set({ cropAspectPreviewRatios: v });
  },
  setCropMaskOpacity: (o) => {
    const v = Math.min(0.95, Math.max(0.1, o));
    saveCropPref("maskOpacity", v);
    set({ cropMaskOpacity: v });
  },
  setCropMaskColor: (c) => {
    saveCropPref("maskColor", c);
    set({ cropMaskColor: c });
  },
  setCropPreviousAspect: (w, h) => {
    if (w <= 0 || h <= 0) return;
    const v = { w, h };
    saveCropPref("previousAspect", v);
    set({ cropPreviousAspect: v });
  },
  rememberCustomRatio: (r) => {
    const cur = get().cropCustomRatios.filter((x) => x !== r);
    const next = [r, ...cur].slice(0, 5);
    saveCropPref("customRatios", next);
    set({ cropCustomRatios: next });
  },
  setCropPpi: (ppi) => {
    const v = Math.max(0, Math.min(1200, Math.round(ppi)));
    saveCropPref("ppi", v);
    set({ cropPpi: v });
  },
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
  settingsOpen: false,
  setSettingsOpen: (on) => set({ settingsOpen: on }),
  viewportBg: (() => {
    const shell = readStoredUiShell();
    const bg = readStoredViewportBg();
    if (shell === "faithful" && !isSolidViewportBg(bg)) return "black";
    return bg;
  })(),
  setViewportBg: (id) => {
    persistViewportBg(id);
    applyViewportBgAttr(id);
    void syncWindowBackdrop(id);
    set({ viewportBg: id });
  },
  uiShell: readStoredUiShell(),
  setUiShell: (shell) => {
    persistUiShell(shell);
    applyUiShellAttr(shell);
    // Faithful: no Zen; clamp fancy backdrops to a solid color.
    set((s) => {
      const nextBg =
        shell === "faithful" && !isSolidViewportBg(s.viewportBg)
          ? ("black" as ViewportBg)
          : s.viewportBg;
      if (nextBg !== s.viewportBg) {
        persistViewportBg(nextBg);
        applyViewportBgAttr(nextBg);
        void syncWindowBackdrop(nextBg);
      }
      return {
        uiShell: shell,
        viewportBg: nextBg,
        showRightPanel: shell === "faithful" ? true : s.showRightPanel,
      };
    });
  },
  earlySupporterOpen: false,
  setEarlySupporterOpen: (on) => set({ earlySupporterOpen: on }),
  isEarlySupporter: false,
  setIsEarlySupporter: (v) => set({ isEarlySupporter: v }),
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
