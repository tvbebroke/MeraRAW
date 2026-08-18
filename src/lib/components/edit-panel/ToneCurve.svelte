<script lang="ts">
  import { curveChannel, type CurveChannel } from "../../../stores/editor";
  import { doc, reconcile } from "../../../stores/doc";
  import { setParam } from "../../../ipc/commands";
  import CollapsibleSection from "./CollapsibleSection.svelte";

  const channels: { id: CurveChannel; color: string; stroke: string }[] = [
    { id: "luma", color: "#e8e8e8", stroke: "rgba(255, 255, 255, 0.85)" },
    { id: "red", color: "#e64f4f", stroke: "#e64f4f" },
    { id: "green", color: "#0fb327", stroke: "#0fb327" },
    { id: "blue", color: "#3b82f6", stroke: "#3b82f6" },
  ];

  const PATH: Record<CurveChannel, string> = {
    luma: "tone_curve.points",
    red: "tone_curve.points_r",
    green: "tone_curve.points_g",
    blue: "tone_curve.points_b",
  };

  const IDENTITY = [
    { x: 0, y: 0 },
    { x: 1, y: 1 },
  ];

  function readChannel(ch: CurveChannel): { x: number; y: number }[] {
    const key =
      ch === "luma" ? "points" : ch === "red" ? "points_r" : ch === "green" ? "points_g" : "points_b";
    const raw = $doc?.modules?.tone_curve?.[key];
    if (!Array.isArray(raw) || raw.length < 2) return [...IDENTITY];
    return (raw as [number, number][]).map(([x, y]) => ({ x, y }));
  }

  let pointsByChannel = $state<Record<CurveChannel, { x: number; y: number }[]>>({
    luma: [...IDENTITY],
    red: [...IDENTITY],
    green: [...IDENTITY],
    blue: [...IDENTITY],
  });

  // Keep local points in sync when the doc mirror changes (undo/redo/open).
  $effect(() => {
    void $doc;
    pointsByChannel = {
      luma: readChannel("luma"),
      red: readChannel("red"),
      green: readChannel("green"),
      blue: readChannel("blue"),
    };
  });

  const activePoints = $derived(pointsByChannel[$curveChannel]);
  const activeChannelColor = $derived(
    channels.find((c) => c.id === $curveChannel)?.stroke || "rgba(255,255,255,0.7)",
  );

  const splinePath = $derived.by(() => {
    const pts = activePoints;
    if (pts.length === 0) return "";
    let path = `M ${pts[0].x * 256} ${(1 - pts[0].y) * 192}`;
    if (pts.length === 2) {
      path += ` L ${pts[1].x * 256} ${(1 - pts[1].y) * 192}`;
      return path;
    }
    for (let i = 0; i < pts.length - 1; i++) {
      const p0 = pts[i - 1] || pts[i];
      const p1 = pts[i];
      const p2 = pts[i + 1];
      const p3 = pts[i + 2] || p2;
      const cp1x = p1.x + (p2.x - p0.x) / 6;
      const cp1y = p1.y + (p2.y - p0.y) / 6;
      const cp2x = p2.x - (p3.x - p1.x) / 6;
      const cp2y = p2.y - (p3.y - p1.y) / 6;
      path += ` C ${cp1x * 256} ${(1 - cp1y) * 192}, ${cp2x * 256} ${(1 - cp2y) * 192}, ${p2.x * 256} ${(1 - p2.y) * 192}`;
    }
    return path;
  });

  let draggedIndex = $state<number | null>(null);

  function payload(pts: { x: number; y: number }[]): [number, number][] | [] {
    const isIdentity =
      pts.length === 2 &&
      pts[0].x === 0 &&
      pts[0].y === 0 &&
      pts[1].x === 1 &&
      pts[1].y === 1;
    if (isIdentity) return [];
    return pts.map((p) => [p.x, p.y] as [number, number]);
  }

  function commit(ch: CurveChannel = $curveChannel) {
    const pts = pointsByChannel[ch];
    void setParam(PATH[ch], payload(pts))
      .then(reconcile)
      .catch(() => {});
  }

  function startDrag(e: PointerEvent, index: number) {
    e.preventDefault();
    draggedIndex = index;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function handlePointerMove(e: PointerEvent) {
    if (draggedIndex === null) return;
    const svgElement = e.currentTarget as SVGSVGElement;
    const rect = svgElement.getBoundingClientRect();
    const currentX = (e.clientX - rect.left) / rect.width;
    const currentY = 1 - (e.clientY - rect.top) / rect.height;
    const updated = [...activePoints];
    const pt = { ...updated[draggedIndex] };
    pt.y = Math.max(0, Math.min(1, currentY));
    if (draggedIndex === 0) pt.x = 0;
    else if (draggedIndex === activePoints.length - 1) pt.x = 1;
    else {
      const minX = updated[draggedIndex - 1].x + 0.05;
      const maxX = updated[draggedIndex + 1].x - 0.05;
      pt.x = Math.max(minX, Math.min(maxX, currentX));
    }
    updated[draggedIndex] = pt;
    pointsByChannel[$curveChannel] = updated;
  }

  function stopDrag(e: PointerEvent) {
    if (draggedIndex !== null) {
      try {
        (e.target as HTMLElement).releasePointerCapture(e.pointerId);
      } catch {
        /* ignore */
      }
      draggedIndex = null;
      commit();
    }
  }

  function handleSvgPointerDown(e: PointerEvent) {
    if ((e.target as SVGElement).tagName === "circle") return;
    const svgElement = e.currentTarget as SVGSVGElement;
    const rect = svgElement.getBoundingClientRect();
    const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    const y = Math.max(0, Math.min(1, 1 - (e.clientY - rect.top) / rect.height));
    let insertIdx = 0;
    while (insertIdx < activePoints.length && activePoints[insertIdx].x < x) insertIdx++;
    const updated = [...activePoints];
    updated.splice(insertIdx, 0, { x, y });
    pointsByChannel[$curveChannel] = updated;
    draggedIndex = insertIdx;
    svgElement.setPointerCapture(e.pointerId);
  }

  function removePoint(index: number) {
    if (index === 0 || index === activePoints.length - 1) return;
    const updated = [...activePoints];
    updated.splice(index, 1);
    pointsByChannel[$curveChannel] = updated;
    commit();
  }
</script>

<CollapsibleSection id="curve" title="Tone Curve">
<div class="curve">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="canvas">
    <svg
      viewBox="0 0 256 192"
      class="svg"
      onpointerdown={handleSvgPointerDown}
      onpointermove={handlePointerMove}
      onpointerup={stopDrag}
      onpointercancel={stopDrag}
    >
      <line x1="64" y1="0" x2="64" y2="192" stroke="white" stroke-opacity="0.05" stroke-dasharray="2 2" />
      <line x1="128" y1="0" x2="128" y2="192" stroke="white" stroke-opacity="0.05" stroke-dasharray="2 2" />
      <line x1="192" y1="0" x2="192" y2="192" stroke="white" stroke-opacity="0.05" stroke-dasharray="2 2" />
      <line x1="0" y1="48" x2="256" y2="48" stroke="white" stroke-opacity="0.05" stroke-dasharray="2 2" />
      <line x1="0" y1="96" x2="256" y2="96" stroke="white" stroke-opacity="0.05" stroke-dasharray="2 2" />
      <line x1="0" y1="144" x2="256" y2="144" stroke="white" stroke-opacity="0.05" stroke-dasharray="2 2" />
      <path
        d={splinePath}
        fill="none"
        stroke={activeChannelColor}
        stroke-width="1.8"
        stroke-linecap="round"
      />
      {#each activePoints as p, i}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <circle
          cx={p.x * 256}
          cy={(1 - p.y) * 192}
          r={draggedIndex === i ? 5.5 : 4}
          class="cursor-pointer fill-fg stroke-black/60 stroke-[1.5px] transition-all duration-100 hover:fill-[#a8a8a8] active:fill-[#8c8c8c]"
          onpointerdown={(e) => startDrag(e, i)}
          ondblclick={(e) => {
            e.stopPropagation();
            removePoint(i);
          }}
          oncontextmenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            removePoint(i);
          }}
        />
      {/each}
    </svg>
  </div>

  <div class="dots">
    {#each channels as ch (ch.id)}
      <button
        aria-label="{ch.id} channel"
        class="dot"
        class:on={$curveChannel === ch.id}
        style="background: {ch.color}"
        onclick={() => curveChannel.set(ch.id)}
      ></button>
    {/each}
  </div>
</div>
</CollapsibleSection>

<style>
  .curve { display: flex; flex-direction: column; gap: var(--space-2); }
  .canvas {
    width: 100%;
    aspect-ratio: 4 / 3;
    position: relative;
    overflow: hidden;
    border-radius: 8px;
    border: 1px solid var(--color-border);
    background: var(--color-active);
    touch-action: none;
    user-select: none;
  }
  .svg { position: absolute; inset: 0; width: 100%; height: 100%; cursor: crosshair; }
  .dots {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    min-height: 28px;
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    border: 0;
    padding: 0;
    cursor: pointer;
    opacity: 0.4;
  }
  .dot.on {
    opacity: 1;
    outline: 1px solid var(--color-fg);
    outline-offset: 2px;
  }
</style>
