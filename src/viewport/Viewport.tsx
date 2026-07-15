// Main develop preview — frame:// JPEG in an <img>, same transport as the
// filmstrip (thumb://). Prefetch via a detached Image before swapping src.
import { useCallback, useEffect, useRef, useState } from "react";
import {
  applyOp,
  reportFrontendStatus,
  requestFrame,
  wbFromPoint,
} from "../ipc/commands";
import { onEngineReady, onFrameReady } from "../ipc/events";
import {
  contentDims,
  contentNormToImageNorm,
  cropModeFor,
  readCropFromDoc,
} from "../crop/cropMath";
import { useDocStore } from "../state/docStore";
import { useUiStore } from "../state/uiStore";
import { CropOverlay } from "./CropOverlay";

/**
 * Dims of the displayed content — the engine's pan/zoom space (crop region
 * once a crop is committed, rotated full frame while the crop tool is open).
 */
function viewContentDims(): { w: number; h: number } | null {
  const ui = useUiStore.getState();
  if (!ui.imageDims) return null;
  const crop = readCropFromDoc(useDocStore.getState().doc?.modules);
  const mode = cropModeFor(crop, ui.cropActive);
  const [w, h] = contentDims(crop, ui.imageDims.w, ui.imageDims.h, mode);
  return { w, h };
}

/** Screen-space point → original-image normalized coords (mask/WB space). */
function screenToOriginalNorm(
  clientX: number,
  clientY: number,
  wrap: HTMLElement,
  effScale: number,
  view: { centerX: number; centerY: number },
): [number, number] | null {
  const ui = useUiStore.getState();
  if (!ui.imageDims) return null;
  const crop = readCropFromDoc(useDocStore.getState().doc?.modules);
  const mode = cropModeFor(crop, ui.cropActive);
  const [cw, ch] = contentDims(crop, ui.imageDims.w, ui.imageDims.h, mode);
  const rect = wrap.getBoundingClientRect();
  const dpr = window.devicePixelRatio;
  const px = (clientX - rect.left) * dpr;
  const py = (clientY - rect.top) * dpr;
  const nx = (view.centerX * cw + (px - (rect.width * dpr) / 2) / effScale) / cw;
  const ny = (view.centerY * ch + (py - (rect.height * dpr) / 2) / effScale) / ch;
  return contentNormToImageNorm(nx, ny, crop, ui.imageDims.w, ui.imageDims.h, mode);
}

const FRAME_BASE = "frame://localhost";

function appZoom(): number {
  return (
    parseFloat(
      getComputedStyle(document.documentElement).getPropertyValue("--app-zoom"),
    ) || 1
  );
}

function measureWrap(wrap: HTMLElement) {
  const layoutW = wrap.clientWidth;
  const layoutH = wrap.clientHeight;
  if (layoutW < 8 || layoutH < 8) return null;
  const zoom = appZoom();
  const dpr = window.devicePixelRatio || 1;
  const clamp = (n: number) => Math.min(8192, Math.max(1, Math.round(n)));
  return {
    layoutW,
    layoutH,
    outW: clamp(layoutW * zoom * dpr),
    outH: clamp(layoutH * zoom * dpr),
  };
}

function isCustomView(v: ViewState): boolean {
  return v.scale !== null || v.centerX !== 0.5 || v.centerY !== 0.5;
}

export function frameUrl(version: number, fmt?: "jpeg"): string {
  const q = fmt === "jpeg" ? "&fmt=jpeg" : "";
  return `${FRAME_BASE}/current?v=${version}${q}`;
}

/**
 * Prefetch a frame:// JPEG the same way filmstrip thumbs load thumb:// —
 * via <img>, not fetch. Custom-protocol fetch is cross-origin from the
 * Vite/dev page and used to fail CORS; <img> only needs img-src CSP.
 */
function preloadFrame(version: number): Promise<string> {
  const url = frameUrl(version, "jpeg");
  return new Promise((resolve, reject) => {
    const probe = new Image();
    probe.onload = () => resolve(url);
    probe.onerror = () => reject(new Error("frame image load failed"));
    probe.src = url;
  });
}

async function probeFrameTransport(retries = 5): Promise<boolean> {
  for (let attempt = 0; attempt < retries; attempt++) {
    try {
      await preloadFrame(0);
      return true;
    } catch {
      await new Promise((r) => setTimeout(r, 150 * (attempt + 1)));
    }
  }
  return false;
}

interface ViewState {
  scale: number | null;
  centerX: number;
  centerY: number;
}

