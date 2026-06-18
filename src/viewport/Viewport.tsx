// Draws frames served by the Rust core over the frame:// protocol.
// View state (zoom/pan/fit) lives here (uiStore-adjacent); the engine
// renders exactly what we ask for via request_frame.
import { useCallback, useEffect, useRef, useState } from "react";
import {
  applyOp,
  reportFrontendStatus,
  requestFrame,
  wbFromPoint,
} from "../ipc/commands";
import { onFrameReady, onImageReady } from "../ipc/events";
import { useDocStore } from "../state/docStore";
import { useUiStore } from "../state/uiStore";

// macOS/Linux custom-scheme URL form. Windows would be http://frame.localhost/.
const FRAME_BASE = "frame://localhost";

export function frameUrl(version: number): string {
  return `${FRAME_BASE}/current?v=${version}`;
}

interface ViewState {
  /** Output px per image px; null = fit. */
  scale: number | null;
  centerX: number;
  centerY: number;
}

export function Viewport() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const imageOpen = useUiStore((s) => s.imageOpen);
  const imageDims = useUiStore((s) => s.imageDims);
  const setZoomLabel = useUiStore((s) => s.setZoomLabel);

  const view = useRef<ViewState>({ scale: null, centerX: 0.5, centerY: 0.5 });
  const inFlight = useRef(false);
  const pending = useRef(false);
  const dragging = useRef<{ x: number; y: number } | null>(null);
  const effScale = useRef(1); // last effective output-px-per-image-px
  const brushPoints = useRef<[number, number][]>([]);

  const drawVersion = useCallback(async (version: number) => {
    const res = await fetch(frameUrl(version));
    if (!res.ok) throw new Error(`frame fetch ${res.status}`);
    const width = parseInt(res.headers.get("X-Frame-Width") ?? "0", 10);
    const height = parseInt(res.headers.get("X-Frame-Height") ?? "0", 10);
    if (!width || !height) throw new Error("frame missing dimension headers");
    const buf = new Uint8ClampedArray(await res.arrayBuffer());
    if (buf.length !== width * height * 4) {
      throw new Error(`frame size mismatch ${buf.length} vs ${width * height * 4}`);
    }
    const canvas = canvasRef.current;
    if (!canvas) return;
    canvas.width = width;
    canvas.height = height;
    // crisp on retina: CSS size = device px / dpr
    canvas.style.width = `${width / window.devicePixelRatio}px`;
    canvas.style.height = `${height / window.devicePixelRatio}px`;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("no 2d context");
    ctx.putImageData(new ImageData(buf, width, height), 0, 0);
    setError(null);
  }, []);

  /** Ask the engine for a fresh render of the current view, then draw it. */
  const refresh = useCallback(async () => {
    if (!wrapRef.current) return;
    if (inFlight.current) {
      pending.current = true;
      return;
    }
    inFlight.current = true;
    try {
      const dpr = window.devicePixelRatio;
      const rect = wrapRef.current.getBoundingClientRect();
      const v = view.current;
      // clamp request to a sane ceiling (guards transient huge measurements
      // and the GPU texture limit)
      const clamp = (n: number) => Math.min(8192, Math.max(1, Math.round(n)));
      const info = await requestFrame({
        outW: clamp(rect.width * dpr),
        outH: clamp(rect.height * dpr),
        scale: v.scale,
        centerX: v.centerX,
        centerY: v.centerY,
      });
      // track effective scale for pan math + zoom label
      const dims = useUiStore.getState().imageDims;
      if (dims) {
        const fit = Math.min(
          (rect.width * dpr) / dims.w,
          (rect.height * dpr) / dims.h,
        );
        effScale.current = v.scale ?? fit;
        setZoomLabel(v.scale === null ? "fit" : `${Math.round(v.scale * 100)}%`);
      }
      await drawVersion(info.version);
    } catch (e) {
      // NoImage during preview phase is expected; ignore quietly
    } finally {
      inFlight.current = false;
      if (pending.current) {
        pending.current = false;
        void refresh();
      }
    }
  }, [drawVersion, setZoomLabel]);

  // engine-pushed frames (preview swap-in, decode-complete render)
  useEffect(() => {
    const un1 = onFrameReady((version) => {
      drawVersion(version).catch((e) =>
        setError(e instanceof Error ? e.message : String(e)),
      );
    });
    const un2 = onImageReady(() => {
      // full decode landed — re-render at exact viewport size + view
      void refresh();
    });
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
    };
  }, [drawVersion, refresh]);

  // P0 transport proof on mount (test pattern until an image opens)
  useEffect(() => {
    drawVersion(0)
      .then(() => reportFrontendStatus("frame-transport-ok"))
      .catch((e) => {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        reportFrontendStatus(`frame-transport-failed: ${msg}`).catch(() => {});
      });
  }, [drawVersion]);

  // resize → re-render
  useEffect(() => {
    if (!wrapRef.current) return;
    const obs = new ResizeObserver(() => {
      if (imageOpen) void refresh();
    });
    obs.observe(wrapRef.current);
    return () => obs.disconnect();
  }, [imageOpen, refresh]);

  // view-command channel (toolbar / navigator)
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

  // interactions: wheel zoom, drag pan, double-click fit/100%
  const onWheel = useCallback(
    (e: React.WheelEvent) => {
      if (!imageOpen || !imageDims) return;
      e.preventDefault();
      const cur = effScale.current;
      const factor = Math.exp(-e.deltaY * 0.0015);
      const next = Math.min(8, Math.max(0.02, cur * factor));
      view.current.scale = next;
      void refresh();
    },
    [imageOpen, imageDims, refresh],
  );

  /** Map a pointer event → normalized image coords via the current view. */
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
        // map click → normalized image coords using current view math
        const rect = wrapRef.current.getBoundingClientRect();
        const dpr = window.devicePixelRatio;
        const px = (e.clientX - rect.left) * dpr;
        const py = (e.clientY - rect.top) * dpr;
        const outW = rect.width * dpr;
        const outH = rect.height * dpr;
        const s = effScale.current;
        const v = view.current;
        const imgX =
          v.centerX * ui.imageDims.w + (px - outW / 2) / s;
        const imgY =
          v.centerY * ui.imageDims.h + (py - outH / 2) / s;
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
    [],
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
    [imageOpen, imageDims, refresh],
  );

  const onPointerUp = useCallback(() => {
    dragging.current = null;
    const ui = useUiStore.getState();
    if (ui.tool === "brush" && ui.selectedMask && brushPoints.current.length > 0) {
      // append the stroke to the brush mask's source (undoable op)
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
    if (view.current.scale === null) {
      view.current.scale = 1; // 1:1
    } else {
      view.current = { scale: null, centerX: 0.5, centerY: 0.5 }; // fit
    }
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
      <canvas ref={canvasRef} className="viewport-canvas" />
      {error && (
        <div style={{ position: "absolute", color: "var(--error)" }}>
          frame transport failed: {error}
        </div>
      )}
    </div>
  );
}
