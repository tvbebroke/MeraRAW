import { useCallback, useEffect, useRef, useState } from "react";
import { applyCropParams } from "../crop/cropActions";
import type { CropOverlayKind } from "../crop/cropConstants";
import {
  angleFromDrag,
  dragHandle,
  imageNormToScreen,
  panCrop,
  readCropFromDoc,
  screenToImageNorm,
  straightenFromLine,
  type CropParams,
  type HandleId,
} from "../crop/cropMath";
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

function GuideLines({
  kind,
  variant,
  x,
  y,
  w,
  h,
}: {
  kind: CropOverlayKind;
  variant: number;
  x: number;
  y: number;
  w: number;
  h: number;
}) {
  if (kind === "none") return null;
  const lines: JSX.Element[] = [];
  const push = (x1: number, y1: number, x2: number, y2: number, key: string) =>
    lines.push(
      <line key={key} x1={x1} y1={y1} x2={x2} y2={y2} className="crop-guide-line" />,
    );

  if (kind === "grid" || kind === "thirds") {
    const n = kind === "grid" ? 6 : 3;
    for (let i = 1; i < n; i++) {
      const t = i / n;
      push(x + w * t, y, x + w * t, y + h, `v${i}`);
      push(x, y + h * t, x + w, y + h * t, `h${i}`);
    }
  } else if (kind === "diagonal") {
    push(x, y, x + w, y + h, "d1");
    push(x + w, y, x, y + h, "d2");
  } else if (kind === "triangle") {
    const flip = variant % 2 === 1;
    if (flip) {
      push(x, y, x + w, y + h, "t1");
      push(x + w, y, x, y + h, "t2");
    } else {
      push(x, y + h, x + w, y, "t1");
      push(x, y, x + w, y + h, "t2");
    }
  } else if (kind === "golden") {
    const phi = 0.618;
    push(x + w * phi, y, x + w * phi, y + h, "gv");
    push(x + w * (1 - phi), y, x + w * (1 - phi), y + h, "gv2");
    push(x, y + h * phi, x + w, y + h * phi, "gh");
    push(x, y + h * (1 - phi), x + w, y + h * (1 - phi), "gh2");
  } else if (kind === "spiral") {
    const ox = variant % 4;
    const cx = ox === 0 || ox === 3 ? x : x + w;
    const cy = ox <= 1 ? y : y + h;
    lines.push(
      <path
        key="spiral"
        className="crop-guide-spiral"
        d={`M ${cx} ${cy} Q ${x + w * 0.5} ${y} ${x + w} ${y + h * 0.5} T ${x + w * 0.2} ${y + h}`}
        fill="none"
      />,
    );
  } else if (kind === "aspects") {
    const ratios = [
      [1, 1],
      [4, 5],
      [16, 9],
    ];
    ratios.forEach(([rw, rh], i) => {
      const r = rw / rh;
      let bw = w * 0.85;
      let bh = bw / r;
      if (bh > h * 0.85) {
        bh = h * 0.85;
        bw = bh * r;
      }
      const bx = x + (w - bw) / 2;
      const by = y + (h - bh) / 2;
      lines.push(
        <rect
          key={`ar${i}`}
          x={bx}
          y={by}
          width={bw}
          height={bh}
          className="crop-guide-aspect"
        />,
      );
    });
  }
  return <g>{lines}</g>;
}

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
  const lightsOut = useUiStore((s) => s.cropLightsOut);
  const imageDims = useUiStore((s) => s.imageDims);
  const doc = useDocStore((s) => s.doc);
  const reconcile = useDocStore((s) => s.reconcile);

  const [draft, setDraft] = useState<CropParams | null>(null);
  const drag = useRef<
    | {
        mode: "handle";
        handle: HandleId;
        start: CropParams;
        startNorm: [number, number];
        mods: { alt: boolean; shift: boolean };
      }
    | { mode: "pan"; start: CropParams; startNorm: [number, number] }
    | { mode: "rotate"; start: CropParams; origin: [number, number]; startNorm: [number, number] }
    | { mode: "straighten"; start: CropParams; a: [number, number] }
    | null
  >(null);

  const crop = draft ?? readCropFromDoc(doc?.modules);

  useEffect(() => {
    if (!cropActive) setDraft(null);
  }, [cropActive]);

  const measure = useCallback(() => {
    const wrap = wrapRef.current;
    const dims = imageDims;
    const view = viewRef.current;
    const scale = effScaleRef.current ?? 1;
    if (!wrap || !dims || !view) return null;
    const dpr = window.devicePixelRatio || 1;
    return {
      outW: wrap.clientWidth * dpr,
      outH: wrap.clientHeight * dpr,
      layoutW: wrap.clientWidth,
      layoutH: wrap.clientHeight,
      dpr,
      imgW: dims.w,
      imgH: dims.h,
      view: { scale, centerX: view.centerX, centerY: view.centerY },
    };
  }, [wrapRef, viewRef, effScaleRef, imageDims]);

  const normDelta = (d: NonNullable<typeof drag.current>, nx: number, ny: number) => {
    if (d.mode === "straighten") return { dx: 0, dy: 0 };
    const dx = nx - d.startNorm[0];
    const dy = ny - d.startNorm[1];
    return { dx, dy };
  };

  const commit = useCallback(
    (next: CropParams, live = false) => {
      setDraft(next);
      void applyCropParams(next, live).then(reconcile);
      onRefresh();
    },
    [reconcile, onRefresh],
  );

  const onPointerDown = (e: React.PointerEvent) => {
    if (!cropActive || !imageDims) return;
    e.stopPropagation();
    (e.target as Element).setPointerCapture(e.pointerId);
    const m = measure();
    if (!m) return;
    const rect = wrapRef.current?.getBoundingClientRect();
    if (!rect) return;
    const px = (e.clientX - rect.left) * m.dpr;
    const py = (e.clientY - rect.top) * m.dpr;
    const startNorm = screenToImageNorm(px, py, m.view, m.imgW, m.imgH, m.outW, m.outH);
    const t = e.target as HTMLElement;
    const handle = t.dataset.handle as HandleId | undefined;
    if (handle) {
      drag.current = {
        mode: "handle",
        handle,
        start: crop,
        startNorm,
        mods: { alt: e.altKey, shift: e.shiftKey },
      };
      return;
    }
    if (t.dataset.cropPan !== undefined) {
      drag.current = { mode: "pan", start: crop, startNorm };
      return;
    }
    if (e.metaKey || e.ctrlKey) {
      drag.current = { mode: "straighten", start: crop, a: startNorm };
      return;
    }
    drag.current = {
      mode: "rotate",
      start: crop,
      origin: [(crop.rect.left + crop.rect.right) * 0.5, (crop.rect.top + crop.rect.bottom) * 0.5],
      startNorm,
    };
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current;
    const m = measure();
    if (!d || !m) return;
    const rect = wrapRef.current?.getBoundingClientRect();
    if (!rect) return;
    const px = (e.clientX - rect.left) * m.dpr;
    const py = (e.clientY - rect.top) * m.dpr;
    const [nx, ny] = screenToImageNorm(px, py, m.view, m.imgW, m.imgH, m.outW, m.outH);
    const { dx, dy } = normDelta(d, nx, ny);

    if (d.mode === "handle") {
      const ratio =
        d.start.aspectW > 0 && d.start.aspectH > 0
          ? d.start.aspectW / d.start.aspectH
          : ((d.start.rect.right - d.start.rect.left) * m.imgW) /
            Math.max((d.start.rect.bottom - d.start.rect.top) * m.imgH, 1);
      const next = dragHandle(d.start.rect, d.handle, dx, dy, {
        imgW: m.imgW,
        imgH: m.imgH,
        aspectLocked: d.start.aspectLocked,
        aspectRatio: ratio,
        fromCenter: d.mods.alt,
        tempAspect: d.mods.shift,
      });
      commit({ ...d.start, rect: next }, true);
    } else if (d.mode === "pan") {
      commit({ ...d.start, rect: panCrop(d.start.rect, dx, dy) }, true);
    } else if (d.mode === "rotate") {
      const delta = angleFromDrag(d.startNorm, [nx, ny], d.origin);
      const angle = Math.max(-45, Math.min(45, d.start.angle + delta));
      commit({ ...d.start, angle }, true);
    } else if (d.mode === "straighten") {
      const angle = straightenFromLine(d.a, [nx, ny], d.start.angle);
      commit({ ...d.start, angle }, true);
    }
  };

  const onPointerUp = () => {
    if (drag.current) {
      drag.current = null;
      setDraft(null);
    }
  };

  if (!cropActive || !imageDims) return null;

  const m = measure();
  if (!m) return null;

  const [x1, y1] = imageNormToScreen(
    crop.rect.left,
    crop.rect.top,
    m.view,
    m.imgW,
    m.imgH,
    m.outW,
    m.outH,
  );
  const [x2, y2] = imageNormToScreen(
    crop.rect.right,
    crop.rect.bottom,
    m.view,
    m.imgW,
    m.imgH,
    m.outW,
    m.outH,
  );
  const left = Math.min(x1, x2) / m.dpr;
  const top = Math.min(y1, y2) / m.dpr;
  const width = Math.abs(x2 - x1) / m.dpr;
  const height = Math.abs(y2 - y1) / m.dpr;

  const dimClass = lightsOut ? "crop-overlay crop-overlay--lights-out" : "crop-overlay";

  return (
    <div
      className={dimClass}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerLeave={onPointerUp}
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
          mask="url(#crop-dim-mask)"
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
        {overlayVisible && (
          <GuideLines
            kind={overlayKind}
            variant={overlayVariant}
            x={left}
            y={top}
            w={width}
            h={height}
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
    </div>
  );
}
