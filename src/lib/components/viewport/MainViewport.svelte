<script lang="ts">
  import { onMount } from "svelte";
  import GlassPanel from "../primitives/GlassPanel.svelte";
  import ContextMenu from "../primitives/ContextMenu.svelte";
  import type { ContextMenuItem } from "../primitives/ContextMenu.svelte";
  import { isExportOpen } from "../../../stores/ui";
  import { shortcutLabels } from "../../shortcuts";
  import {
    applyOp,
    reportFrontendStatus,
    requestFrame,
    sampleColor,
    setClipWarnings,
    setDisplayLook,
    setPreviewBypass,
    setProofTarget,
    wbFromPoint,
  } from "../../../ipc/commands";
  import { onEngineReady, onFrameReady } from "../../../ipc/events";
  import CropOverlay from "../../../crop/CropOverlay.svelte";
  import VideoScrubber from "./VideoScrubber.svelte";
  import {
    containRect,
    contentDims,
    contentNormToImageNorm,
    imageNormToContentNorm,
    cropModeFor,
    readCropFromDoc,
  } from "../../../crop/cropMath";
  import { preloadFrame, probeFrameTransport } from "../../engine/frame";
  import { axesDisagree, hintDestDims, openingRotateDeg } from "../../previewOrient";
  import {
    cropActive,
    decodeState,
    displayLook,
    engineReady,
    imageDims,
    imageMeta,
    imageOpen,
    lastOpenedPath,
    openingPreviewHint,
    openingPreviewUrl,
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
  import {
    brushFlow,
    brushHardness,
    beginMaskAdjust,
    deselectMask,
    endMaskAdjust,
    markMaskPending,
    maskAdjusting,
    maskRefineMode,
    objectPickActive,
    instancePickKind,
    colorPickActive,
    geomPlacementKind,
    stopGeomPlacement,
    maskOverlayVisible,
    syncMaskOverlay,
    syncViewportToolForMask,
  } from "../../../stores/mask";
  import { rightPanelMode } from "../../../stores/editor";
  import MaskGeometryOverlay from "./MaskGeometryOverlay.svelte";
  import ObjectPickOverlay from "./ObjectPickOverlay.svelte";
  import { doc, reconcile } from "../../../stores/doc";
  import { setWorkspace, workspace } from "../../../stores/workspace";

  let { minimal = false }: { minimal?: boolean } = $props();

  let maskTapStart: { x: number; y: number } | null = null;
  const MASK_TAP_PX = 6;

  function maskPanelClickOffEnabled(): boolean {
    return (
      rightPanelMode.get() === "mask" &&
      !!selectedMask.get() &&
      !objectPickActive.get() &&
      !colorPickActive.get() &&
      !geomPlacementKind.get()
    );
  }

  function rgbToRange(r: number, g: number, b: number) {
    const mx = Math.max(r, g, b);
    const mn = Math.min(r, g, b);
    const chroma = mx - mn;
    let hue = 0;
    if (chroma > 1e-5) {
      if (mx === r) hue = (g - b) / chroma;
      else if (mx === g) hue = 2 + (b - r) / chroma;
      else hue = 4 + (r - g) / chroma;
      hue *= 60;
      if (hue < 0) hue += 360;
    }
    const luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    return { hue, chroma, luma };
  }

  async function applyColorSample(x: number, y: number) {
    try {
      let mask = doc.get()?.masks?.find((m) => m.id === selectedMask.get());
      if (!mask || mask.kind !== "parametric") {
        const delta = await applyOp({
          op: "add_mask",
          kind: "parametric",
          source: {
            type: "parametric",
            luma_lo: 0,
            luma_hi: 1,
            chroma_lo: 0,
            chroma_hi: 1,
            hue_lo: 0,
            hue_hi: 360,
            softness: 0.08,
          },
        });
        reconcile(delta);
        if (delta.newMaskId) {
          selectedMask.set(delta.newMaskId);
          mask = delta.doc.masks?.find((m) => m.id === delta.newMaskId);
        }
      }
      if (!mask) return;
      const c = await sampleColor(x, y);
      const [dr, dg, db] = c.display.map((v) => v / 255);
      const { hue, chroma, luma } = rgbToRange(dr, dg, db);
      const source = {
        type: "parametric",
        luma_lo: Math.max(0, luma - 0.12),
        luma_hi: Math.min(1, luma + 0.12),
        chroma_lo: Math.max(0, chroma - 0.08),
        chroma_hi: Math.min(1, chroma + 0.2),
        hue_lo: (hue - 22 + 360) % 360,
        hue_hi: (hue + 22) % 360,
        softness: 0.06,
      };
      // hue_hi < hue_lo is a wrap-around band; shader supports it.
      reconcile(await applyOp({ op: "set_mask_source", id: mask.id, source }));
      syncMaskOverlay();
    } catch {
      /* ignore */
    }
  }

  async function createInstanceMask(x: number, y: number) {
    const kind = instancePickKind.get();
    const model =
      kind === "people" ? "people_v1" : kind === "subject" ? "subject_v1" : "object_v1";
    const source = {
      type: "segmented",
      model,
      hint: { point: [x, y] },
    };
    try {
      const delta = await applyOp({ op: "add_mask", kind, source });
      reconcile(delta);
      if (delta.newMaskId) {
        selectedRetouch.set(null);
        selectedMask.set(delta.newMaskId);
        const m = delta.doc.masks?.find((entry) => entry.id === delta.newMaskId);
        syncViewportToolForMask(m ?? null);
        markMaskPending(delta.newMaskId);
        syncMaskOverlay();
      }
    } catch {
      /* ignore */
    }
  }

  /** In-progress drag for linear/radial placement. */
  let geomDraft = $state<{
    kind: "linear" | "radial";
    start: [number, number];
    end: [number, number];
  } | null>(null);

  async function commitGeomPlacement(
    kind: "linear" | "radial",
    start: [number, number],
    end: [number, number],
  ) {
    const dx = end[0] - start[0];
    const dy = end[1] - start[1];
    const dist = Math.hypot(dx, dy);
    if (dist < 0.01) {
      stopGeomPlacement();
      geomDraft = null;
      return;
    }
    let source: Record<string, unknown>;
    if (kind === "linear") {
      source = { type: "linear", start, end };
    } else {
      const rx = Math.max(0.04, Math.abs(dx));
      const ry = Math.max(0.04, Math.abs(dy));
      source = {
        type: "radial",
        center: start,
        radii: [rx, ry],
        rotation: 0,
      };
    }
    try {
      const delta = await applyOp({ op: "add_mask", kind, source });
      reconcile(delta);
      stopGeomPlacement();
      geomDraft = null;
      if (delta.newMaskId) {
        selectedRetouch.set(null);
        selectedMask.set(delta.newMaskId);
        const m = delta.doc.masks?.find((entry) => entry.id === delta.newMaskId);
        syncViewportToolForMask(m ?? null);
        maskOverlayVisible.set(true);
        syncMaskOverlay();
      }
    } catch {
      stopGeomPlacement();
      geomDraft = null;
    }
  }

  const isVideoWs = $derived($workspace === "video");
  const imgAspect = $derived(
    $imageDims && $imageDims.h > 0 ? $imageDims.w / $imageDims.h : 1,
  );
  const mediaMismatch = $derived(
    $imageOpen &&
      ((isVideoWs && $imageMeta?.kind !== "video") ||
        (!isVideoWs && $imageMeta?.kind === "video")),
  );
  const selectedMaskEnabled = $derived.by(() => {
    if (!$selectedMask) return false;
    const m = $doc?.masks?.find((x) => x.id === $selectedMask);
    return m?.enabled !== false;
  });

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
  let imgBox = $state<{ left: number; top: number; width: number; height: number } | null>(
    null,
  );
  let displaySrc = $state<string | null>(null);
  let previewNatural = $state<{ w: number; h: number } | null>(null);
  const destDims = $derived($imageDims ?? hintDestDims($openingPreviewHint));
  const openingRotate = $derived.by(() => {
    if (displaySrc) return 0;
    const dest = destDims;
    const nat = previewNatural;
    if (!dest || !nat) return 0;
    if (!axesDisagree(nat.w, nat.h, dest.w, dest.h)) return 0;
    return openingRotateDeg($imageMeta?.orientation ?? $openingPreviewHint?.orientation);
  });
  const catalogReady = $derived(
    !!displaySrc ||
      !!(
        previewNatural &&
        (destDims || $openingPreviewHint?.orientation === "Normal")
      ),
  );
  const viewportSrc = $derived(displaySrc ?? $openingPreviewUrl);
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
  let blinkies = $state(false);
  let proofSpace = $state(0);
  let proofGamut = $state(false);
  const PROOF_LABELS = ["Proof", "sRGB", "P3", "Adobe", "ProPhoto"];

  /** Cursor loupe (magnifier), not full-viewport 1:1. */
  let loupeOn = $state(false);
  let loupePos = $state<{ x: number; y: number } | null>(null);
  let loupeCanvas = $state<HTMLCanvasElement | null>(null);
  let chromeVisible = $state(true);
  let chromeTimer: ReturnType<typeof setTimeout> | undefined;

  function bumpChrome() {
    chromeVisible = true;
    if (chromeTimer) clearTimeout(chromeTimer);
    chromeTimer = setTimeout(() => {
      chromeVisible = false;
    }, 1800);
  }

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

  function measureImageBox(wrap: HTMLElement) {
    const img = wrap.querySelector("img.viewport-frame");
    if (!img) {
      imgBox = null;
      return;
    }
    // Engine frames are wrap-sized with letterbox around the photo. Size the
    // crop overlay to the contained photo, not the workspace.
    const content = viewContentDims();
    if (cropActive.get() && content && wrap.clientWidth >= 8 && wrap.clientHeight >= 8) {
      imgBox = containRect(wrap.clientWidth, wrap.clientHeight, content.w, content.h);
      return;
    }
    const wr = wrap.getBoundingClientRect();
    const ir = img.getBoundingClientRect();
    imgBox = {
      left: ir.left - wr.left,
      top: ir.top - wr.top,
      width: ir.width,
      height: ir.height,
    };
  }

  $effect(() => {
    const wrap = wrapEl;
    const src = viewportSrc;
    const _cropOn = $cropActive;
    const _dims = $imageDims;
    const _modules = $doc?.modules?.crop;
    if (!wrap || !src) {
      imgBox = null;
      return;
    }
    measureImageBox(wrap);
    const obs = new ResizeObserver(() => measureImageBox(wrap));
    obs.observe(wrap);
    const img = wrap.querySelector("img.viewport-frame");
    if (img) obs.observe(img);
    return () => obs.disconnect();
  });

  function screenToOriginalNorm(
    clientX: number,
    clientY: number,
    wrap: HTMLElement,
    scale: number,
    v: ViewState,
    clampToImage = true,
  ): [number, number] | null {
    const dims = imageDims.get();
    if (!dims || !imgBox || imgBox.width < 4 || imgBox.height < 4) return null;
    const crop = readCropFromDoc(doc.get()?.modules);
    const mode = cropModeFor(crop, cropActive.get());
    const [cw, ch] = contentDims(crop, dims.w, dims.h, mode);
    const rect = wrap.getBoundingClientRect();
    const dpr = window.devicePixelRatio;
    const localX = clientX - rect.left;
    const localY = clientY - rect.top;
    if (
      clampToImage &&
      (localX < imgBox.left ||
        localY < imgBox.top ||
        localX > imgBox.left + imgBox.width ||
        localY > imgBox.top + imgBox.height)
    ) {
      return null;
    }
    const fu = (localX - imgBox.left) / imgBox.width;
    const fv = (localY - imgBox.top) / imgBox.height;
    const nx = v.centerX + ((fu - 0.5) * imgBox.width * dpr) / (scale * cw);
    const ny = v.centerY + ((fv - 0.5) * imgBox.height * dpr) / (scale * ch);
    if (mode === 0) return [nx, ny];
    const mapped = contentNormToImageNorm(nx, ny, crop, dims.w, dims.h, mode);
    return mapped ?? (clampToImage ? null : [nx, ny]);
  }

  /** Mask geometry: map screen → image norm, extrapolating outside the image bounds. */
  function imageNormFromScreen(clientX: number, clientY: number): [number, number] | null {
    if (!wrapEl) return null;
    return screenToOriginalNorm(clientX, clientY, wrapEl, effScale, view, false);
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
    // While open_image is still resolving metadata, a queued frame event can
    // only belong to the outgoing photo. Keep the new catalog preview visible.
    if (openingPreviewUrl.get() && imageMeta.get() === null) return;
    if (version <= shownVer) return;
    pendingVer = version;
    preloadFrame(version)
      .then((url) => {
        if (pendingVer !== version) return;
        shownVer = version;
        displaySrc = url;
        openingPreviewUrl.set(null);
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
        void refresh(forceNext);
      }
    }
  }

  function toImageCoords(e: { clientX: number; clientY: number }): [number, number] | null {
    if (!wrapEl) return null;
    return screenToOriginalNorm(e.clientX, e.clientY, wrapEl, effScale, view);
  }

  /** Image-normalized coords (mask space) → CSS position inside viewport wrap. */
  function imageNormToLocal(ix: number, iy: number): { x: number; y: number } | null {
    const dims = imageDims.get();
    if (!dims || !imgBox || imgBox.width < 4 || imgBox.height < 4) return null;
    const crop = readCropFromDoc(doc.get()?.modules);
    const mode = cropModeFor(crop, cropActive.get());
    const [cw, ch] = contentDims(crop, dims.w, dims.h, mode);
    const dpr = window.devicePixelRatio;
    let nx = ix;
    let ny = iy;
    if (mode !== 0) {
      const mapped = imageNormToContentNorm(ix, iy, crop, dims.w, dims.h, mode);
      if (!mapped) return null;
      nx = mapped[0];
      ny = mapped[1];
    }
    return {
      x: imgBox.left + imgBox.width / 2 + ((nx - view.centerX) * cw * effScale) / dpr,
      y: imgBox.top + imgBox.height / 2 + ((ny - view.centerY) * ch * effScale) / dpr,
    };
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

  function toggleBlinkies() {
    blinkies = !blinkies;
    void setClipWarnings(blinkies, blinkies).catch(() => {});
  }

  function cycleProof() {
    proofSpace = (proofSpace + 1) % 5;
    if (proofSpace === 0) proofGamut = false;
    void setProofTarget(proofSpace, proofGamut).catch(() => {});
  }

  function toggleGamut() {
    if (proofSpace === 0) proofSpace = 1;
    proofGamut = !proofGamut;
    void setProofTarget(proofSpace, proofGamut).catch(() => {});
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
    if (cropActive.get()) return;
    const factor = Math.exp(-e.deltaY * 0.0015);
    view.scale = Math.min(8, Math.max(0.02, effScale * factor));
    void refresh();
  }

  function onPointerDown(e: PointerEvent) {
    if (compareSplit && (e.target as HTMLElement).dataset?.splitHandle === "1") {
      onSplitPointerDown(e);
      return;
    }
    // Lightroom-style linear/radial: drag on image to place a NEW mask.
    const placeKind = geomPlacementKind.get();
    if (placeKind && e.button === 0) {
      const p = toImageCoords(e);
      if (p) {
        geomDraft = { kind: placeKind, start: p, end: p };
        beginMaskAdjust();
        (e.currentTarget as Element).setPointerCapture(e.pointerId);
      }
      return;
    }
    if (maskAdjusting.get()) return;
    const tool = viewportTool.get();
    if (tool === "mask-geo" && e.button !== 1 && e.button !== 0) return;
    if (tool === "brush" && (selectedMask.get() || selectedRetouch.get())) {
      const p = toImageCoords(e);
      if (p) {
        brushPoints = [p];
        if (selectedMask.get()) beginMaskAdjust();
      }
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
    if ((objectPickActive.get() || colorPickActive.get()) && e.button === 0) {
      maskTapStart = { x: e.clientX, y: e.clientY };
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
      return;
    }
    if (maskPanelClickOffEnabled() && e.button === 0) {
      maskTapStart = { x: e.clientX, y: e.clientY };
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
      return;
    }
    // mask-geo: only pan with middle mouse; left-click is for handles / click-off above
    if (tool === "mask-geo" && e.button !== 1) return;
    dragging = { x: e.clientX, y: e.clientY };
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (splitDragging) {
      moveSplit(e);
      return;
    }
    if (geomDraft) {
      const p = toImageCoords(e);
      if (p) geomDraft = { ...geomDraft, end: p };
      return;
    }
    if (maskTapStart && !dragging) {
      const dx = e.clientX - maskTapStart.x;
      const dy = e.clientY - maskTapStart.y;
      if (dx * dx + dy * dy > MASK_TAP_PX * MASK_TAP_PX) {
        dragging = { x: maskTapStart.x, y: maskTapStart.y };
        maskTapStart = null;
      } else {
        return;
      }
    }
    if (maskAdjusting.get()) return;
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

  function onPointerUp(e: PointerEvent) {
    if (geomDraft) {
      const draft = geomDraft;
      geomDraft = null;
      endMaskAdjust();
      void commitGeomPlacement(draft.kind, draft.start, draft.end);
      return;
    }
    if (maskTapStart) {
      const dx = e.clientX - maskTapStart.x;
      const dy = e.clientY - maskTapStart.y;
      if (dx * dx + dy * dy <= MASK_TAP_PX * MASK_TAP_PX) {
        if (objectPickActive.get()) {
          const p = toImageCoords(e);
          if (p) void createInstanceMask(p[0], p[1]);
          // Stay in pick mode so multiple subjects/people can be selected.
        } else if (colorPickActive.get()) {
          const p = toImageCoords(e);
          if (p) void applyColorSample(p[0], p[1]);
          colorPickActive.set(false);
        } else {
          deselectMask();
        }
      }
      maskTapStart = null;
    }
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
      hardness: brushHardness.get(),
      flow: brushFlow.get(),
      density: brushFlow.get(),
      amount: brushFlow.get(),
      mode: maskRefineMode.get(),
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
    if (!mask) return;

    if (mask.source.type === "brush") {
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
        .catch(() => {})
        .finally(() => endMaskAdjust());
      return;
    }

    if (mask.source.type === "composite") {
      const comps = [...(mask.source.components as Record<string, unknown>[])];
      const last = comps[comps.length - 1];
      if (
        last?.source &&
        (last.source as { type?: string }).type === "brush" &&
        last.op === maskRefineMode.get()
      ) {
        const src = last.source as { strokes?: unknown[] };
        const strokes = Array.isArray(src.strokes) ? [...src.strokes] : [];
        strokes.push(stroke);
        comps[comps.length - 1] = {
          ...last,
          source: { type: "brush", strokes },
        };
        applyOp({
          op: "set_mask_source",
          id: mask.id,
          source: { type: "composite", components: comps },
        })
          .then((delta) => reconcile(delta))
          .catch(() => {})
          .finally(() => endMaskAdjust());
      } else {
        applyOp({
          op: "add_mask_component",
          id: mask.id,
          mode: maskRefineMode.get(),
          source: { type: "brush", strokes: [stroke] },
        })
          .then((delta) => reconcile(delta))
          .catch(() => {})
          .finally(() => endMaskAdjust());
      }
      return;
    }

    // Refine AI / gradient masks with brush add/subtract.
    applyOp({
      op: "add_mask_component",
      id: mask.id,
      mode: maskRefineMode.get(),
      source: { type: "brush", strokes: [stroke] },
    })
      .then((delta) => reconcile(delta))
      .catch(() => {})
      .finally(() => endMaskAdjust());
  }

  function onDoubleClick() {
    if (!imageOpen.get()) return;
    if (view.scale === null) view.scale = 1;
    else view = { scale: null, centerX: 0.5, centerY: 0.5 };
    void refresh();
  }

  // Global "Z" shortcut (see lib/shortcuts.ts) — toggle between fit and 1:1.
  function onToggleZoomShortcut() {
    if (!imageOpen.get()) return;
    sendViewCmd(view.scale === null ? "oneToOne" : "fit");
  }

  // Global "\" shortcut (see lib/shortcuts.ts) — before/after preview toggle
  // is this app's real equivalent of a "compare" view.
  function onToggleCompareShortcut() {
    toggleAfter();
  }

  onMount(() => {
    window.addEventListener("meraraw:toggle-zoom", onToggleZoomShortcut);
    window.addEventListener("meraraw:toggle-compare", onToggleCompareShortcut);
    return () => {
      window.removeEventListener("meraraw:toggle-zoom", onToggleZoomShortcut);
      window.removeEventListener("meraraw:toggle-compare", onToggleCompareShortcut);
    };
  });

  // Right-click context menu.
  let ctxMenu = $state<{ x: number; y: number } | null>(null);

  function handleContextMenu(e: MouseEvent) {
    if (!imageOpen.get()) return;
    e.preventDefault();
    e.stopPropagation();
    ctxMenu = { x: e.clientX, y: e.clientY };
  }

  const ctxItems = $derived<ContextMenuItem[]>([
    {
      type: "item",
      label: view.scale === null ? "Zoom to 1:1" : "Zoom to fit",
      shortcut: shortcutLabels.zoom,
      onclick: onToggleZoomShortcut,
    },
    {
      type: "item",
      label: $previewBypass ? "Show after" : "Show before",
      shortcut: shortcutLabels.compare,
      onclick: toggleAfter,
    },
    { type: "separator" },
    {
      type: "item",
      label: "Export…",
      shortcut: shortcutLabels.export,
      onclick: () => isExportOpen.set(true),
    },
    {
      type: "item",
      label: "Copy to clipboard",
      disabled: true,
      onclick: () => {},
    },
  ]);

  // Reset view when a new image opens.
  $effect(() => {
    const _path = $lastOpenedPath;
    view = { scale: null, centerX: 0.5, centerY: 0.5 };
    shownVer = 0;
    pendingVer = 0;
    displaySrc = null;
  });

  // Catalog selections have an already-generated preview. Clear the outgoing
  // engine frame as soon as that preview is available so switching photos is
  // immediate instead of waiting for RAW preview extraction.
  $effect(() => {
    const preview = $openingPreviewUrl;
    previewNatural = null;
    if (!preview) return;
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

  // Resize (panel drag, collapse/expand, window resize) → re-request the
  // frame at the new container size so it stays crisp instead of the
  // browser just CSS-stretching a stale bitmap. Applies in Fit mode too,
  // not just when the user has a custom zoom/pan.
  $effect(() => {
    const wrap = wrapEl;
    const open = $imageOpen;
    const ready = $decodeState;
    if (!wrap) return;

    let t: ReturnType<typeof setTimeout> | undefined;
    const obs = new ResizeObserver(() => {
      if (!open || ready !== "ready") return;
      if (t) clearTimeout(t);
      // Wait for the drag to actually settle before re-fetching — CSS
      // (object-contain) already scales the current bitmap smoothly for
      // every intermediate size, so a mid-drag round trip would just
      // stutter the live resize for no visual benefit.
      t = setTimeout(() => void refresh(true), 300);
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
    if (cropActive.get() && cmd !== "fit") return;
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
  class="relative flex h-full min-h-0 min-w-0 w-full flex-1 flex-col overflow-hidden"
  onpointermove={bumpChrome}
  onpointerleave={() => (chromeVisible = false)}
  onpointerenter={bumpChrome}
>
  {#if mediaMismatch}
    <div class="mismatch">
      {#if $imageMeta?.kind === "video"}
        <span>Video grading is coming soon.</span>
      {:else}
        <span>This file is a still.</span>
        <button type="button" class="tb-btn" onclick={() => setWorkspace("photo")}>Open in Photo Editor</button>
      {/if}
    </div>
  {/if}
  <!-- Main Viewport — frame:// JPEG transport -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={wrapEl}
    role="img"
    aria-label="Develop preview"
    class="viewport-surround relative flex min-h-0 min-w-0 flex-1 items-center justify-center select-none {$objectPickActive || $colorPickActive || $geomPlacementKind ? 'cursor-crosshair overflow-hidden' : $viewportTool === 'mask-geo' ? 'overflow-visible cursor-default' : $cropActive ? 'cursor-default overflow-hidden' : 'cursor-grab active:cursor-grabbing overflow-hidden'}"
    onwheel={onWheel}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
    ondblclick={onDoubleClick}
    oncontextmenu={handleContextMenu}
  >
    {#if viewportSrc}
      <div class="absolute inset-0 flex items-center justify-center">
        {#if openingRotate && destDims}
          <div
            class="opening-fit"
            style="aspect-ratio: {destDims.w} / {destDims.h}; --opening-scale: {Math.max(
              destDims.w / destDims.h,
              destDims.h / destDims.w,
            )};"
          >
            <img
              src={viewportSrc}
              alt=""
              draggable={false}
              class="viewport-frame opening-fit-img pointer-events-none"
              style="transform: rotate({openingRotate}deg) scale(var(--opening-scale)); visibility: {catalogReady
                ? 'visible'
                : 'hidden'};"
              onload={(e) => {
                const img = e.currentTarget as HTMLImageElement;
                if (img.naturalWidth > 1 && img.naturalHeight > 1) {
                  previewNatural = { w: img.naturalWidth, h: img.naturalHeight };
                }
              }}
              onerror={() => (error = "frame transport failed")}
            />
          </div>
        {:else}
          <img
            src={viewportSrc}
            alt=""
            draggable={false}
            class="viewport-frame block object-contain pointer-events-none {displaySrc
              ? 'max-h-full max-w-full'
              : 'h-full w-full'}"
            style="visibility: {catalogReady ? 'visible' : 'hidden'}"
            onload={(e) => {
              const img = e.currentTarget as HTMLImageElement;
              if (img.naturalWidth > 1 && img.naturalHeight > 1) {
                previewNatural = { w: img.naturalWidth, h: img.naturalHeight };
              }
            }}
            onerror={() => (error = "frame transport failed")}
          />
        {/if}
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
            <div class="absolute inset-y-0 left-1/2 w-[2px] -translate-x-1/2 bg-fg shadow-[0_0_6px_rgba(0,0,0,0.6)]"></div>
            <div class="absolute top-1/2 left-1/2 flex size-[18px] -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full border border-fg bg-black/50 text-[8px] text-fg">
              ‖
            </div>
          </div>
          <span class="absolute left-2 top-2 rounded bg-black/50 px-1.5 py-0.5 text-[8px] text-secondary">Before</span>
          <span class="absolute right-2 top-2 rounded bg-black/50 px-1.5 py-0.5 text-[8px] text-secondary">After</span>
        {/if}
      </div>
    {:else if $imageOpen && $decodeState !== "ready"}
      <div class="flex flex-col items-center justify-center gap-[6px] text-subtle">
        <span class="rounded-[8px] bg-sunken px-[10px] py-[6px] text-[11px]">
          {$decodeState === "preview" ? "Preview" : "Loading"}
        </span>
        <span class="text-[10px]">Decoding image…</span>
      </div>
    {:else if !$imageOpen}
      <div class="flex items-center justify-center text-[12px] text-subtle">
        No photo selected
      </div>
    {/if}

    {#if $cropActive && displaySrc && !isVideoWs && imgBox}
      <CropOverlay photoBox={imgBox} />
    {/if}

    {#if displaySrc && $selectedMask && selectedMaskEnabled && !isVideoWs}
      <MaskGeometryOverlay
        {wrapEl}
        {imgAspect}
        toImageCoords={imageNormFromScreen}
        {imageNormToLocal}
      />
    {/if}

    {#if displaySrc && $objectPickActive && !isVideoWs}
      <ObjectPickOverlay
        {imgBox}
        {imageNormToLocal}
        onPick={(centroid) => {
          void createInstanceMask(centroid[0], centroid[1]);
        }}
      />
    {/if}

    {#if geomDraft && imgBox}
      {@const a = imageNormToLocal(geomDraft.start[0], geomDraft.start[1])}
      {@const b = imageNormToLocal(geomDraft.end[0], geomDraft.end[1])}
      {#if a && b}
        <svg class="geom-draft" aria-hidden="true">
          {#if geomDraft.kind === "linear"}
            <line x1={a.x} y1={a.y} x2={b.x} y2={b.y} class="draft-line" />
            <circle cx={a.x} cy={a.y} r="6" class="draft-handle" />
            <circle cx={b.x} cy={b.y} r="6" class="draft-handle" />
          {:else}
            {@const rx = Math.max(4, Math.abs(b.x - a.x))}
            {@const ry = Math.max(4, Math.abs(b.y - a.y))}
            <ellipse cx={a.x} cy={a.y} rx={rx} ry={ry} class="draft-ring" />
            <circle cx={a.x} cy={a.y} r="6" class="draft-handle" />
          {/if}
        </svg>
      {/if}
    {/if}

    {#if loupeOn}
      <canvas
        bind:this={loupeCanvas}
        width={140}
        height={140}
        class="pointer-events-none absolute z-20 rounded-full border border-fg shadow-lg {loupePos ? '' : 'opacity-0'}"
        style={loupePos
          ? `left: ${loupePos.x + 16}px; top: ${loupePos.y + 16}px; width: 140px; height: 140px;`
          : "left: 0; top: 0; width: 140px; height: 140px;"}
      ></canvas>
    {/if}

    {#if error}
      <div class="absolute text-[11px] text-red-400">{error}</div>
    {/if}
  </div>

  {#if $imageMeta?.kind === "video"}
    <VideoScrubber />
  {/if}

  {#if !minimal}
    <div class="viewer-toolbar" class:is-dim={!chromeVisible}>
      <button type="button" onclick={() => sendViewCmd("zoomOut")} aria-label="Zoom Out" class="tb-btn">−</button>
      <button type="button" onclick={() => sendViewCmd("fit")} class="tb-btn" class:is-on={$zoomLabel === "fit"}>Fit</button>
      <button type="button" onclick={() => sendViewCmd("zoomIn")} aria-label="Zoom In" class="tb-btn">+</button>
      <button type="button" onclick={() => sendViewCmd("oneToOne")} class="tb-btn" class:is-on={$zoomLabel === "100%"}>100%</button>
      <span class="tb-sep"></span>
      <button type="button" onclick={toggleAfter} class="tb-btn" class:is-on={!$previewBypass && !compareSplit}>
        {$previewBypass ? "Before" : "After"}
      </button>
      <button type="button" onclick={() => void toggleCompare()} disabled={compareBusy} class="tb-btn" class:is-on={compareSplit}>
        Compare
      </button>
      <button type="button" onclick={toggleBlinkies} class="tb-btn" class:is-on={blinkies}>
        Blinkies
      </button>
      <button type="button" onclick={cycleProof} class="tb-btn" class:is-on={proofSpace > 0}>
        {PROOF_LABELS[proofSpace]}
      </button>
      <button type="button" onclick={toggleGamut} class="tb-btn" class:is-on={proofGamut} disabled={proofSpace === 0 && !proofGamut}>
        Gamut
      </button>
      <span class="tb-sep"></span>
      {#each LOOKS as look}
        <button type="button" onclick={() => pickLook(look.value)} class="tb-btn" class:is-on={$displayLook === look.value}>
          {look.label}
        </button>
      {/each}
      <span class="num tb-zoom">{$zoomLabel}</span>
    </div>
  {/if}
</GlassPanel>

{#if ctxMenu}
  <ContextMenu
    x={ctxMenu.x}
    y={ctxMenu.y}
    items={ctxItems}
    onclose={() => (ctxMenu = null)}
  />
{/if}

<style>
  .opening-fit {
    position: relative;
    height: 100%;
    width: auto;
    max-width: 100%;
    max-height: 100%;
    overflow: hidden;
  }
  .opening-fit-img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    transform-origin: center;
  }
  .geom-draft {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 45;
    overflow: visible;
  }
  .draft-line,
  .draft-ring {
    fill: none;
    stroke: rgba(255, 90, 90, 0.95);
    stroke-width: 2;
    stroke-dasharray: 6 4;
  }
  .draft-handle {
    fill: #fff;
    stroke: rgba(255, 80, 80, 0.95);
    stroke-width: 2;
  }
  .mismatch {
    display: flex;
    flex: none;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    font-size: 12px;
    color: var(--color-fg);
    background: var(--color-sunken);
    border-bottom: 1px solid var(--color-border);
  }
</style>
