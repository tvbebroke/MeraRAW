// Main develop preview — same transport as the filmstrip: frame:// JPEG in an
// <img>. Raw RGBA fetch remains available for the navigator + selftest.
import { useCallback, useEffect, useRef, useState } from "react";
import {
  applyOp,
  reportFrontendStatus,
  requestFrame,
  wbFromPoint,
} from "../ipc/commands";
import { onFrameReady } from "../ipc/events";
import { useDocStore } from "../state/docStore";
import { useUiStore } from "../state/uiStore";

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
  const lastOpenedPath = useUiStore((s) => s.lastOpenedPath);
  const setZoomLabel = useUiStore((s) => s.setZoomLabel);

  const view = useRef<ViewState>({ scale: null, centerX: 0.5, centerY: 0.5 });
  const shownVer = useRef(0);
  const pendingVer = useRef(0);
  const inFlight = useRef(false);
  const pending = useRef(false);
  const dragging = useRef<{ x: number; y: number } | null>(null);
  const effScale = useRef(1);
  const brushPoints = useRef<[number, number][]>([]);

  const updateZoomLabel = useCallback(() => {
    const dims = useUiStore.getState().imageDims;
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
      const url = frameUrl(version, "jpeg");
      const probe = new Image();
      probe.onload = () => {
        if (pendingVer.current !== version) return;
        shownVer.current = version;
        setDisplaySrc(url);
        setError(null);
        requestAnimationFrame(() => updateZoomLabel());
      };
      probe.onerror = () => {
        if (pendingVer.current === version) setError("frame transport failed");
      };
      probe.src = url;
    },
    [updateZoomLabel],
  );

  const refresh = useCallback(async () => {
    if (useUiStore.getState().decodeState !== "ready") return;
    const wrap = wrapRef.current;
    if (!wrap) return;
    const m = measureWrap(wrap);
    if (!m) return;
    if (inFlight.current) {
      pending.current = true;
      return;
    }
    inFlight.current = true;
    try {
      const v = view.current;
      if (!isCustomView(v) && shownVer.current > 0) {
        updateZoomLabel();
        return;
      }
      const info = await requestFrame({
        outW: m.outW,
        outH: m.outH,
        scale: v.scale,
        centerX: v.centerX,
        centerY: v.centerY,
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
  }, [lastOpenedPath]);

  useEffect(() => {
    const un = onFrameReady((version) => showFrame(version));
    return () => {
      un.then((f) => f());
    };
  }, [showFrame]);

  useEffect(() => {
    const probe = new Image();
    probe.onload = () => reportFrontendStatus("frame-transport-ok");
    probe.onerror = () => {
      setError("frame transport failed");
      reportFrontendStatus("frame-transport-failed").catch(() => {});
    };
    probe.src = frameUrl(0, "jpeg");
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
      const ui = useUiStore.getState();
      if (!ui.imageDims || !wrapRef.current) return null;
      const rect = wrapRef.current.getBoundingClientRect();
      const dpr = window.devicePixelRatio;
      const px = (e.clientX - rect.left) * dpr;
      const py = (e.clientY - rect.top) * dpr;
      const s = effScale.current;
      const v = view.current;
      const nx =
        (v.centerX * ui.imageDims.w + (px - (rect.width * dpr) / 2) / s) /
        ui.imageDims.w;
      const ny =
        (v.centerY * ui.imageDims.h + (py - (rect.height * dpr) / 2) / s) /
        ui.imageDims.h;
      return [nx, ny];
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
      if (ui.tool === "wb" && ui.imageDims && wrapRef.current) {
        const rect = wrapRef.current.getBoundingClientRect();
        const dpr = window.devicePixelRatio;
        const px = (e.clientX - rect.left) * dpr;
        const py = (e.clientY - rect.top) * dpr;
        const outW = rect.width * dpr;
        const outH = rect.height * dpr;
        const s = effScale.current;
        const v = view.current;
        const imgX = v.centerX * ui.imageDims.w + (px - outW / 2) / s;
        const imgY = v.centerY * ui.imageDims.h + (py - outH / 2) / s;
        const nx = imgX / ui.imageDims.w;
        const ny = imgY / ui.imageDims.h;
        if (nx >= 0 && nx <= 1 && ny >= 0 && ny <= 1) {
          wbFromPoint(nx, ny)
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
      if (ui.tool === "brush" && brushPoints.current.length > 0) {
        const p = toImageCoords(e);
        if (p) brushPoints.current.push(p);
        return;
      }
      if (!dragging.current || !imageOpen || !imageDims) return;
      const dpr = window.devicePixelRatio;
      const dx = (e.clientX - dragging.current.x) * dpr;
      const dy = (e.clientY - dragging.current.y) * dpr;
      dragging.current = { x: e.clientX, y: e.clientY };
      const s = effScale.current;
      view.current.centerX -= dx / s / imageDims.w;
      view.current.centerY -= dy / s / imageDims.h;
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
      className="viewport-wrap"
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
      {error && (
        <div style={{ position: "absolute", color: "var(--error)" }}>
          {error}
        </div>
      )}
    </div>
  );
}