export function Viewport() {
  const wrapRef = useRef<HTMLDivElement>(null);
  const [displaySrc, setDisplaySrc] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const imageOpen = useUiStore((s) => s.imageOpen);
  const imageDims = useUiStore((s) => s.imageDims);
  const decodeState = useUiStore((s) => s.decodeState);
  const cropActive = useUiStore((s) => s.cropActive);
  const lastOpenedPath = useUiStore((s) => s.lastOpenedPath);
  const setZoomLabel = useUiStore((s) => s.setZoomLabel);

  const view = useRef<ViewState>({ scale: null, centerX: 0.5, centerY: 0.5 });
  const shownVer = useRef(0);
  const pendingVer = useRef(0);
  const objUrl = useRef<string | null>(null);
  const inFlight = useRef(false);
  const pending = useRef(false);
  const dragging = useRef<{ x: number; y: number } | null>(null);
  const effScale = useRef(1);
  const brushPoints = useRef<[number, number][]>([]);

  const updateZoomLabel = useCallback(() => {
    const dims = viewContentDims();
    const wrap = wrapRef.current;
    const m = wrap && dims ? measureWrap(wrap) : null;
    if (!dims || !m) return;
    const v = view.current;
    const fit = Math.min(m.outW / dims.w, m.outH / dims.h);
    effScale.current = v.scale ?? fit;
    setZoomLabel(v.scale === null ? "fit" : `${Math.round(v.scale * 100)}%`);
  }, [setZoomLabel]);

  const showFrame = useCallback(
    (version: number) => {
      if (version <= shownVer.current) return;
      pendingVer.current = version;
      // Prefetch into a detached Image, then swap <img src> once decoded so
      // the visible frame doesn't flash empty. Same custom-protocol path as
      // filmstrip thumbs (no fetch/CORS).
      preloadFrame(version)
        .then((url) => {
          if (pendingVer.current !== version) return;
          shownVer.current = version;
          const prev = objUrl.current;
          objUrl.current = null;
          setDisplaySrc(url);
          if (prev) URL.revokeObjectURL(prev);
          setError(null);
          requestAnimationFrame(() => updateZoomLabel());
        })
        .catch(() => {
          if (pendingVer.current === version) setError("frame transport failed");
        });
    },
    [updateZoomLabel],
  );

  const forceNext = useRef(false);
  const refresh = useCallback(async (force = false) => {
    if (useUiStore.getState().decodeState !== "ready") return;
    const wrap = wrapRef.current;
    if (!wrap) return;
    const m = measureWrap(wrap);
    if (!m) return;
    if (force) forceNext.current = true;
    if (inFlight.current) {
      pending.current = true;
      return;
    }
    inFlight.current = true;
    try {
      const v = view.current;
      if (!forceNext.current && !isCustomView(v) && shownVer.current > 0) {
        updateZoomLabel();
        return;
      }
      forceNext.current = false;
      const info = await requestFrame({
        outW: m.outW,
        outH: m.outH,
        scale: v.scale,
        centerX: v.centerX,
        centerY: v.centerY,
        cropPreview: useUiStore.getState().cropActive,
      });
      showFrame(info.version);
    } catch {
      // preview phase — ignore
    } finally {
      inFlight.current = false;
      if (pending.current) {
        pending.current = false;
        void refresh();
      }
    }
  }, [showFrame, updateZoomLabel]);

  useEffect(() => {
    view.current = { scale: null, centerX: 0.5, centerY: 0.5 };
    shownVer.current = 0;
    pendingVer.current = 0;
    setDisplaySrc(null);
    if (objUrl.current) {
      URL.revokeObjectURL(objUrl.current);
      objUrl.current = null;
    }
  }, [lastOpenedPath]);

  // Entering/leaving the crop tool changes the content space (full rotated
  // frame vs. crop region): reset to fit and force a re-request so the
  // engine re-renders with the right cropPreview flag.
  const firstCropToggle = useRef(true);
  useEffect(() => {
    if (firstCropToggle.current) {
      firstCropToggle.current = false;
      return;
    }
    view.current = { scale: null, centerX: 0.5, centerY: 0.5 };
    void refresh(true);
  }, [cropActive, refresh]);

  useEffect(() => {
    const un = onFrameReady((version) => showFrame(version));
    return () => {
      un.then((f) => f());
    };
  }, [showFrame]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    const runProbe = async () => {
      const ok = await probeFrameTransport();
      if (cancelled) return;
      if (ok) {
        setError(null);
        reportFrontendStatus("frame-transport-ok").catch(() => {});
      } else {
        setError("frame transport failed");
        reportFrontendStatus("frame-transport-failed").catch(() => {});
      }
    };

    if (useUiStore.getState().engineReady) {
      void runProbe();
    } else {
      onEngineReady(() => {
        if (!cancelled) void runProbe();
      }).then((u) => {
        unlisten = () => {
          u();
        };
      });
    }

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!wrapRef.current) return;
    let t: ReturnType<typeof setTimeout> | undefined;
    const obs = new ResizeObserver(() => {
      if (!imageOpen || decodeState !== "ready") return;
      if (!isCustomView(view.current)) return;
      if (t) clearTimeout(t);
      t = setTimeout(() => void refresh(), 50);
    });
    obs.observe(wrapRef.current);
    return () => {
      obs.disconnect();
      if (t) clearTimeout(t);
    };
  }, [imageOpen, decodeState, refresh]);

  const viewCmdNonce = useUiStore((s) => s.viewCmdNonce);
  useEffect(() => {
    if (viewCmdNonce === 0 || !imageOpen) return;
    const cmd = useUiStore.getState().viewCmd;
    if (cmd === "fit") view.current = { scale: null, centerX: 0.5, centerY: 0.5 };
    else if (cmd === "oneToOne") view.current.scale = 1;
    else if (cmd === "zoomIn")
      view.current.scale = Math.min(8, (effScale.current || 1) * 1.5);
    else if (cmd === "zoomOut")
      view.current.scale = Math.max(0.02, (effScale.current || 1) / 1.5);
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [viewCmdNonce]);

  const onWheel = useCallback(
    (e: React.WheelEvent) => {
      if (!imageOpen || !imageDims) return;
      e.preventDefault();
      const cur = effScale.current;
      const factor = Math.exp(-e.deltaY * 0.0015);
      view.current.scale = Math.min(8, Math.max(0.02, cur * factor));
      void refresh();
    },
    [imageOpen, imageDims, refresh],
  );

  const toImageCoords = useCallback(
    (e: { clientX: number; clientY: number }): [number, number] | null => {
      if (!wrapRef.current) return null;
      return screenToOriginalNorm(
        e.clientX,
        e.clientY,
        wrapRef.current,
        effScale.current,
        view.current,
      );
    },
    [],
  );

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      const ui = useUiStore.getState();
      if (ui.tool === "brush" && ui.selectedMask) {
        const p = toImageCoords(e);
        if (p) brushPoints.current = [p];
        (e.target as Element).setPointerCapture(e.pointerId);
        return;
      }
      if (ui.tool === "crop" && ui.cropActive) return;
      if (ui.tool === "wb" && ui.imageDims && wrapRef.current) {
        const p = screenToOriginalNorm(
          e.clientX,
          e.clientY,
          wrapRef.current,
          effScale.current,
          view.current,
        );
        if (p) {
          wbFromPoint(p[0], p[1])
            .then((delta) => useDocStore.getState().reconcile(delta))
            .catch(() => {});
        }
        ui.setTool("pan");
        return;
      }
      dragging.current = { x: e.clientX, y: e.clientY };
      (e.target as Element).setPointerCapture(e.pointerId);
    },
    [toImageCoords],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      const ui = useUiStore.getState();
      if (ui.tool === "crop" && ui.cropActive) return;
      if (ui.tool === "brush" && brushPoints.current.length > 0) {
        const p = toImageCoords(e);
        if (p) brushPoints.current.push(p);
        return;
      }
      if (!dragging.current || !imageOpen || !imageDims) return;
      const content = viewContentDims();
      if (!content) return;
      const dpr = window.devicePixelRatio;
      const dx = (e.clientX - dragging.current.x) * dpr;
      const dy = (e.clientY - dragging.current.y) * dpr;
      dragging.current = { x: e.clientX, y: e.clientY };
      const s = effScale.current;
      view.current.centerX -= dx / s / content.w;
      view.current.centerY -= dy / s / content.h;
      view.current.centerX = Math.min(1, Math.max(0, view.current.centerX));
      view.current.centerY = Math.min(1, Math.max(0, view.current.centerY));
      void refresh();
    },
    [imageOpen, imageDims, refresh, toImageCoords],
  );

  const onPointerUp = useCallback(() => {
    dragging.current = null;
    const ui = useUiStore.getState();
    if (ui.tool === "brush" && ui.selectedMask && brushPoints.current.length > 0) {
      const pts = brushPoints.current;
      brushPoints.current = [];
      const doc = useDocStore.getState().doc;
      const mask = doc?.masks?.find((m) => m.id === ui.selectedMask);
      if (mask && mask.source.type === "brush") {
        const strokes = Array.isArray(mask.source.strokes)
          ? [...(mask.source.strokes as unknown[])]
          : [];
        const radius = useUiStore.getState().brushRadius;
        strokes.push({ points: pts, radius, hardness: 0.6, mode: "add" });
        applyOp({
          op: "set_mask_source",
          id: mask.id,
          source: { type: "brush", strokes },
        })
          .then((delta) => useDocStore.getState().reconcile(delta))
          .catch(() => {});
      }
    }
  }, []);

  const onDoubleClick = useCallback(() => {
    if (!imageOpen) return;
    if (view.current.scale === null) view.current.scale = 1;
    else view.current = { scale: null, centerX: 0.5, centerY: 0.5 };
    void refresh();
  }, [imageOpen, refresh]);

  return (
    <div
      ref={wrapRef}
      className={`viewport-wrap${cropActive ? " viewport-wrap--crop" : ""}`}
      onWheel={onWheel}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onDoubleClick={onDoubleClick}
    >
      {displaySrc && (
        <img
          src={displaySrc}
          className="viewport-img"
          alt=""
          draggable={false}
          onError={() => setError("frame transport failed")}
        />
      )}
      <CropOverlay
        wrapRef={wrapRef}
        viewRef={view}
        effScaleRef={effScale}
        onRefresh={() => void refresh()}
      />
      {error && (
        <div style={{ position: "absolute", color: "var(--error)" }}>
          {error}
        </div>
      )}
    </div>
  );
}
