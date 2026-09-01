<script lang="ts">
  import { applyOp } from "../../../ipc/commands";
  import { reconcile, doc } from "../../../stores/doc";
  import { selectedMask } from "../../../stores/app";
  import { beginMaskAdjust, endMaskAdjust } from "../../../stores/mask";

  interface Props {
    wrapEl: HTMLDivElement | null;
    imgAspect: number;
    toImageCoords: (clientX: number, clientY: number) => [number, number] | null;
    imageNormToLocal: (nx: number, ny: number) => { x: number; y: number } | null;
  }

  let { wrapEl, imgAspect, toImageCoords, imageNormToLocal }: Props = $props();

  const active = $derived(
    $selectedMask
      ? ($doc?.masks?.find((m) => m.id === $selectedMask) ?? null)
      : null,
  );

  type HandleKind = "center" | "edgeX" | "edgeY" | "rotate" | "start" | "end" | "move";

  let dragging = $state<{ kind: HandleKind; last?: [number, number] } | null>(null);

  $effect(() => {
    if (!dragging) return;
    const move = (e: PointerEvent) => onPointerMove(e);
    const up = () => onPointerUp();
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
    };
  });

  function sourceOf(m: NonNullable<typeof active>): Record<string, unknown> {
    if (m.source?.type === "composite") {
      const comps = m.source.components as { source?: Record<string, unknown> }[] | undefined;
      for (let i = comps?.length ?? 0; i > 0; i--) {
        const s = comps?.[i - 1]?.source;
        if (s?.type === "radial" || s?.type === "linear") return s;
      }
      const first = comps?.[0]?.source;
      if (first) return first;
    }
    return m.source as Record<string, unknown>;
  }

  async function updateSource(source: Record<string, unknown>, live = false) {
    if (!active) return;
    let payload: Record<string, unknown> = source;
    if (active.source?.type === "composite") {
      const comps = [...(active.source.components as Record<string, unknown>[])];
      let idx = comps.length - 1;
      for (let i = comps.length - 1; i >= 0; i--) {
        const t = (comps[i]?.source as { type?: string } | undefined)?.type;
        if (t === source.type) {
          idx = i;
          break;
        }
      }
      comps[idx] = { ...(comps[idx] as object), source };
      payload = { type: "composite", components: comps };
    }
    try {
      reconcile(
        await applyOp({ op: "set_mask_source", id: active.id, source: payload }, live),
      );
    } catch {
      /* ignore */
    }
  }

  function radialHandles(src: Record<string, unknown>) {
    const cx = (src.cx as number) ?? (src.center as number[])?.[0] ?? 0.5;
    const cy = (src.cy as number) ?? (src.center as number[])?.[1] ?? 0.5;
    const rx = (src.rx as number) ?? (src.radii as number[])?.[0] ?? 0.25;
    const ry = (src.ry as number) ?? (src.radii as number[])?.[1] ?? 0.25;
    const rotation = (src.rotation as number) ?? 0;
    return { cx, cy, rx, ry, rotation };
  }

  function linearHandles(src: Record<string, unknown>) {
    const x0 = (src.x0 as number) ?? (src.start as number[])?.[0] ?? 0.5;
    const y0 = (src.y0 as number) ?? (src.start as number[])?.[1] ?? 0.2;
    const x1 = (src.x1 as number) ?? (src.end as number[])?.[0] ?? 0.5;
    const y1 = (src.y1 as number) ?? (src.end as number[])?.[1] ?? 0.8;
    return { x0, y0, x1, y1 };
  }

  function radialImagePoint(
    cx: number,
    cy: number,
    rx: number,
    ry: number,
    rotation: number,
    theta: number,
  ): [number, number] {
    let lx = rx * Math.cos(theta);
    let ly = ry * Math.sin(theta);
    lx *= imgAspect;
    const bx = lx * Math.cos(rotation) - ly * Math.sin(rotation);
    const by = lx * Math.sin(rotation) + ly * Math.cos(rotation);
    return [cx + bx / imgAspect, cy + by];
  }

  function onPointerDown(e: PointerEvent, kind: HandleKind) {
    e.stopPropagation();
    e.preventDefault();
    const p = toImageCoords(e.clientX, e.clientY);
    dragging = { kind, last: p ?? undefined };
    beginMaskAdjust();
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging || !active) return;
    const p = toImageCoords(e.clientX, e.clientY);
    if (!p) return;
    const src = { ...sourceOf(active) };
    const kind = active.kind === "radial" || active.kind === "linear"
      ? active.kind
      : (src.type as string);

    if (kind === "radial") {
      const h = radialHandles(src);
      if (dragging.kind === "center") {
        src.type = "radial";
        src.center = [p[0], p[1]];
        src.radii = [h.rx, h.ry];
        src.rotation = h.rotation;
      } else if (dragging.kind === "edgeX") {
        const dx = (p[0] - h.cx) * imgAspect;
        const dy = p[1] - h.cy;
        const c = Math.cos(-h.rotation);
        const s = Math.sin(-h.rotation);
        const lx = dx * c - dy * s;
        src.type = "radial";
        src.center = [h.cx, h.cy];
        src.radii = [Math.max(0.02, Math.abs(lx) / imgAspect), h.ry];
        src.rotation = h.rotation;
      } else if (dragging.kind === "edgeY") {
        const dx = (p[0] - h.cx) * imgAspect;
        const dy = p[1] - h.cy;
        const c = Math.cos(-h.rotation);
        const s = Math.sin(-h.rotation);
        const ly = dx * s + dy * c;
        src.type = "radial";
        src.center = [h.cx, h.cy];
        src.radii = [h.rx, Math.max(0.02, Math.abs(ly))];
        src.rotation = h.rotation;
      } else if (dragging.kind === "rotate") {
        const dx = (p[0] - h.cx) * imgAspect;
        const dy = p[1] - h.cy;
        src.type = "radial";
        src.center = [h.cx, h.cy];
        src.radii = [h.rx, h.ry];
        src.rotation = Math.atan2(dy, dx) + Math.PI / 2;
      }
    } else if (kind === "linear") {
      const h = linearHandles(src);
      src.type = "linear";
      if (dragging.kind === "start") {
        src.start = [p[0], p[1]];
        src.end = [h.x1, h.y1];
      } else if (dragging.kind === "end") {
        src.end = [p[0], p[1]];
        src.start = [h.x0, h.y0];
      } else if (dragging.kind === "move" && dragging.last) {
        const ddx = p[0] - dragging.last[0];
        const ddy = p[1] - dragging.last[1];
        src.start = [h.x0 + ddx, h.y0 + ddy];
        src.end = [h.x1 + ddx, h.y1 + ddy];
        dragging = { kind: "move", last: p };
      }
    }
    void updateSource(src, true);
  }

  function onPointerUp() {
    if (dragging) endMaskAdjust();
    dragging = null;
  }

  const geomKind = $derived.by(() => {
    if (!active) return null;
    if (active.kind === "radial" || active.kind === "linear") return active.kind;
    const t = sourceOf(active).type;
    return t === "radial" || t === "linear" ? t : null;
  });
