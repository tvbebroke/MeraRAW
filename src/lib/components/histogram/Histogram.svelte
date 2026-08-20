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
  import { workspace } from "../../../stores/workspace";

  let {
    onResizeStart,
    height,
    class: cls = "",
    embedded = false,
  }: {
    onResizeStart?: (e: MouseEvent) => void;
    height?: number;
    class?: string;
    embedded?: boolean;
  } = $props();

  let canvasEl = $state<HTMLCanvasElement | null>(null);
  let canvasBoxW = $state(256);
  let canvasBoxH = $state(72);
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

  function persistMode(mode: HistMode) {
    setHistMode(mode);
    try {
      localStorage.setItem(`hist-mode-${workspace.get()}`, mode);
    } catch {
      /* ignore */
    }
  }

  $effect(() => {
    const ws = $workspace;
    let next: HistMode = ws === "video" ? "parade" : "rgb";
    try {
      const v = localStorage.getItem(`hist-mode-${ws}`);
      if (v === "luma" || v === "parade" || v === "rgb" || v === "wave" || v === "scope") {
        next = v;
      }
    } catch {
      /* ignore */
    }
    if (histMode.get() !== next) setHistMode(next);
  });

  $effect(() => {
    const canvas = canvasEl;
    const s = stats;
    const mode = $histMode;
    const scale = $histScale;
    const _w = canvasBoxW;
    const _h = canvasBoxH;
    if (!canvas || !s) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const W = canvas.width;
    const H = canvas.height;
    ctx.clearRect(0, 0, W, H);

    if (mode === "wave") {
      const ww = s.waveformW ?? 0;
      const wh = s.waveformH ?? 0;
      const wf = s.waveform ?? [];
      ctx.fillStyle = "#0b0b0c";
      ctx.fillRect(0, 0, W, H);
      if (ww > 0 && wh > 0 && wf.length >= ww * wh) {
        let max = 1;
        for (const n of wf) max = Math.max(max, n);
        const cw = W / ww;
        const ch = H / wh;
        for (let y = 0; y < wh; y++) {
          for (let x = 0; x < ww; x++) {
            const v = wf[y * ww + x] / max;
            if (v <= 0) continue;
            const a = Math.min(1, 0.15 + v * 0.85);
            ctx.fillStyle = `rgba(210, 230, 210, ${a})`;
            ctx.fillRect(x * cw, y * ch, Math.ceil(cw), Math.ceil(ch));
          }
        }
      }
    } else if (mode === "scope") {
      const vs = s.vectorscopeSize ?? 0;
      const bins = s.vectorscope ?? [];
      ctx.fillStyle = "#0b0b0c";
      ctx.fillRect(0, 0, W, H);
      const side = Math.min(W, H);
      const ox = (W - side) / 2;
      const oy = (H - side) / 2;
      if (vs > 0 && bins.length >= vs * vs) {
        let max = 1;
        for (const n of bins) max = Math.max(max, n);
        const cw = side / vs;
        for (let y = 0; y < vs; y++) {
          for (let x = 0; x < vs; x++) {
            const v = bins[y * vs + x] / max;
            if (v <= 0) continue;
            const a = Math.min(1, 0.12 + v * 0.88);
            ctx.fillStyle = `rgba(180, 220, 160, ${a})`;
            ctx.fillRect(ox + x * cw, oy + y * cw, Math.ceil(cw), Math.ceil(cw));
          }
        }
      }
      ctx.strokeStyle = "rgba(255,255,255,0.18)";
      ctx.beginPath();
      ctx.arc(ox + side / 2, oy + side / 2, side * 0.42, 0, Math.PI * 2);
      ctx.stroke();
    } else if (mode === "luma") {
      const max = Math.max(1, ...s.luma);
      drawChannel(ctx, s.luma, "rgba(230,230,230,0.75)", 0, W, H, max, scale);
    } else if (mode === "parade") {
      const ww = s.waveformW ?? 0;
      const wh = s.waveformH ?? 0;
      const packed = s.parade ?? [];
      const plane = ww * wh;
      ctx.fillStyle = "#0b0b0c";
      ctx.fillRect(0, 0, W, H);
      if (ww > 0 && wh > 0 && packed.length >= plane * 3) {
        let max = 1;
        for (const n of packed) max = Math.max(max, n);
        const third = W / 3;
        const colors = [
          "255,90,90",
          "110,230,110",
          "110,140,255",
        ];
        for (let p = 0; p < 3; p++) {
          const cw = third / ww;
          const ch = H / wh;
          const x0 = p * third;
          for (let y = 0; y < wh; y++) {
            for (let x = 0; x < ww; x++) {
              const v = packed[p * plane + y * ww + x] / max;
              if (v <= 0) continue;
              const a = Math.min(1, 0.12 + v * 0.88);
              ctx.fillStyle = `rgba(${colors[p]}, ${a})`;
              ctx.fillRect(x0 + x * cw, y * ch, Math.ceil(cw), Math.ceil(ch));
            }
          }
        }
        ctx.strokeStyle = "rgba(255,255,255,0.12)";
        ctx.beginPath();
        ctx.moveTo(third, 0);
        ctx.lineTo(third, H);
        ctx.moveTo(third * 2, 0);
        ctx.lineTo(third * 2, H);
        ctx.stroke();
      } else {
        const third = W / 3;
        const max = Math.max(1, ...s.r, ...s.g, ...s.b);
        drawChannel(ctx, s.r, "rgba(255,90,90,0.7)", 0, third, H, max, scale);
        drawChannel(ctx, s.g, "rgba(110,230,110,0.7)", third, third, H, max, scale);
        drawChannel(ctx, s.b, "rgba(110,140,255,0.7)", third * 2, third, H, max, scale);
      }
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
    { id: "wave", label: "Wave" },
    { id: "scope", label: "Scope" },
  ];
  const scales: { id: HistScale; label: string }[] = [
    { id: "sqrt", label: "√" },
    { id: "linear", label: "Lin" },
    { id: "log", label: "Log" },
  ];
</script>

<div class="relative {embedded ? 'h-full min-h-0' : 'shrink-0'} {cls}" style={embedded ? undefined : `height: ${height}px`}>
  {#if onResizeStart && !embedded}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      role="separator"
      aria-orientation="horizontal"
      class="group absolute left-0 right-0 -top-[11px] h-[11px] cursor-row-resize z-50 flex items-center justify-center focus:outline-none"
      onmousedown={onResizeStart}
    >
      <div
        class="h-[2px] w-[40px] rounded-full bg-hover group-hover:bg-border-strong group-active:bg-border-strong transition-all duration-200"
      ></div>
    </div>
  {/if}

  {#if embedded}
    <div class="relative flex h-full min-h-0 flex-col">
      <div class="mb-[6px] flex min-w-0 items-center justify-between gap-1 shrink-0">
        <div class="flex min-w-0 flex-wrap gap-[2px]">
          {#each modes as m}
            <button
              type="button"
              class="rail-chip {$histMode === m.id ? 'is-active' : ''}"
              onclick={() => persistMode(m.id)}
            >{m.label}</button>
          {/each}
        </div>
        <div class="flex gap-[2px]">
          {#each scales as sc}
            <button
              type="button"
              class="rail-chip {$histScale === sc.id ? 'is-active' : ''}"
              onclick={() => setHistScale(sc.id)}
              title="Y scale"
            >{sc.label}</button>
          {/each}
        </div>
      </div>
      <div bind:clientWidth={canvasBoxW} bind:clientHeight={canvasBoxH} class="relative min-h-0 flex-1">
        <canvas
          bind:this={canvasEl}
          width={canvasBoxW}
          height={canvasBoxH}
          class="absolute inset-0 h-full w-full rounded-[4px] bg-black/30"
        ></canvas>
      </div>
    </div>
  {:else}
  <GlassPanel class="relative flex h-full flex-col overflow-hidden">
    <div class="flex min-h-0 flex-1 flex-col px-[12px] py-[10px]">
      <div class="mb-[8px] flex items-center justify-between gap-1 shrink-0">
        <div class="flex gap-[2px]">
          {#each modes as m}
            <button
              type="button"
              class="rail-chip {$histMode === m.id ? 'is-active' : ''}"
              onclick={() => persistMode(m.id)}
            >{m.label}</button>
          {/each}
        </div>
        <div class="flex gap-[2px]">
          {#each scales as sc}
            <button
              type="button"
              class="rail-chip {$histScale === sc.id ? 'is-active' : ''}"
              onclick={() => setHistScale(sc.id)}
              title="Y scale"
            >{sc.label}</button>
          {/each}
        </div>
      </div>

      <div bind:clientWidth={canvasBoxW} bind:clientHeight={canvasBoxH} class="relative min-h-0 flex-1">
        <canvas
          bind:this={canvasEl}
          width={canvasBoxW}
          height={canvasBoxH}
          class="absolute inset-0 h-full w-full rounded-[4px] border border-border bg-black/40"
        ></canvas>
      </div>
      {#if stats}
        <div class="num mt-[8px] flex shrink-0 items-center justify-between gap-2 text-[8px] text-subtle">
          <button
            type="button"
            class="truncate {clipLo ? 'text-sky-300 animate-pulse' : 'hover:text-secondary'}"
            title="Toggle shadow clipping blinkies"
            onclick={() => {
              clipLo = !clipLo;
              syncClip();
            }}
          >
            ▼ {stats.clipLowPct.toFixed(1)}%
          </button>
          <span class="text-subtle">{stats.bins} bins</span>
          <button
            type="button"
            class="truncate {clipHi ? 'text-red-300 animate-pulse' : 'hover:text-secondary'}"
            title="Toggle highlight clipping blinkies"
            onclick={() => {
              clipHi = !clipHi;
              syncClip();
            }}
          >
            ▲ {stats.clipHighPct.toFixed(1)}%
          </button>
        </div>
      {:else}
        <span class="mt-[8px] shrink-0 text-center text-[9px] text-subtle">Histogram</span>
      {/if}
    </div>
  </GlassPanel>
  {/if}
</div>
