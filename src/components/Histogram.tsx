// RGB histogram + clip readout (contract F1). Updates on render settle.
import { useEffect, useRef, useState } from "react";
import { getStats } from "../ipc/commands";
import { onFrameReady } from "../ipc/events";
import type { FrameStats } from "../ipc/types";

export function Histogram() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [stats, setStats] = useState<FrameStats | null>(null);
  const timer = useRef<number | null>(null);

  useEffect(() => {
    const refresh = () => {
      if (timer.current) window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => {
        getStats().then(setStats).catch(() => {});
      }, 250);
    };
    refresh();
    const un = onFrameReady(refresh);
    return () => {
      un.then((f) => f());
      if (timer.current) window.clearTimeout(timer.current);
    };
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !stats) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const { width: W, height: H } = canvas;
    ctx.clearRect(0, 0, W, H);
    const channels: [number[], string][] = [
      [stats.r, "rgba(255,90,90,0.55)"],
      [stats.g, "rgba(110,230,110,0.55)"],
      [stats.b, "rgba(110,140,255,0.55)"],
    ];
    const max = Math.max(
      1,
      ...channels.flatMap(([c]) => c),
    );
    for (const [bins, color] of channels) {
      ctx.fillStyle = color;
      const bw = W / bins.length;
      for (let i = 0; i < bins.length; i++) {
        const h = (Math.sqrt(bins[i] / max)) * (H - 2);
        ctx.fillRect(i * bw, H - h, Math.ceil(bw), h);
      }
    }
  }, [stats]);

  return (
    <div>
      <canvas
        ref={canvasRef}
        width={252}
        height={72}
        style={{
          width: "100%",
          background: "var(--bg)",
          border: "1px solid var(--border)",
          borderRadius: 4,
        }}
      />
      {stats && (
        <div className="muted" style={{ display: "flex", justifyContent: "space-between" }}>
          <span>▼ {stats.clipLowPct.toFixed(1)}%</span>
          <span>▲ {stats.clipHighPct.toFixed(1)}%</span>
        </div>
      )}
    </div>
  );
}
