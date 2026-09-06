<script lang="ts">
  import { onDestroy } from "svelte";
  import { cropDraft, flushCropDraft, setCropDraft } from "./cropSession";
  import {
    aspectRatio,
    cropParamsEqual,
    cropWithDraft,
    dragHandle,
    type CropParams,
    type HandleId,
  } from "./cropMath";
  import { doc } from "../stores/doc";
  import { imageDims } from "../stores/app";

  let {
    photoBox,
  }: {
    photoBox: { left: number; top: number; width: number; height: number };
  } = $props();

  const crop = $derived(cropWithDraft($doc?.modules, $cropDraft));
  const dims = $derived($imageDims);

  type Drag =
    | { kind: "handle"; handle: HandleId; start: CropParams; x: number; y: number }
    | { kind: "move"; start: CropParams; x: number; y: number };

  let drag = $state<Drag | null>(null);
  let atX = $state(0);
  let atY = $state(0);
  let fromCenter = $state(false);
  let tempAspect = $state(false);
  let committed = false;

  $effect(() => {
    if (!drag) return;
    const move = (e: PointerEvent) => onPointerMove(e);
    const up = (e: PointerEvent) => finish(e);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
    };
  });

  onDestroy(() => {
    void flushCropDraft();
  });

  function previewCrop(): CropParams {
    const d = drag;
    if (!d || photoBox.width < 4 || photoBox.height < 4) return crop;
    const imgW = dims?.w ?? 1000;
    const imgH = dims?.h ?? 1000;
    const dx = (atX - d.x) / photoBox.width;
    const dy = (atY - d.y) / photoBox.height;
    if (d.kind === "move") {
      const r = d.start.rect;
      const w = r.right - r.left;
      const h = r.bottom - r.top;
      const left = Math.max(0, Math.min(1 - w, r.left + dx));
      const top = Math.max(0, Math.min(1 - h, r.top + dy));
      return { ...d.start, rect: { left, top, right: left + w, bottom: top + h } };
    }
    const ratio =
      d.start.aspectLocked && d.start.aspectW > 0 && d.start.aspectH > 0
        ? d.start.aspectW / d.start.aspectH
        : aspectRatio(d.start.rect, imgW, imgH);
    return {
      ...d.start,
      rect: dragHandle(d.start.rect, d.handle, dx, dy, {
        imgW,
        imgH,
        aspectLocked: d.start.aspectLocked,
        aspectRatio: ratio,
        fromCenter,
        tempAspect,
      }),
    };
  }

  const shown = $derived(previewCrop());

  const frame = $derived.by(() => {
    if (photoBox.width < 4 || photoBox.height < 4) return null;
    const r = shown.rect;
    return {
      left: r.left * photoBox.width,
      top: r.top * photoBox.height,
      width: Math.max((r.right - r.left) * photoBox.width, 1),
      height: Math.max((r.bottom - r.top) * photoBox.height, 1),
    };
  });

  const handles: { id: HandleId; cls: string; cursor: string }[] = [
    { id: "nw", cls: "h-nw", cursor: "nwse-resize" },
    { id: "n", cls: "h-n", cursor: "ns-resize" },
    { id: "ne", cls: "h-ne", cursor: "nesw-resize" },
    { id: "e", cls: "h-e", cursor: "ew-resize" },
    { id: "se", cls: "h-se", cursor: "nwse-resize" },
    { id: "s", cls: "h-s", cursor: "ns-resize" },
    { id: "sw", cls: "h-sw", cursor: "nesw-resize" },
    { id: "w", cls: "h-w", cursor: "ew-resize" },
  ];

  function begin(e: PointerEvent, next: Drag) {
    e.stopPropagation();
    e.preventDefault();
    committed = false;
    atX = e.clientX;
    atY = e.clientY;
    fromCenter = e.altKey;
    tempAspect = e.shiftKey;
    drag = next;
  }

  function onHandleDown(e: PointerEvent, handle: HandleId) {
    begin(e, { kind: "handle", handle, start: structuredClone(crop), x: e.clientX, y: e.clientY });
  }

  function onMoveDown(e: PointerEvent) {
    begin(e, { kind: "move", start: structuredClone(crop), x: e.clientX, y: e.clientY });
  }

  function onPointerMove(e: PointerEvent) {
    atX = e.clientX;
    atY = e.clientY;
    fromCenter = e.altKey;
    tempAspect = e.shiftKey;
    setCropDraft(previewCrop());
  }

  function finish(e: PointerEvent) {
    if (!drag || committed) return;
    committed = true;
    atX = e.clientX;
    atY = e.clientY;
    fromCenter = e.altKey;
    tempAspect = e.shiftKey;
    const start = drag.start;
    const next = previewCrop();
    drag = null;
    if (cropParamsEqual(next, start)) {
      setCropDraft(null);
      return;
    }
    setCropDraft(next);
    void flushCropDraft();
  }