</script>

{#if wrapEl && active && geomKind}
  {@const src = sourceOf(active)}
  {#if geomKind === "radial"}
    {@const h = radialHandles(src)}
    {@const c = imageNormToLocal(h.cx, h.cy)}
    {@const exPt = radialImagePoint(h.cx, h.cy, h.rx, h.ry, h.rotation, 0)}
    {@const eyPt = radialImagePoint(h.cx, h.cy, h.rx, h.ry, h.rotation, Math.PI / 2)}
    {@const rotPt = radialImagePoint(h.cx, h.cy, h.rx, h.ry, h.rotation, -Math.PI / 2)}
    {@const ex = imageNormToLocal(exPt[0], exPt[1])}
    {@const ey = imageNormToLocal(eyPt[0], eyPt[1])}
    {@const rot = imageNormToLocal(rotPt[0], rotPt[1])}
    {#if c && ex && ey && rot}
      {@const rotDeg = (h.rotation * 180) / Math.PI}
      {@const rxPx = Math.max(4, Math.hypot(ex.x - c.x, ex.y - c.y))}
      {@const ryPx = Math.max(4, Math.hypot(ey.x - c.x, ey.y - c.y))}
      <svg class="mask-geo" aria-hidden="true">
        <g transform="rotate({rotDeg} {c.x} {c.y})">
          <ellipse cx={c.x} cy={c.y} rx={rxPx} ry={ryPx} class="ring" />
        </g>
        <line x1={c.x} y1={c.y} x2={rot.x} y2={rot.y} class="spoke" />
        <circle
          cx={c.x}
          cy={c.y}
          r="10"
          class="handle center"
          role="presentation"
          onpointerdown={(e) => onPointerDown(e, "center")}
        />
        <circle
          cx={ex.x}
          cy={ex.y}
          r="10"
          class="handle edge"
          role="presentation"
          onpointerdown={(e) => onPointerDown(e, "edgeX")}
        />
        <circle
          cx={ey.x}
          cy={ey.y}
          r="10"
          class="handle edge"
          role="presentation"
          onpointerdown={(e) => onPointerDown(e, "edgeY")}
        />
        <circle
          cx={rot.x}
          cy={rot.y}
          r="10"
          class="handle rotate"
          role="presentation"
          onpointerdown={(e) => onPointerDown(e, "rotate")}
        />
      </svg>
    {/if}
  {:else}
    {@const h = linearHandles(src)}
    {@const a = imageNormToLocal(h.x0, h.y0)}
    {@const b = imageNormToLocal(h.x1, h.y1)}
    {#if a && b}
      <svg class="mask-geo" aria-hidden="true">
        <line
          x1={a.x}
          y1={a.y}
          x2={b.x}
          y2={b.y}
          class="line hit"
          role="presentation"
          onpointerdown={(e) => onPointerDown(e, "move")}
        />
        <line x1={a.x} y1={a.y} x2={b.x} y2={b.y} class="line" />
        <circle
          cx={(a.x + b.x) / 2}
          cy={(a.y + b.y) / 2}
          r="9"
          class="handle move"
          role="presentation"
          onpointerdown={(e) => onPointerDown(e, "move")}
        />
        <circle
          cx={a.x}
          cy={a.y}
          r="10"
          class="handle"
          role="presentation"
          onpointerdown={(e) => onPointerDown(e, "start")}
        />
        <circle
          cx={b.x}
          cy={b.y}
          r="10"
          class="handle"
          role="presentation"
          onpointerdown={(e) => onPointerDown(e, "end")}
        />
      </svg>
    {/if}
  {/if}
{/if}

<style>
  .mask-geo {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 40;
    overflow: visible;
  }
  .ring,
  .line,
  .spoke {
    fill: none;
    stroke: rgba(255, 80, 80, 0.85);
    stroke-width: 2;
  }
  .spoke {
    stroke-dasharray: 4 4;
    pointer-events: none;
  }
  .line.hit {
    stroke: transparent;
    stroke-width: 18;
    pointer-events: stroke;
    cursor: move;
  }
  .handle {
    fill: white;
    fill-opacity: 0.92;
    stroke: rgba(255, 80, 80, 0.95);
    stroke-width: 2;
    pointer-events: auto;
    cursor: grab;
  }
  .handle.move {
    fill: rgba(255, 255, 255, 0.75);
    cursor: move;
  }
  .handle.rotate {
    fill: rgba(255, 220, 120, 0.95);
    cursor: alias;
  }
  .handle:active {
    cursor: grabbing;
  }
</style>
