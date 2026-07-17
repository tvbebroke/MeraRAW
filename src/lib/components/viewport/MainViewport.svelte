<script lang="ts">
  import GlassPanel from "../primitives/GlassPanel.svelte";
  import {
    applyOp,
    reportFrontendStatus,
    requestFrame,
    setDisplayLook,
    setPreviewBypass,
    wbFromPoint,
  } from "../../../ipc/commands";
  import { onEngineReady, onFrameReady } from "../../../ipc/events";
  import {
    contentDims,
    contentNormToImageNorm,
    cropModeFor,
    readCropFromDoc,
  } from "../../../crop/cropMath";
  import { preloadFrame, probeFrameTransport } from "../../engine/frame";
  import {
    cropActive,
    decodeState,
    displayLook,
    engineReady,
    imageDims,
    imageOpen,
    lastOpenedPath,
    previewBypass,
    sendViewCmd,
    selectedMask,
    viewportTool,
    viewCmdNonce,
    viewCmd,
    zoomLabel,
    brushRadius,
  } from "../../../stores/app";
  import { doc, reconcile } from "../../../stores/doc";

  let { minimal = false }: { minimal?: boolean } = $props();

  interface ViewState {
    scale: number | null;
    centerX: number;
    centerY: number;
  }

  const LOOKS: { label: string; value: number }[] = [
    { label: "Neutral", value: 0 },
    { label: "Camera", value: 1 },
    { label: "Filmic", value: 2 },
    { label: "Original", value: 4 },
  ];

  let wrapEl = $state<HTMLDivElement | null>(null);
  let displaySrc = $state<string | null>(null);
  let error = $state<string | null>(null);

  let view: ViewState = { scale: null, centerX: 0.5, centerY: 0.5 };
  let shownVer = 0;
  let pendingVer = 0;
  let inFlight = false;
  let pending = false;
  let dragging: { x: number; y: number } | null = null;
  let effScale = 1;
  let brushPoints: [number, number][] = [];
  let forceNext = false;
  let firstCropToggle = true;

  function appZoom(): number {
    return (
      parseFloat(
        getComputedStyle(document.documentElement).getPropertyValue("--app-zoom"),
      ) || 1
    );
  }

  function measureWrap(wrap: HTMLElement) {
    const layoutW = wrap.clientWidth;
    const layoutH = wrap.clientHeight;
    if (layoutW < 8 || layoutH < 8) return null;
    const zoom = appZoom();
    const dpr = window.devicePixelRatio || 1;
    const clamp = (n: number) => Math.min(8192, Math.max(1, Math.round(n)));
    return {
      layoutW,
      layoutH,
      outW: clamp(layoutW * zoom * dpr),
      outH: clamp(layoutH * zoom * dpr),
    };
  }

  function isCustomView(v: ViewState): boolean {
    return v.scale !== null || v.centerX !== 0.5 || v.centerY !== 0.5;
  }

  function viewContentDims(): { w: number; h: number } | null {
    const dims = imageDims.get();
    if (!dims) return null;
    const crop = readCropFromDoc(doc.get()?.modules);
    const mode = cropModeFor(crop, cropActive.get());
    const [w, h] = contentDims(crop, dims.w, dims.h, mode);
    return { w, h };
  }

  function screenToOriginalNorm(
    clientX: number,
    clientY: number,
    wrap: HTMLElement,
    scale: number,
    v: ViewState,
  ): [number, number] | null {
    const dims = imageDims.get();
    if (!dims) return null;
    const crop = readCropFromDoc(doc.get()?.modules);
    const mode = cropModeFor(crop, cropActive.get());
    const [cw, ch] = contentDims(crop, dims.w, dims.h, mode);
    const rect = wrap.getBoundingClientRect();
    const dpr = window.devicePixelRatio;
    const px = (clientX - rect.left) * dpr;
    const py = (clientY - rect.top) * dpr;
    const nx = (v.centerX * cw + (px - (rect.width * dpr) / 2) / scale) / cw;
    const ny = (v.centerY * ch + (py - (rect.height * dpr) / 2) / scale) / ch;
    return contentNormToImageNorm(nx, ny, crop, dims.w, dims.h, mode);
  }

  function updateZoomLabel() {
    const dims = viewContentDims();
    const wrap = wrapEl;
    const m = wrap && dims ? measureWrap(wrap) : null;
    if (!dims || !m) return;
    effScale = view.scale ?? Math.min(m.outW / dims.w, m.outH / dims.h);
    zoomLabel.set(view.scale === null ? "fit" : `${Math.round(view.scale * 100)}%`);
  }

  function showFrame(version: number) {
    if (version <= shownVer) return;
    pendingVer = version;
    preloadFrame(version)
      .then((url) => {
        if (pendingVer !== version) return;
        shownVer = version;
        displaySrc = url;
        error = null;
        requestAnimationFrame(() => updateZoomLabel());
      })
      .catch(() => {
        if (pendingVer === version) error = "frame transport failed";
      });
  }

  async function refresh(force = false) {
    if (decodeState.get() !== "ready") return;
    const wrap = wrapEl;
    if (!wrap) return;
    const m = measureWrap(wrap);
    if (!m) return;
    if (force) forceNext = true;
    if (inFlight) {
      pending = true;
      return;
    }
    inFlight = true;
    try {
      if (!forceNext && !isCustomView(view) && shownVer > 0) {
        updateZoomLabel();
        return;
      }
      forceNext = false;
      const info = await requestFrame({
        outW: m.outW,
        outH: m.outH,
        scale: view.scale,
        centerX: view.centerX,
        centerY: view.centerY,
        cropPreview: cropActive.get(),
      });
      showFrame(info.version);
    } catch {
      // preview phase — ignore
    } finally {
      inFlight = false;
      if (pending) {
        pending = false;
        void refresh();
      }
    }
  }

  function toImageCoords(e: { clientX: number; clientY: number }): [number, number] | null {
    if (!wrapEl) return null;
    return screenToOriginalNorm(e.clientX, e.clientY, wrapEl, effScale, view);
  }

  function pickLook(value: number) {
    displayLook.set(value);
    void setDisplayLook(value).catch(() => {});
  }

  function toggleAfter() {
    const next = !previewBypass.get();
    previewBypass.set(next);
    void setPreviewBypass(next).catch(() => {});
  }

  function toggleFullscreen() {
    if (!document.fullscreenElement) {
      document.documentElement.requestFullscreen().catch(() => {});
    } else {
      document.exitFullscreen().catch(() => {});
    }
  }

  function onWheel(e: WheelEvent) {
    if (!imageOpen.get() || !imageDims.get()) return;
    e.preventDefault();
    const factor = Math.exp(-e.deltaY * 0.0015);
    view.scale = Math.min(8, Math.max(0.02, effScale * factor));
    void refresh();
  }

  function onPointerDown(e: PointerEvent) {
    const tool = viewportTool.get();
    if (tool === "brush" && selectedMask.get()) {
      const p = toImageCoords(e);
      if (p) brushPoints = [p];
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
      return;
    }
    if (tool === "crop" && cropActive.get()) return;
    if (tool === "wb" && imageDims.get() && wrapEl) {
      const p = screenToOriginalNorm(
        e.clientX,
        e.clientY,
        wrapEl,
        effScale,
        view,
      );
      if (p) {
        wbFromPoint(p[0], p[1])
          .then((delta) => reconcile(delta))
          .catch(() => {});
      }
      viewportTool.set("pan");
      return;
    }
    dragging = { x: e.clientX, y: e.clientY };
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    const tool = viewportTool.get();
    if (tool === "crop" && cropActive.get()) return;
    if (tool === "brush" && brushPoints.length > 0) {
      const p = toImageCoords(e);
      if (p) brushPoints.push(p);
      return;
    }
    if (!dragging || !imageOpen.get() || !imageDims.get()) return;
    const content = viewContentDims();
    if (!content) return;
    const dpr = window.devicePixelRatio;
    const dx = (e.clientX - dragging.x) * dpr;
    const dy = (e.clientY - dragging.y) * dpr;
    dragging = { x: e.clientX, y: e.clientY };
    const s = effScale;
    view.centerX -= dx / s / content.w;
    view.centerY -= dy / s / content.h;
    view.centerX = Math.min(1, Math.max(0, view.centerX));
    view.centerY = Math.min(1, Math.max(0, view.centerY));
    void refresh();
  }

  function onPointerUp() {
    dragging = null;
    const tool = viewportTool.get();
    if (tool === "brush" && selectedMask.get() && brushPoints.length > 0) {
      const pts = brushPoints;
      brushPoints = [];
      const d = doc.get();
      const maskId = selectedMask.get();
      const mask = d?.masks?.find((m) => m.id === maskId);
      if (mask && mask.source.type === "brush") {
        const strokes = Array.isArray(mask.source.strokes)
          ? [...(mask.source.strokes as unknown[])]
          : [];
        strokes.push({
          points: pts,
          radius: brushRadius.get(),
          hardness: 0.6,
          mode: "add",
        });
        applyOp({
          op: "set_mask_source",
          id: mask.id,
          source: { type: "brush", strokes },
        })
          .then((delta) => reconcile(delta))
          .catch(() => {});
      }
    }
  }

  function onDoubleClick() {
    if (!imageOpen.get()) return;
    if (view.scale === null) view.scale = 1;
    else view = { scale: null, centerX: 0.5, centerY: 0.5 };
    void refresh();
  }

  // Reset view when a new image opens.
  $effect(() => {
    const _path = $lastOpenedPath;
    view = { scale: null, centerX: 0.5, centerY: 0.5 };
    shownVer = 0;
    pendingVer = 0;
    displaySrc = null;
  });

  // Crop tool toggles content space — reset to fit.
  $effect(() => {
    const _crop = $cropActive;
    if (firstCropToggle) {
      firstCropToggle = false;
      return;
    }
    view = { scale: null, centerX: 0.5, centerY: 0.5 };
    void refresh(true);
  });

  // Frame-ready events from the engine.
  $effect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    onFrameReady((version) => showFrame(version)).then((u) => {
      if (cancelled) u();
      else unlisten = u;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  });

  // Probe frame:// transport once the engine is up.
  $effect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    const runProbe = async () => {
      const ok = await probeFrameTransport();
      if (cancelled) return;
      if (ok) {
        error = null;
        reportFrontendStatus("frame-transport-ok").catch(() => {});
      } else {
        error = "frame transport failed";
        reportFrontendStatus("frame-transport-failed").catch(() => {});
      }
    };

    if (engineReady.get()) {
      void runProbe();
    } else {
      onEngineReady(() => {
        if (!cancelled) void runProbe();
      }).then((u) => {
        unlisten = () => u();
      });
    }

    return () => {
      cancelled = true;
      unlisten?.();
    };
  });

  // Resize → re-request when using a custom view.
  $effect(() => {
    const wrap = wrapEl;
    const open = $imageOpen;
    const ready = $decodeState;
    if (!wrap) return;

    let t: ReturnType<typeof setTimeout> | undefined;
    const obs = new ResizeObserver(() => {
      if (!open || ready !== "ready") return;
      if (!isCustomView(view)) return;
      if (t) clearTimeout(t);
      t = setTimeout(() => void refresh(), 50);
    });
    obs.observe(wrap);
    return () => {
      obs.disconnect();
      if (t) clearTimeout(t);
    };
  });

  // One-shot zoom commands from the chrome bar.
  $effect(() => {
    const nonce = $viewCmdNonce;
    if (nonce === 0 || !imageOpen.get()) return;
    const cmd = viewCmd.get();
    if (cmd === "fit") view = { scale: null, centerX: 0.5, centerY: 0.5 };
    else if (cmd === "oneToOne") view.scale = 1;
    else if (cmd === "zoomIn")
      view.scale = Math.min(8, (effScale || 1) * 1.5);
    else if (cmd === "zoomOut")
      view.scale = Math.max(0.02, (effScale || 1) / 1.5);
    void refresh();
  });