</script>

{#if frame}
  <div
    class="crop-root"
    style="left: {photoBox.left}px; top: {photoBox.top}px; width: {photoBox.width}px; height: {photoBox.height}px;"
  >
    <div class="dim" style="left: 0; top: 0; width: 100%; height: {Math.max(frame.top, 0)}px;"></div>
    <div
      class="dim"
      style="left: 0; top: {frame.top + frame.height}px; width: 100%; height: {Math.max(photoBox.height - (frame.top + frame.height), 0)}px;"
    ></div>
    <div
      class="dim"
      style="left: 0; top: {frame.top}px; width: {Math.max(frame.left, 0)}px; height: {frame.height}px;"
    ></div>
    <div
      class="dim"
      style="left: {frame.left + frame.width}px; top: {frame.top}px; width: {Math.max(photoBox.width - (frame.left + frame.width), 0)}px; height: {frame.height}px;"
    ></div>

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="crop-frame"
      class:is-drag={!!drag}
      style="left: {frame.left}px; top: {frame.top}px; width: {frame.width}px; height: {frame.height}px;"
      onpointerdown={onMoveDown}
    >
      <div class="thirds" aria-hidden="true">
        <i class="v" style="left: 33.333%"></i>
        <i class="v" style="left: 66.667%"></i>
        <i class="h" style="top: 33.333%"></i>
        <i class="h" style="top: 66.667%"></i>
      </div>

      {#each handles as h (h.id)}
        <button
          type="button"
          aria-label="Crop handle {h.id}"
          class="handle {h.cls}"
          style="cursor: {h.cursor};"
          onpointerdown={(e) => onHandleDown(e, h.id)}
        ></button>
      {/each}
    </div>
  </div>
{/if}

<style>
  .crop-root {
    position: absolute;
    overflow: hidden;
    pointer-events: none;
    z-index: 25;
  }
  .dim {
    position: absolute;
    background: rgba(0, 0, 0, 0.5);
  }
  .crop-frame {
    position: absolute;
    pointer-events: auto;
    cursor: grab;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.95);
  }
  .crop-frame.is-drag { cursor: grabbing; }
  .thirds {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .thirds i {
    position: absolute;
    background: rgba(255, 255, 255, 0.35);
  }
  .thirds .v { top: 0; bottom: 0; width: 1px; }
  .thirds .h { left: 0; right: 0; height: 1px; }

  .handle {
    position: absolute;
    padding: 0;
    border: 0;
    background: transparent;
    box-shadow: none;
  }
  .handle::before {
    content: "";
    position: absolute;
    pointer-events: none;
  }

  .h-nw, .h-ne, .h-sw, .h-se {
    width: 18px;
    height: 18px;
  }
  .h-nw { left: 0; top: 0; }
  .h-ne { right: 0; top: 0; }
  .h-sw { left: 0; bottom: 0; }
  .h-se { right: 0; bottom: 0; }
  .h-nw::before, .h-ne::before, .h-sw::before, .h-se::before {
    width: 11px;
    height: 11px;
    border: 2px solid #fff;
  }
  .h-nw::before { left: 2px; top: 2px; border-right: 0; border-bottom: 0; }
  .h-ne::before { right: 2px; top: 2px; border-left: 0; border-bottom: 0; }
  .h-sw::before { left: 2px; bottom: 2px; border-right: 0; border-top: 0; }
  .h-se::before { right: 2px; bottom: 2px; border-left: 0; border-top: 0; }

  .h-n, .h-s { left: 50%; width: 28px; height: 12px; transform: translateX(-50%); }
  .h-n { top: 0; }
  .h-s { bottom: 0; }
  .h-w, .h-e { top: 50%; width: 12px; height: 28px; transform: translateY(-50%); }
  .h-w { left: 0; }
  .h-e { right: 0; }
  .h-n::before, .h-s::before {
    left: 4px;
    right: 4px;
    height: 2px;
    background: #fff;
  }
  .h-n::before { top: 2px; }
  .h-s::before { bottom: 2px; }
  .h-w::before, .h-e::before {
    top: 4px;
    bottom: 4px;
    width: 2px;
    background: #fff;
  }
  .h-w::before { left: 2px; }
  .h-e::before { right: 2px; }
</style>
