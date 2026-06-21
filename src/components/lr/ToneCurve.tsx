// Draggable tone-curve editor — RGB luma + per-channel R/G/B curves.
// Right-click point: delete · right-click curve: reset menu · channel tabs.
import { useCallback, useEffect, useRef, useState } from "react";
import { setParam } from "../../ipc/commands";
import { useDocStore } from "../../state/docStore";

type Pt = [number, number];
type Channel = "rgb" | "r" | "g" | "b";

const SIZE = 248;
const PAD = 10;

const CHANNELS: { key: Channel; label: string; color: string; path: string }[] = [
  { key: "rgb", label: "RGB", color: "#d4d4d4", path: "tone_curve.points" },
  { key: "r", label: "R", color: "#e0564f", path: "tone_curve.points_r" },
  { key: "g", label: "G", color: "#6db86d", path: "tone_curve.points_g" },
  { key: "b", label: "B", color: "#5a8fe0", path: "tone_curve.points_b" },
];

const IDENTITY: Pt[] = [
  [0, 0],
  [1, 1],
];

type MenuState =
  | { kind: "point"; x: number; y: number; index: number }
  | { kind: "curve"; x: number; y: number }
  | null;

function toCanvas(p: Pt): [number, number] {
  return [PAD + p[0] * (SIZE - 2 * PAD), SIZE - PAD - p[1] * (SIZE - 2 * PAD)];
}

function toNorm(cx: number, cy: number): Pt {
  return [
    Math.min(1, Math.max(0, (cx - PAD) / (SIZE - 2 * PAD))),
    Math.min(1, Math.max(0, (SIZE - PAD - cy) / (SIZE - 2 * PAD))),
  ];
}

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

function readChannelPts(
  doc: ReturnType<typeof useDocStore.getState>["doc"],
  ch: Channel,
): Pt[] {
  const mod = doc?.modules?.tone_curve as Record<string, unknown> | undefined;
  const key =
    ch === "rgb" ? "points" : ch === "r" ? "points_r" : ch === "g" ? "points_g" : "points_b";
  const raw = mod?.[key] as Pt[] | undefined;
  return raw && raw.length >= 2 ? raw : IDENTITY;
}

function isIdentityPayload(pts: Pt[]): boolean {
  return (
    pts.length === 2 &&
    pts[0][0] === 0 &&
    pts[0][1] === 0 &&
    pts[1][0] === 1 &&
    pts[1][1] === 1
  );
}

function flattenPts(pts: Pt[]): Pt[] {
  if (pts.length <= 2) return [pts[0], pts[pts.length - 1]];
  return [pts[0], pts[pts.length - 1]];
}