</script>

<GlassPanel
  variant="viewport"
  class="flex min-h-0 flex-1 flex-col overflow-hidden px-[10px] py-[8px]"
>
  <!-- Main Viewport — frame:// JPEG transport (CropOverlay not ported yet) -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={wrapEl}
    role="img"
    aria-label="Develop preview"
    class="relative min-h-0 flex-1 flex items-center justify-center overflow-hidden select-none {$cropActive ? 'cursor-default' : 'cursor-grab active:cursor-grabbing'}"
    onwheel={onWheel}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
    ondblclick={onDoubleClick}
  >
    {#if displaySrc}
      <img
        src={displaySrc}
        alt=""
        draggable={false}
        class="block max-h-full max-w-full object-contain pointer-events-none"
        onerror={() => (error = "frame transport failed")}
      />
    {:else if $imageOpen && $decodeState !== "ready"}
      <div class="flex flex-col items-center justify-center gap-[6px] text-white/30">
        <span class="rounded-[8px] bg-panel-3 px-[10px] py-[6px] text-[11px]">
          {$decodeState === "preview" ? "Preview" : "Loading"}
        </span>
        <span class="text-[10px]">Decoding image…</span>
      </div>
    {:else if !$imageOpen}
      <div class="flex items-center justify-center text-[12px] text-white/25">
        No photo selected
      </div>
    {/if}

    {#if error}
      <div class="absolute text-[11px] text-red-400">{error}</div>
    {/if}
  </div>

  {#if !minimal}
    <!-- Bottom Control Bar -->
    <div
      class="mt-[8px] flex h-[30px] shrink-0 items-center justify-between px-[14px] rounded-[22px] bg-panel-2 border border-white/[0.02] text-[9px] text-white/40 shadow-inner select-none"
    >
      <div class="flex items-center gap-[14px]">
        <button
          type="button"
          onclick={toggleFullscreen}
          title="Toggle Fullscreen"
          aria-label="Toggle Fullscreen"
          class="flex items-center justify-center cursor-pointer text-white/40 hover:text-white/95 transition-colors"
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.3">
            <path d="M3.5 1H1.5V3" stroke-linecap="round" />
            <path d="M8.5 1H10.5V3" stroke-linecap="round" />
            <path d="M3.5 11H1.5V9" stroke-linecap="round" />
            <path d="M8.5 11H10.5V9" stroke-linecap="round" />
          </svg>
        </button>

        <button
          type="button"
          onclick={() => sendViewCmd("oneToOne")}
          class="text-[9px] cursor-pointer transition-colors {$zoomLabel === '100%' ? 'font-bold text-white/95' : 'text-white/40 hover:text-white/80'}"
        >
          1:1
        </button>

        <button
          type="button"
          onclick={() => sendViewCmd("zoomOut")}
          aria-label="Zoom Out"
          class="text-[12px] font-medium cursor-pointer text-white/40 hover:text-white/80 transition-colors"
        >
          -
        </button>

        <button
          type="button"
          onclick={() => sendViewCmd("fit")}
          class="text-[9px] cursor-pointer transition-colors {$zoomLabel === 'fit' ? 'font-medium text-white/85' : 'text-white/40 hover:text-white/80'}"
        >
          fit
        </button>

        <button
          type="button"
          onclick={() => sendViewCmd("zoomIn")}
          aria-label="Zoom In"
          class="text-[10px] font-medium cursor-pointer text-white/40 hover:text-white/80 transition-colors"
        >
          +
        </button>
      </div>

      <div class="flex items-center gap-[14px]">
        {#each LOOKS as look}
          <button
            type="button"
            onclick={() => pickLook(look.value)}
            class="text-[9px] cursor-pointer transition-colors {$displayLook === look.value ? 'font-bold text-white/95' : 'text-white/40 hover:text-white/80'}"
          >
            {look.label}
          </button>
        {/each}

        <button
          type="button"
          onclick={toggleAfter}
          class="flex items-center gap-[5px] text-[9px] cursor-pointer transition-colors {!$previewBypass ? 'text-white/95 font-medium' : 'text-white/40 hover:text-white/80'}"
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.2">
            <rect x="1.5" y="1.5" width="9" height="9" rx="1" />
            <line x1="6" y1="1.5" x2="6" y2="10.5" />
          </svg>
          <span>After</span>
        </button>

        <button
          type="button"
          onclick={() => sendViewCmd("oneToOne")}
          aria-label="Toggle Zoom Loupe"
          class="flex items-center justify-center cursor-pointer transition-colors {$zoomLabel === '100%' ? 'text-white/95' : 'text-white/40 hover:text-white/80'}"
        >
          <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="5" cy="5" r="3.5" />
            <line x1="7.5" y1="7.5" x2="10.5" y2="10.5" stroke-linecap="round" />
          </svg>
        </button>
      </div>
    </div>
  {/if}
</GlassPanel>
