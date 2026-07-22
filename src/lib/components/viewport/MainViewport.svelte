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
  import CropOverlay from "../../../crop/CropOverlay.svelte";
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
    selectedRetouch,
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

  /** Split before/after (phase 12). */
  let compareSplit = $state(false);
  let beforeSrc = $state<string | null>(null);
  let splitRatio = $state(0.5);
  let splitDragging = false;
  let compareBusy = $state(false);

  /** Cursor loupe (magnifier), not full-viewport 1:1. */
  let loupeOn = $state(false);
  let loupePos = $state<{ x: number; y: number } | null>(null);
  let loupeCanvas = $state<HTMLCanvasElement | null>(null);

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
    if (compareSplit) {
      compareSplit = false;
      beforeSrc = null;
    }
    const next = !previewBypass.get();
    previewBypass.set(next);
    void setPreviewBypass(next).catch(() => {});
  }

  function waitForNewerFrame(prev: number, timeoutMs = 3500): Promise<void> {
    return new Promise((resolve) => {
      const t0 = Date.now();
      const id = window.setInterval(() => {
        if (shownVer > prev || Date.now() - t0 > timeoutMs) {
          window.clearInterval(id);
          resolve();
        }
      }, 40);
    });
  }

  function snapshotDisplay(): string | null {
    const img = wrapEl?.querySelector("img.viewport-frame") as HTMLImageElement | null;
    if (!img?.naturalWidth) return null;
    const c = document.createElement("canvas");
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext("2d");
    if (!ctx) return null;
    ctx.drawImage(img, 0, 0);
    return c.toDataURL("image/jpeg", 0.9);
  }

  async function toggleCompare() {
    if (compareBusy) return;
    if (compareSplit) {
      compareSplit = false;
      beforeSrc = null;
      return;
    }
    if (decodeState.get() !== "ready" || !displaySrc) return;
    compareBusy = true;
    try {
      const v0 = shownVer;
      previewBypass.set(true);
      await setPreviewBypass(true);
      await refresh(true);
      await waitForNewerFrame(v0);
      beforeSrc = snapshotDisplay();
      const v1 = shownVer;
      previewBypass.set(false);
      await setPreviewBypass(false);
      await refresh(true);
      await waitForNewerFrame(v1);
      if (beforeSrc) {
        compareSplit = true;
        splitRatio = 0.5;
      }
    } catch {
      beforeSrc = null;
      compareSplit = false;
    } finally {
      compareBusy = false;
    }
  }

  function onSplitPointerDown(e: PointerEvent) {
    e.stopPropagation();
    splitDragging = true;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    moveSplit(e);
  }
  function moveSplit(e: PointerEvent) {
    if (!splitDragging || !wrapEl) return;
    const rect = wrapEl.getBoundingClientRect();
    splitRatio = Math.min(0.92, Math.max(0.08, (e.clientX - rect.left) / rect.width));
  }
  function endSplitDrag() {
    splitDragging = false;
  }

  function toggleLoupe() {
    loupeOn = !loupeOn;
    if (!loupeOn) loupePos = null;
  }

  function paintLoupe(clientX: number, clientY: number) {
    if (!loupeOn || !wrapEl || !loupeCanvas) return;
    const img = wrapEl.querySelector("img.viewport-frame") as HTMLImageElement | null;
    if (!img?.naturalWidth) return;
    const rect = img.getBoundingClientRect();
    if (clientX < rect.left || clientX > rect.right || clientY < rect.top || clientY > rect.bottom) {
      loupePos = null;
      return;
    }
    loupePos = { x: clientX - wrapEl.getBoundingClientRect().left, y: clientY - wrapEl.getBoundingClientRect().top };
    const nx = (clientX - rect.left) / rect.width;
    const ny = (clientY - rect.top) / rect.height;
    const srcX = nx * img.naturalWidth;
    const srcY = ny * img.naturalHeight;
    const zoom = 2.5;
    const size = loupeCanvas.width;
    const half = size / (2 * zoom);
    const ctx = loupeCanvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, size, size);
    ctx.fillStyle = "#111";
    ctx.fillRect(0, 0, size, size);
    ctx.drawImage(
      img,
      srcX - half,
      srcY - half,
      half * 2,
      half * 2,
      0,
      0,
      size,
      size,
    );
    ctx.strokeStyle = "rgba(255,255,255,0.5)";
    ctx.lineWidth = 2;
    ctx.strokeRect(1, 1, size - 2, size - 2);
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
    if (compareSplit && (e.target as HTMLElement).dataset?.splitHandle === "1") {
      onSplitPointerDown(e);
      return;
    }
    const tool = viewportTool.get();
    if (tool === "brush" && (selectedMask.get() || selectedRetouch.get())) {
      const p = toImageCoords(e);
      if (p) brushPoints = [p];
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
      return;
    }
    if (tool === "crop" && cropActive.get()) return;
    if (loupeOn) {
      paintLoupe(e.clientX, e.clientY);
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
      return;
    }
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
    if (splitDragging) {
      moveSplit(e);
      return;
    }
    if (loupeOn) {
      paintLoupe(e.clientX, e.clientY);
      return;
    }
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
    endSplitDrag();
    const tool = viewportTool.get();
    if (tool !== "brush" || brushPoints.length === 0) return;
    const pts = brushPoints;
    brushPoints = [];
    const d = doc.get();
    const stroke = {
      points: pts,
      radius: brushRadius.get(),
      hardness: 0.6,
      mode: "add",
    };

    const retouchId = selectedRetouch.get();
    if (retouchId) {
      const spot = d?.retouch?.find((s) => s.id === retouchId);
      if (spot && spot.source.type === "brush") {
        const strokes = Array.isArray(spot.source.strokes)
          ? [...(spot.source.strokes as unknown[])]
          : [];
        strokes.push(stroke);
        applyOp({
          op: "set_retouch_source",
          id: spot.id,
          source: { type: "brush", strokes },
        })
          .then((delta) => reconcile(delta))
          .catch(() => {});
      }
      return;
    }

    const maskId = selectedMask.get();
    if (!maskId) return;
    const mask = d?.masks?.find((m) => m.id === maskId);
    if (mask && mask.source.type === "brush") {
      const strokes = Array.isArray(mask.source.strokes)
        ? [...(mask.source.strokes as unknown[])]
        : [];
      strokes.push(stroke);
      applyOp({
        op: "set_mask_source",
        id: mask.id,
        source: { type: "brush", strokes },
      })
        .then((delta) => reconcile(delta))
        .catch(() => {});
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
  <!-- Main Viewport — frame:// JPEG transport -->
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
      <div class="relative max-h-full max-w-full">
        <img
          src={displaySrc}
          alt=""
          draggable={false}
          class="viewport-frame block max-h-full max-w-full object-contain pointer-events-none"
          onerror={() => (error = "frame transport failed")}
        />
        {#if compareSplit && beforeSrc}
          <img
            src={beforeSrc}
            alt=""
            draggable={false}
            class="pointer-events-none absolute inset-0 h-full w-full object-contain"
            style="clip-path: inset(0 {(1 - splitRatio) * 100}% 0 0)"
          />
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            data-split-handle="1"
            class="absolute top-0 bottom-0 z-10 w-[12px] -translate-x-1/2 cursor-col-resize"
            style="left: {splitRatio * 100}%"
            onpointerdown={onSplitPointerDown}
          >
            <div class="absolute inset-y-0 left-1/2 w-[2px] -translate-x-1/2 bg-white/85 shadow-[0_0_6px_rgba(0,0,0,0.6)]"></div>
            <div class="absolute top-1/2 left-1/2 flex size-[18px] -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full border border-white/40 bg-black/50 text-[8px] text-white/90">
              ‖
            </div>
          </div>
          <span class="absolute left-2 top-2 rounded bg-black/50 px-1.5 py-0.5 text-[8px] text-white/70">Before</span>
          <span class="absolute right-2 top-2 rounded bg-black/50 px-1.5 py-0.5 text-[8px] text-white/70">After</span>
        {/if}
      </div>
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

    {#if $cropActive && displaySrc}
      <CropOverlay {wrapEl} />
    {/if}

    {#if loupeOn}
      <canvas
        bind:this={loupeCanvas}
        width={140}
        height={140}
        class="pointer-events-none absolute z-20 rounded-full border border-white/40 shadow-lg {loupePos ? '' : 'opacity-0'}"
        style={loupePos
          ? `left: ${loupePos.x + 16}px; top: ${loupePos.y + 16}px; width: 140px; height: 140px;`
          : "left: 0; top: 0; width: 140px; height: 140px;"}
      ></canvas>
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
          class="flex items-center gap-[5px] text-[9px] cursor-pointer transition-colors {!$previewBypass && !compareSplit ? 'text-white/95 font-medium' : 'text-white/40 hover:text-white/80'}"
          title="Toggle full before/after"
        >
          <span>{$previewBypass ? "Before" : "After"}</span>
        </button>

        <button
          type="button"
          onclick={() => void toggleCompare()}
          disabled={compareBusy}
          class="flex items-center gap-[5px] text-[9px] cursor-pointer transition-colors {compareSplit ? 'text-white/95 font-medium' : 'text-white/40 hover:text-white/80'}"
          title="Split before/after"
          aria-pressed={compareSplit}
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.2">
            <rect x="1.5" y="1.5" width="9" height="9" rx="1" />
            <line x1="6" y1="1.5" x2="6" y2="10.5" />
          </svg>
          <span>Split</span>
        </button>

        <button
          type="button"
          onclick={toggleLoupe}
          aria-label="Toggle Zoom Loupe"
          aria-pressed={loupeOn}
          title="Cursor loupe (2.5×)"
          class="flex items-center justify-center cursor-pointer transition-colors {loupeOn ? 'text-white/95' : 'text-white/40 hover:text-white/80'}"
        >
          <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="5" cy="5" r="3.5" />
            <line x1="7.5" y1="7.5" x2="10.5" y2="10.5" stroke-linecap="round" />
          </svg>
        </button>

        <span class="text-[9px] text-white/35 tabular-nums">{$zoomLabel}</span>
      </div>
    </div>
  {/if}
</GlassPanel>
