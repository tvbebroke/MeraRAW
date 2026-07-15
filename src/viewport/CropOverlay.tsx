import { useCallback, useEffect, useRef, useState } from "react";
import { applyCropParams, resetCropModule } from "../crop/cropActions";
import { copyCropToClipboard, readCropClipboard } from "../crop/cropClipboard";
import {
  constrainRectToImage,
  contentDims,
  dragHandle,
  imageNormToScreen,
  panCrop,
  readCropFromDoc,
  screenToImageNorm,
  straightenFromLine,
  type CropParams,
  type HandleId,
} from "../crop/cropMath";
import { renderGuide } from "../crop/guides";
import { useDocStore } from "../state/docStore";
import { useUiStore } from "../state/uiStore";

type ViewRef = {
  scale: number | null;
  centerX: number;
  centerY: number;
};

const HANDLES: { id: HandleId; x: number; y: number; cursor: string }[] = [
  { id: "nw", x: 0, y: 0, cursor: "nwse-resize" },
  { id: "n", x: 0.5, y: 0, cursor: "ns-resize" },
  { id: "ne", x: 1, y: 0, cursor: "nesw-resize" },
  { id: "e", x: 1, y: 0.5, cursor: "ew-resize" },
  { id: "se", x: 1, y: 1, cursor: "nwse-resize" },
  { id: "s", x: 0.5, y: 1, cursor: "ns-resize" },
  { id: "sw", x: 0, y: 1, cursor: "nesw-resize" },
  { id: "w", x: 0, y: 0.5, cursor: "ew-resize" },
];

type DragState =
  | {
      mode: "handle";
      handle: HandleId;
      start: CropParams;
      startNorm: [number, number];
      mods: { alt: boolean; shift: boolean };
    }
  | { mode: "pan"; start: CropParams; startNorm: [number, number] }
  | {
      mode: "rotate";
      start: CropParams;
      origin: [number, number];
      startNorm: [number, number];
    }
  | { mode: "straighten"; start: CropParams; a: [number, number] };

