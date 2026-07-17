<script lang="ts">
  import GlassPanel from "../primitives/GlassPanel.svelte";
  import { getStats } from "../../../ipc/commands";
  import { onFrameReady } from "../../../ipc/events";
  import type { FrameStats } from "../../../ipc/types";

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
  let timer: ReturnType<typeof setTimeout> | undefined;

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
    if (!canvas || !s) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const W = canvas.width;
    const H = canvas.height;
    ctx.clearRect(0, 0, W, H);
    const channels: [number[], string][] = [
      [s.r, "rgba(255,90,90,0.55)"],
      [s.g, "rgba(110,230,110,0.55)"],
      [s.b, "rgba(110,140,255,0.55)"],
    ];
    const max = Math.max(1, ...channels.flatMap(([c]) => c));
    for (const [bins, color] of channels) {
      ctx.fillStyle = color;
      const bw = W / bins.length;
      for (let i = 0; i < bins.length; i++) {
        const h = Math.sqrt(bins[i] / max) * (H - 2);
        ctx.fillRect(i * bw, H - h, Math.ceil(bw), h);
      }
    }
  });
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
    <canvas
      bind:this={canvasEl}
      width={252}
      height={72}
      class="w-full rounded-[4px] border border-white/[0.06] bg-black/40"
    ></canvas>
    {#if stats}
      <div class="mt-[4px] flex justify-between text-[8px] text-white/35">
        <span>▼ {stats.clipLowPct.toFixed(1)}%</span>
        <span>▲ {stats.clipHighPct.toFixed(1)}%</span>
      </div>
    {:else}
      <span class="mt-[4px] text-center text-[9px] text-white/20">Histogram</span>
    {/if}
  </div>
</GlassPanel>
