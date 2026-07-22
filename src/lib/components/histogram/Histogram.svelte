<script lang="ts">
  import GlassPanel from "../primitives/GlassPanel.svelte";
  import { getStats, setClipWarnings } from "../../../ipc/commands";
  import { onFrameReady } from "../../../ipc/events";
  import type { FrameStats } from "../../../ipc/types";
  import {
    histMode,
    histScale,
    setHistMode,
    setHistScale,
    type HistMode,
    type HistScale,
  } from "../../../stores/app";

  let {
    onResizeStart,
    height,
    class: cls = "",
  }: {
    onResizeStart?: (e: MouseEvent) => void;
    height?: number;
    class?: string;
  } = $props();

  let canvasEl = $state<HTMLCanvasElement | null>(null);
  let stats = $state<FrameStats | null>(null);
  let clipHi = $state(false);
  let clipLo = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function syncClip() {
    void setClipWarnings(clipHi, clipLo).catch(() => {});
  }

  function refresh() {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
      getStats()
        .then((s) => {
          if (s) stats = s;
        })
        .catch(() => {});
    }, 250);
  }

  function scaleY(v: number, max: number, H: number, scale: HistScale): number {
    if (max <= 0 || v <= 0) return 0;
    const t = v / max;
    const n =
      scale === "linear" ? t : scale === "log" ? Math.log1p(v) / Math.log1p(max) : Math.sqrt(t);
    return n * (H - 2);
  }

  function drawChannel(
    ctx: CanvasRenderingContext2D,
    bins: number[],
    color: string,
    x0: number,
    w: number,
    H: number,
    max: number,
    scale: HistScale,
  ) {
    ctx.fillStyle = color;
    const bw = w / bins.length;
    for (let i = 0; i < bins.length; i++) {
      const h = scaleY(bins[i], max, H, scale);
      ctx.fillRect(x0 + i * bw, H - h, Math.ceil(bw), h);
    }
  }

  $effect(() => {
    refresh();
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    onFrameReady(refresh).then((u) => {
      if (cancelled) u();
      else unlisten = u;
    });
    return () => {
      cancelled = true;
      unlisten?.();
      if (timer) clearTimeout(timer);
    };
  });

  $effect(() => {
    const canvas = canvasEl;
    const s = stats;
    const mode = $histMode;
    const scale = $histScale;
    if (!canvas || !s) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const W = canvas.width;
    const H = canvas.height;
    ctx.clearRect(0, 0, W, H);

    if (mode === "luma") {
      const max = Math.max(1, ...s.luma);
      drawChannel(ctx, s.luma, "rgba(230,230,230,0.75)", 0, W, H, max, scale);
    } else if (mode === "parade") {
      const third = W / 3;
      const max = Math.max(1, ...s.r, ...s.g, ...s.b);
      drawChannel(ctx, s.r, "rgba(255,90,90,0.7)", 0, third, H, max, scale);
      drawChannel(ctx, s.g, "rgba(110,230,110,0.7)", third, third, H, max, scale);
      drawChannel(ctx, s.b, "rgba(110,140,255,0.7)", third * 2, third, H, max, scale);
      ctx.strokeStyle = "rgba(255,255,255,0.12)";
      ctx.beginPath();
      ctx.moveTo(third, 0);
      ctx.lineTo(third, H);
      ctx.moveTo(third * 2, 0);
      ctx.lineTo(third * 2, H);
      ctx.stroke();
    } else {
      const channels: [number[], string][] = [
        [s.r, "rgba(255,90,90,0.55)"],
        [s.g, "rgba(110,230,110,0.55)"],
        [s.b, "rgba(110,140,255,0.55)"],
      ];
      const max = Math.max(1, ...channels.flatMap(([c]) => c));
      for (const [bins, color] of channels) {
        drawChannel(ctx, bins, color, 0, W, H, max, scale);
      }
    }
  });

  const modes: { id: HistMode; label: string }[] = [
    { id: "rgb", label: "RGB" },
    { id: "luma", label: "Luma" },
    { id: "parade", label: "Parade" },
  ];
  const scales: { id: HistScale; label: string }[] = [
    { id: "sqrt", label: "√" },
    { id: "linear", label: "Lin" },
    { id: "log", label: "Log" },
  ];
</script>

<GlassPanel
  class="relative flex shrink-0 flex-col overflow-hidden {cls}"
  style="height: {height}px"
>
  {#if onResizeStart}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      role="separator"
      aria-orientation="horizontal"
      class="group absolute left-0 right-0 -top-[11px] h-[11px] cursor-row-resize z-50 flex items-center justify-center focus:outline-none"
      onmousedown={onResizeStart}
    >
      <div
        class="h-[2px] w-[40px] rounded-full bg-white/5 group-hover:bg-white/25 group-active:bg-accent transition-all duration-200"
      ></div>
    </div>
  {/if}

  <div class="flex min-h-0 flex-1 flex-col justify-center px-[8px] py-[6px]">
    <div class="mb-[4px] flex items-center justify-between gap-1">
      <div class="flex gap-[2px]">
        {#each modes as m}
          <button
            type="button"
            class="rounded px-[5px] py-[1px] text-[8px] {$histMode === m.id
              ? 'bg-white/12 text-white/90'
              : 'text-white/35 hover:text-white/60'}"
            onclick={() => setHistMode(m.id)}
          >{m.label}</button>
        {/each}
      </div>
      <div class="flex gap-[2px]">
        {#each scales as sc}
          <button
            type="button"
            class="rounded px-[5px] py-[1px] text-[8px] {$histScale === sc.id
              ? 'bg-white/12 text-white/90'
              : 'text-white/35 hover:text-white/60'}"
            onclick={() => setHistScale(sc.id)}
            title="Y scale"
          >{sc.label}</button>
        {/each}
      </div>
    </div>

    <canvas
      bind:this={canvasEl}
      width={256}
      height={72}
      class="w-full rounded-[4px] border border-white/[0.06] bg-black/40"
    ></canvas>
    {#if stats}
      <div class="mt-[4px] flex items-center justify-between gap-2 text-[8px] text-white/35">
        <button
          type="button"
          class="truncate {clipLo ? 'text-sky-300 animate-pulse' : 'hover:text-white/60'}"
          title="Toggle shadow clipping overlay"
          onclick={() => {
            clipLo = !clipLo;
            syncClip();
          }}
        >
          ▼ {stats.clipLowPct.toFixed(1)}%
        </button>
        <span class="text-white/20">{stats.bins} bins</span>
        <button
          type="button"
          class="truncate {clipHi ? 'text-red-300 animate-pulse' : 'hover:text-white/60'}"
          title="Toggle highlight clipping overlay"
          onclick={() => {
            clipHi = !clipHi;
            syncClip();
          }}
        >
          ▲ {stats.clipHighPct.toFixed(1)}%
        </button>
      </div>
    {:else}
      <span class="mt-[4px] text-center text-[9px] text-white/20">Histogram</span>
    {/if}
  </div>
</GlassPanel>