export function CropOverlay({
  wrapRef,
  viewRef,
  effScaleRef,
  onRefresh,
}: {
  wrapRef: React.RefObject<HTMLDivElement | null>;
  viewRef: React.RefObject<ViewRef>;
  effScaleRef: React.RefObject<number>;
  onRefresh: () => void;
}) {
  const cropActive = useUiStore((s) => s.cropActive);
  const overlayKind = useUiStore((s) => s.cropOverlay);
  const overlayVisible = useUiStore((s) => s.cropOverlayVisible);
  const overlayVariant = useUiStore((s) => s.cropOverlayVariant);
  const guideMode = useUiStore((s) => s.cropGuideMode);
  const guideColor = useUiStore((s) => s.cropGuideColor);
  const guideOpacity = useUiStore((s) => s.cropGuideOpacity);
  const gridSize = useUiStore((s) => s.cropGridSize);
  const aspectPreviewRatios = useUiStore((s) => s.cropAspectPreviewRatios);
  const maskOpacity = useUiStore((s) => s.cropMaskOpacity);
  const maskColor = useUiStore((s) => s.cropMaskColor);
  const lightsOut = useUiStore((s) => s.cropLightsOut);
  const imageDims = useUiStore((s) => s.imageDims);
  const ppi = useUiStore((s) => s.cropPpi);
  const doc = useDocStore((s) => s.doc);
  const reconcile = useDocStore((s) => s.reconcile);

  const [draft, setDraft] = useState<CropParams | null>(null);
  const [dragging, setDragging] = useState<DragState["mode"] | null>(null);
  const [ruler, setRuler] = useState<[[number, number], [number, number]] | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const drag = useRef<DragState | null>(null);
  const draftRef = useRef<CropParams | null>(null);

  const crop = draft ?? readCropFromDoc(doc?.modules);

  useEffect(() => {
    if (!cropActive) {
      setDraft(null);
      draftRef.current = null;
      setDragging(null);
      setRuler(null);
      setMenu(null);
    }
  }, [cropActive]);

  const measure = useCallback(() => {
    const wrap = wrapRef.current;
    const dims = imageDims;
    const view = viewRef.current;
    const scale = effScaleRef.current ?? 1;
    if (!wrap || !dims || !view) return null;
    const dpr = window.devicePixelRatio || 1;
    // While the crop tool is open the viewport shows the ROTATED full image
    // (extract mode 2) — all mapping runs against those content dims.
    const c = readCropFromDoc(useDocStore.getState().doc?.modules);
    const [cw, ch] = contentDims(c, dims.w, dims.h, 2);
    return {
      outW: wrap.clientWidth * dpr,
      outH: wrap.clientHeight * dpr,
      layoutW: wrap.clientWidth,
      layoutH: wrap.clientHeight,
      dpr,
      cw,
      ch,
      imgW: dims.w,
      imgH: dims.h,
      view: { scale, centerX: view.centerX, centerY: view.centerY },
    };
  }, [wrapRef, viewRef, effScaleRef, imageDims]);

  const commit = useCallback(
    (next: CropParams, live = false) => {
      const prev = draftRef.current ?? readCropFromDoc(useDocStore.getState().doc?.modules);
      draftRef.current = next;
      setDraft(next);
      void applyCropParams(next, live).then(reconcile);
      // Rect-only edits while the crop tool is open are UI overlay only —
      // skip the GPU frame request (RapidRAW lesson: 60 fps drag).
      const geomChanged =
        Math.abs(next.angle - prev.angle) > 1e-4 ||
        next.rotate90 !== prev.rotate90 ||
        next.flipH !== prev.flipH ||
        next.flipV !== prev.flipV;
      if (!live || geomChanged) onRefresh();
    },
    [reconcile, onRefresh],
  );

  const constrain = useCallback(
    (p: CropParams): CropParams => {
      if (!p.constrainCrop || !imageDims) return p;
      return { ...p, rect: constrainRectToImage(p.rect, p, imageDims.w, imageDims.h) };
    },
    [imageDims],
  );

  const pointerNorm = (e: React.PointerEvent, m: NonNullable<ReturnType<typeof measure>>) => {
    const rect = wrapRef.current!.getBoundingClientRect();
    const px = (e.clientX - rect.left) * m.dpr;
    const py = (e.clientY - rect.top) * m.dpr;
    return screenToImageNorm(px, py, m.view, m.cw, m.ch, m.outW, m.outH);
  };

  const onPointerDown = (e: React.PointerEvent) => {
    if (!cropActive || !imageDims || e.button === 2) return;
    e.stopPropagation();
    setMenu(null);
    (e.target as Element).setPointerCapture(e.pointerId);
    const m = measure();
    if (!m) return;
    const startNorm = pointerNorm(e, m);
    const t = e.target as HTMLElement;
    const handle = t.dataset.handle as HandleId | undefined;
    let next: DragState;
    if (handle) {
      next = {
        mode: "handle",
        handle,
        start: crop,
        startNorm,
        mods: { alt: e.altKey, shift: e.shiftKey },
      };
    } else if (e.metaKey || e.ctrlKey) {
      next = { mode: "straighten", start: crop, a: startNorm };
    } else if (t.dataset.cropPan !== undefined) {
      next = { mode: "pan", start: crop, startNorm };
    } else {
      next = {
        mode: "rotate",
        start: crop,
        origin: [
          (crop.rect.left + crop.rect.right) * 0.5,
          (crop.rect.top + crop.rect.bottom) * 0.5,
        ],
        startNorm,
      };
    }
    drag.current = next;
    setDragging(next.mode);
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current;
    const m = measure();
    if (!d || !m) return;
    const [nx, ny] = pointerNorm(e, m);

    if (d.mode === "handle") {
      const dx = nx - d.startNorm[0];
      const dy = ny - d.startNorm[1];
      const ratio =
        d.start.aspectW > 0 && d.start.aspectH > 0
          ? d.start.aspectW / d.start.aspectH
          : ((d.start.rect.right - d.start.rect.left) * m.cw) /
            Math.max((d.start.rect.bottom - d.start.rect.top) * m.ch, 1);
      let rect = dragHandle(d.start.rect, d.handle, dx, dy, {
        imgW: m.cw,
        imgH: m.ch,
        aspectLocked: d.start.aspectLocked,
        aspectRatio: ratio,
        fromCenter: d.mods.alt,
        tempAspect: d.mods.shift,
      });
      let next = { ...d.start, rect };
      if (Math.abs(d.start.angle) > 0.001) next = constrain(next);
      commit(next, true);
    } else if (d.mode === "pan") {
      const dx = nx - d.startNorm[0];
      const dy = ny - d.startNorm[1];
      commit(constrain({ ...d.start, rect: panCrop(d.start.rect, dx, dy) }), true);
    } else if (d.mode === "rotate") {
      const a0 = Math.atan2(d.startNorm[1] - d.origin[1], d.startNorm[0] - d.origin[0]);
      const a1 = Math.atan2(ny - d.origin[1], nx - d.origin[0]);
      const delta = ((a1 - a0) * 180) / Math.PI;
      const angle = Math.max(-45, Math.min(45, d.start.angle + delta));
      commit(constrain({ ...d.start, angle }), true);
    } else if (d.mode === "straighten") {
      const angle = straightenFromLine(d.a, [nx, ny], d.start.angle);
      setRuler([d.a, [nx, ny]]);
      commit(constrain({ ...d.start, angle }), true);
    }
  };

  const onPointerUp = () => {
    if (drag.current) {
      drag.current = null;
      setDragging(null);
      setRuler(null);
      // final non-live commit → one undo step per gesture
      if (draft) commit(draft, false);
      setDraft(null);
    }
  };

  const onContextMenu = (e: React.MouseEvent) => {
    if (!cropActive) return;
    e.preventDefault();
    e.stopPropagation();
    // macOS fires contextmenu on ctrl+click, which is the straighten gesture
    if (drag.current) return;
    const rect = wrapRef.current?.getBoundingClientRect();
    if (!rect) return;
    setMenu({ x: e.clientX - rect.left, y: e.clientY - rect.top });
  };

  if (!cropActive || !imageDims) return null;

  const m = measure();
  if (!m) return null;

  const [x1, y1] = imageNormToScreen(
    crop.rect.left,
    crop.rect.top,
    m.view,
    m.cw,
    m.ch,
    m.outW,
    m.outH,
  );
  const [x2, y2] = imageNormToScreen(
    crop.rect.right,
    crop.rect.bottom,
    m.view,
    m.cw,
    m.ch,
    m.outW,
    m.outH,
  );
  const left = Math.min(x1, x2) / m.dpr;
  const top = Math.min(y1, y2) / m.dpr;
  const width = Math.abs(x2 - x1) / m.dpr;
  const height = Math.abs(y2 - y1) / m.dpr;

  const rotating = dragging === "rotate" || dragging === "straighten";
  const guideShown =
    guideMode !== "never" &&
    overlayVisible &&
    (guideMode === "always" || dragging !== null);

  // HUD: px dims + ratio (+ optional print size) while resizing, angle while rotating
  const pxW = Math.round((crop.rect.right - crop.rect.left) * m.cw);
  const pxH = Math.round((crop.rect.bottom - crop.rect.top) * m.ch);
  const mp = (pxW * pxH) / 1_000_000;
  const g = gcd(pxW, pxH);
  const ratioLabel =
    crop.aspectW > 0 && crop.aspectH > 0
      ? `${trimNum(crop.aspectW)}:${trimNum(crop.aspectH)}`
      : g > 0 && pxW / g < 50 && pxH / g < 50
        ? `${pxW / g}:${pxH / g}`
        : (pxW / Math.max(pxH, 1)).toFixed(2);
  let hud: string | null = null;
  if (rotating) {
    hud = `${crop.angle.toFixed(2)}°`;
  } else if (dragging) {
    hud = `${pxW} × ${pxH} px · ${ratioLabel} · ${mp.toFixed(1)} MP`;
    if (ppi > 0) {
      hud += `  ·  @${ppi} PPI → ${(pxW / ppi).toFixed(1)} × ${(pxH / ppi).toFixed(1)} in`;
    }
  }

  const dim = lightsOut ? 0.94 : maskOpacity;
  const maskRgb = hexToRgb(maskColor);
  const rulerPx =
    ruler &&
    ([
      imageNormToScreen(ruler[0][0], ruler[0][1], m.view, m.cw, m.ch, m.outW, m.outH),
      imageNormToScreen(ruler[1][0], ruler[1][1], m.view, m.cw, m.ch, m.outW, m.outH),
    ] as const);

  const menuAction = (fn: () => void) => () => {
    setMenu(null);
    fn();
  };

  return (
    <div
      className={`crop-overlay${lightsOut ? " crop-overlay--lights-out" : ""}`}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerLeave={onPointerUp}
      onContextMenu={onContextMenu}
    >
      <svg className="crop-overlay-svg" width={m.layoutW} height={m.layoutH}>
        <defs>
          <mask id="crop-dim-mask">
            <rect width="100%" height="100%" fill="white" />
            <rect x={left} y={top} width={width} height={height} fill="black" />
          </mask>
        </defs>
        <rect
          width="100%"
          height="100%"
          className="crop-dim"
          style={{ fill: `rgba(${maskRgb}, ${dim})` }}
          mask="url(#crop-dim-mask)"
          onPointerDown={onPointerDown}
        />
        <rect
          x={left}
          y={top}
          width={width}
          height={height}
          className="crop-frame"
          data-crop-pan=""
          onPointerDown={onPointerDown}
        />
        {(guideShown || rotating) && (
          <g
            className="crop-guides"
            style={{
              stroke: guideColor,
              opacity: rotating ? Math.max(guideOpacity, 0.5) : guideOpacity,
              fill: "none",
              pointerEvents: "none",
              strokeWidth: 1,
            }}
          >
            {renderGuide(rotating ? "grid" : overlayKind, {
              x: left,
              y: top,
              w: width,
              h: height,
              variant: overlayVariant,
              gridSize: rotating ? 10 : gridSize,
              aspectRatios: aspectPreviewRatios,
            })}
          </g>
        )}
        {rulerPx && (
          <line
            x1={rulerPx[0][0] / m.dpr}
            y1={rulerPx[0][1] / m.dpr}
            x2={rulerPx[1][0] / m.dpr}
            y2={rulerPx[1][1] / m.dpr}
            style={{ stroke: guideColor, strokeWidth: 1.5, strokeDasharray: "6 4" }}
          />
        )}
        {HANDLES.map((h) => (
          <rect
            key={h.id}
            data-handle={h.id}
            x={left + width * h.x - 5}
            y={top + height * h.y - 5}
            width={10}
            height={10}
            className="crop-handle"
            style={{ cursor: h.cursor }}
            onPointerDown={onPointerDown}
          />
        ))}
      </svg>
      {hud && (
        <div
          style={{
            position: "absolute",
            left: left + width / 2,
            top: top + height / 2,
            transform: "translate(-50%, -50%)",
            background: "rgba(0, 0, 0, 0.65)",
            color: "#fff",
            font: "12px/1.6 -apple-system, system-ui, sans-serif",
            padding: "3px 10px",
            borderRadius: 4,
            pointerEvents: "none",
            whiteSpace: "nowrap",
          }}
        >
          {hud}
        </div>
      )}
      {menu && (
        <div
          style={{
            position: "absolute",
            left: menu.x,
            top: menu.y,
            zIndex: 30,
            background: "var(--panel-bg, #222)",
            border: "1px solid rgba(255,255,255,0.15)",
            borderRadius: 6,
            padding: 4,
            minWidth: 180,
            boxShadow: "0 6px 24px rgba(0,0,0,0.5)",
            font: "12px -apple-system, system-ui, sans-serif",
            color: "var(--text, #ddd)",
          }}
          onPointerDown={(e) => e.stopPropagation()}
        >
          {[
            {
              label: "Reset crop",
              fn: () => void resetCropModule().then(reconcile),
            },
            {
              label: "Crop as shot",
              fn: () =>
                commit({
                  ...crop,
                  rect: { left: 0, top: 0, right: 1, bottom: 1 },
                  angle: 0,
                }),
            },
            {
              label: crop.aspectLocked ? "Unlock aspect ratio" : "Lock aspect ratio",
              fn: () => commit({ ...crop, aspectLocked: !crop.aspectLocked }),
            },
            { label: "Copy crop", fn: () => copyCropToClipboard(crop) },
            {
              label: "Paste crop",
              fn: () => {
                const p = readCropClipboard();
                if (p) commit(constrain({ ...p, constrainCrop: crop.constrainCrop }));
              },
            },
          ].map((item) => (
            <div
              key={item.label}
              onClick={menuAction(item.fn)}
              style={{ padding: "5px 10px", cursor: "default", borderRadius: 4 }}
              onMouseEnter={(e) =>
                ((e.target as HTMLElement).style.background = "rgba(255,255,255,0.1)")
              }
              onMouseLeave={(e) =>
                ((e.target as HTMLElement).style.background = "transparent")
              }
            >
              {item.label}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function gcd(a: number, b: number): number {
  let x = Math.abs(Math.round(a));
  let y = Math.abs(Math.round(b));
  while (y) [x, y] = [y, x % y];
  return x;
}

function trimNum(n: number): string {
  return Number.isInteger(n) ? String(n) : n.toFixed(2).replace(/0+$/, "").replace(/\.$/, "");
}

/** "#rrggbb" → "r, g, b" for rgba() fill. */
function hexToRgb(hex: string): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return "0, 0, 0";
  const n = parseInt(m[1], 16);
  return `${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}`;
}
