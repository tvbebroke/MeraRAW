// Draggable tone-curve editor bound to tone_curve.points (ty:"curve").
// Reads the curve from the doc mirror, renders + lets you drag/add/remove
// control points, commits via setParam (debounced) — one op per edit.
import { useCallback, useEffect, useRef, useState } from "react";
import { setParam } from "../../ipc/commands";
import { useDocStore } from "../../state/docStore";

type Pt = [number, number]; // x,y in [0,1]
const SIZE = 248;
const PAD = 10;

function toCanvas(p: Pt): [number, number] {
  return [PAD + p[0] * (SIZE - 2 * PAD), SIZE - PAD - p[1] * (SIZE - 2 * PAD)];
}
function toNorm(cx: number, cy: number): Pt {
  return [
    Math.min(1, Math.max(0, (cx - PAD) / (SIZE - 2 * PAD))),
    Math.min(1, Math.max(0, (SIZE - PAD - cy) / (SIZE - 2 * PAD))),
  ];
}

/** Monotone-ish Catmull-Rom sampling for the rendered curve line. */
function sampleCurve(pts: Pt[]): Pt[] {
  if (pts.length < 2) return pts;
  const out: Pt[] = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const p0 = pts[Math.max(0, i - 1)];
    const p1 = pts[i];
    const p2 = pts[i + 1];
    const p3 = pts[Math.min(pts.length - 1, i + 2)];
    for (let t = 0; t < 1; t += 0.05) {
      const t2 = t * t;
      const t3 = t2 * t;
      const y =
        0.5 *
        (2 * p1[1] +
          (-p0[1] + p2[1]) * t +
          (2 * p0[1] - 5 * p1[1] + 4 * p2[1] - p3[1]) * t2 +
          (-p0[1] + 3 * p1[1] - 3 * p2[1] + p3[1]) * t3);
      const x = p1[0] + (p2[0] - p1[0]) * t;
      out.push([x, Math.min(1, Math.max(0, y))]);
    }
  }
  out.push(pts[pts.length - 1]);
  return out;
}

export function ToneCurve() {
  const doc = useDocStore((s) => s.doc);
  const reconcile = useDocStore((s) => s.reconcile);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const drag = useRef<number | null>(null);
  const commitTimer = useRef<number | null>(null);

  // points from the doc, defaulting to identity endpoints
  const docPts = (doc?.modules?.tone_curve?.points as Pt[] | undefined) ?? [];
  const [pts, setPts] = useState<Pt[]>(
    docPts.length >= 2 ? docPts : [[0, 0], [1, 1]],
  );

  // reconcile external doc changes (undo/redo/preset/AI) into local points
  useEffect(() => {
    const next = (doc?.modules?.tone_curve?.points as Pt[] | undefined) ?? [];
    if (drag.current === null) {
      setPts(next.length >= 2 ? next : [[0, 0], [1, 1]]);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [doc?.modules?.tone_curve?.points]);

  const commit = useCallback(
    (next: Pt[]) => {
      if (commitTimer.current) window.clearTimeout(commitTimer.current);
      commitTimer.current = window.setTimeout(() => {
        // identity (just endpoints) → send empty = identity
        const isIdentity =
          next.length === 2 &&
          next[0][0] === 0 &&
          next[0][1] === 0 &&
          next[1][0] === 1 &&
          next[1][1] === 1;
        setParam("tone_curve.points", isIdentity ? [] : next)
          .then(reconcile)
          .catch(() => {});
      }, 120);
    },
    [reconcile],
  );

  // draw
  useEffect(() => {
    const c = canvasRef.current;
    if (!c) return;
    const ctx = c.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, SIZE, SIZE);
    // grid
    ctx.strokeStyle = "rgba(255,255,255,0.06)";
    ctx.lineWidth = 1;
    for (let i = 1; i < 4; i++) {
      const g = PAD + (i / 4) * (SIZE - 2 * PAD);
      ctx.beginPath();
      ctx.moveTo(g, PAD);
      ctx.lineTo(g, SIZE - PAD);
      ctx.moveTo(PAD, g);
      ctx.lineTo(SIZE - PAD, g);
      ctx.stroke();
    }
    // diagonal reference
    ctx.strokeStyle = "rgba(255,255,255,0.12)";
    ctx.beginPath();
    const a = toCanvas([0, 0]);
    const b = toCanvas([1, 1]);
    ctx.moveTo(a[0], a[1]);
    ctx.lineTo(b[0], b[1]);
    ctx.stroke();
    // curve
    const curve = sampleCurve(pts);
    ctx.strokeStyle = "#d4d4d4";
    ctx.lineWidth = 1.6;
    ctx.beginPath();
    curve.forEach((p, i) => {
      const [x, y] = toCanvas(p);
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.stroke();
    // points
    pts.forEach((p) => {
      const [x, y] = toCanvas(p);
      ctx.fillStyle = "#e8e8e8";
      ctx.strokeStyle = "#1c1c1c";
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(x, y, 4, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
    });
  }, [pts]);

  function hit(cx: number, cy: number): number | null {
    for (let i = 0; i < pts.length; i++) {
      const [x, y] = toCanvas(pts[i]);
      if (Math.hypot(x - cx, y - cy) < 9) return i;
    }
    return null;
  }

  const onDown = (e: React.PointerEvent) => {
    const r = canvasRef.current!.getBoundingClientRect();
    const cx = ((e.clientX - r.left) / r.width) * SIZE;
    const cy = ((e.clientY - r.top) / r.height) * SIZE;
    let i = hit(cx, cy);
    if (i === null) {
      // add a point at this x
      const np = toNorm(cx, cy);
      const next = [...pts, np].sort((p, q) => p[0] - q[0]);
      i = next.findIndex((p) => p === np);
      setPts(next);
    }
    drag.current = i;
    (e.target as Element).setPointerCapture(e.pointerId);
  };
  const onMove = (e: React.PointerEvent) => {
    if (drag.current === null) return;
    const r = canvasRef.current!.getBoundingClientRect();
    const cx = ((e.clientX - r.left) / r.width) * SIZE;
    const cy = ((e.clientY - r.top) / r.height) * SIZE;
    const np = toNorm(cx, cy);
    const i = drag.current;
    const next = pts.slice();
    // endpoints keep their x pinned; interior points clamp between neighbors
    if (i === 0) np[0] = 0;
    else if (i === next.length - 1) np[0] = 1;
    else {
      np[0] = Math.min(
        next[i + 1][0] - 0.01,
        Math.max(next[i - 1][0] + 0.01, np[0]),
      );
    }
    next[i] = np;
    setPts(next);
    commit(next);
  };
  const onUp = () => {
    drag.current = null;
  };
  const onDouble = (e: React.MouseEvent) => {
    const r = canvasRef.current!.getBoundingClientRect();
    const cx = ((e.clientX - r.left) / r.width) * SIZE;
    const cy = ((e.clientY - r.top) / r.height) * SIZE;
    const i = hit(cx, cy);
    if (i !== null && i !== 0 && i !== pts.length - 1) {
      const next = pts.filter((_, k) => k !== i);
      setPts(next);
      commit(next);
    }
  };

  return (
    <div className="tone-curve">
      <canvas
        ref={canvasRef}
        width={SIZE}
        height={SIZE}
        style={{ width: "100%", aspectRatio: "1", touchAction: "none" }}
        onPointerDown={onDown}
        onPointerMove={onMove}
        onPointerUp={onUp}
        onDoubleClick={onDouble}
      />
      <div className="muted" style={{ fontSize: 10 }}>
        click to add · drag to shape · double-click a point to remove
      </div>
    </div>
  );
}