export function ToneCurve() {
  const doc = useDocStore((s) => s.doc);
  const reconcile = useDocStore((s) => s.reconcile);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const drag = useRef<number | null>(null);
  const commitTimer = useRef<number | null>(null);
  const [channel, setChannel] = useState<Channel>("rgb");
  const [menu, setMenu] = useState<MenuState>(null);
  const [pts, setPts] = useState<Pt[]>(IDENTITY);

  const active = CHANNELS.find((c) => c.key === channel)!;

  useEffect(() => {
    if (drag.current === null) {
      setPts(readChannelPts(doc, channel));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [doc?.modules?.tone_curve, channel]);

  const commitPath = useCallback(
    (path: string, next: Pt[]) => {
      const payload = isIdentityPayload(next) ? [] : next;
      return setParam(path, payload).then(reconcile).catch(() => {});
    },
    [reconcile],
  );

  const commit = useCallback(
    (next: Pt[]) => {
      if (commitTimer.current) window.clearTimeout(commitTimer.current);
      commitTimer.current = window.setTimeout(() => {
        commitPath(active.path, next);
      }, 120);
    },
    [active.path, commitPath],
  );

  const applyPts = useCallback(
    (next: Pt[]) => {
      setPts(next);
      commit(next);
    },
    [commit],
  );

  useEffect(() => {
    const c = canvasRef.current;
    if (!c) return;
    const ctx = c.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, SIZE, SIZE);
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
    ctx.strokeStyle = "rgba(255,255,255,0.12)";
    ctx.beginPath();
    const a = toCanvas([0, 0]);
    const b = toCanvas([1, 1]);
    ctx.moveTo(a[0], a[1]);
    ctx.lineTo(b[0], b[1]);
    ctx.stroke();
    const curve = sampleCurve(pts);
    ctx.strokeStyle = active.color;
    ctx.lineWidth = 1.6;
    ctx.beginPath();
    curve.forEach((p, i) => {
      const [x, y] = toCanvas(p);
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.stroke();
    pts.forEach((p) => {
      const [x, y] = toCanvas(p);
      ctx.fillStyle = active.color;
      ctx.strokeStyle = "#1c1c1c";
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(x, y, 4, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
    });
  }, [pts, active.color]);

  function hit(cx: number, cy: number): number | null {
    for (let i = 0; i < pts.length; i++) {
      const [x, y] = toCanvas(pts[i]);
      if (Math.hypot(x - cx, y - cy) < 9) return i;
    }
    return null;
  }

  const resetCurve = () => {
    applyPts(IDENTITY);
    setMenu(null);
  };

  const resetAll = async () => {
    setPts(IDENTITY);
    setMenu(null);
    await Promise.all(CHANNELS.map((ch) => commitPath(ch.path, IDENTITY)));
  };

  const flattenCurve = () => {
    applyPts(flattenPts(pts));
    setMenu(null);
  };

  const deletePoint = (index: number) => {
    if (index === 0 || index === pts.length - 1) return;
    applyPts(pts.filter((_, k) => k !== index));
    setMenu(null);
  };

  const onDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    setMenu(null);
    const r = canvasRef.current!.getBoundingClientRect();
    const cx = ((e.clientX - r.left) / r.width) * SIZE;
    const cy = ((e.clientY - r.top) / r.height) * SIZE;
    let i = hit(cx, cy);
    if (i === null) {
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
    if (i === 0) np[0] = 0;
    else if (i === next.length - 1) np[0] = 1;
    else {
      np[0] = Math.min(next[i + 1][0] - 0.01, Math.max(next[i - 1][0] + 0.01, np[0]));
    }
    next[i] = np;
    setPts(next);
    commit(next);
  };

  const onUp = () => {
    drag.current = null;
  };

  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    const r = canvasRef.current!.getBoundingClientRect();
    const cx = ((e.clientX - r.left) / r.width) * SIZE;
    const cy = ((e.clientY - r.top) / r.height) * SIZE;
    const i = hit(cx, cy);
    if (i !== null && i !== 0 && i !== pts.length - 1) {
      setMenu({ kind: "point", x: e.clientX, y: e.clientY, index: i });
    } else {
      setMenu({ kind: "curve", x: e.clientX, y: e.clientY });
    }
  };

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    window.addEventListener("pointerdown", close);
    window.addEventListener("scroll", close, true);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("scroll", close, true);
    };
  }, [menu]);

  const channelEdited = (ch: Channel) => {
    const raw = readChannelPts(doc, ch);
    return !isIdentityPayload(raw);
  };

  return (
    <div className="tone-curve">
      <div className="curve-channels">
        {CHANNELS.map((ch) => (
          <button
            key={ch.key}
            type="button"
            className={`curve-channel ${channel === ch.key ? "active" : ""}`}
            style={{ color: ch.color, borderColor: channel === ch.key ? ch.color : undefined }}
            title={`Edit ${ch.label} channel`}
            onClick={() => {
              setMenu(null);
              setChannel(ch.key);
            }}
          >
            {ch.label}
            {channelEdited(ch.key) && <span className="curve-channel-dot" />}
          </button>
        ))}
      </div>
      <div className="tone-curve-canvas-wrap">
        <canvas
          ref={canvasRef}
          width={SIZE}
          height={SIZE}
          style={{ width: "100%", aspectRatio: "1", touchAction: "none" }}
          onPointerDown={onDown}
          onPointerMove={onMove}
          onPointerUp={onUp}
          onContextMenu={onContextMenu}
        />
        {menu?.kind === "point" && (
          <div
            className="curve-menu"
            style={{ left: menu.x, top: menu.y }}
            onPointerDown={(e) => e.stopPropagation()}
          >
            <button type="button" onClick={() => deletePoint(menu.index)}>
              Delete Control Point
            </button>
          </div>
        )}
        {menu?.kind === "curve" && (
          <div
            className="curve-menu"
            style={{ left: menu.x, top: menu.y }}
            onPointerDown={(e) => e.stopPropagation()}
          >
            <button type="button" onClick={resetCurve}>
              Reset Curve
            </button>
            <button type="button" onClick={resetAll}>
              Reset All
            </button>
            <button type="button" onClick={flattenCurve}>
              Flatten
            </button>
          </div>
        )}
      </div>
      <div className="muted" style={{ fontSize: 10 }}>
        click to add · drag to shape · right-click point to delete · right-click curve to reset
      </div>
    </div>
  );
}
