<script lang="ts">
  import { applyCropParams } from "./cropActions";
  import {
    aspectRatio,
    dragHandle,
    readCropFromDoc,
    type CropParams,
    type HandleId,
  } from "./cropMath";
  import { doc, reconcile } from "../stores/doc";
  import { imageDims } from "../stores/app";

  let {
    wrapEl,
  }: {
    wrapEl: HTMLElement | null;
  } = $props();

  const crop = $derived(readCropFromDoc($doc?.modules));
  const dims = $derived($imageDims);

  let imgBox = $state<{ left: number; top: number; width: number; height: number } | null>(
    null,
  );

  type Drag =
    | { kind: "handle"; handle: HandleId; start: CropParams; x: number; y: number }
    | { kind: "move"; start: CropParams; x: number; y: number };

  let drag = $state<Drag | null>(null);

  $effect(() => {
    if (!drag) return;
    const move = (e: PointerEvent) => onPointerMove(e);
    const up = (e: PointerEvent) => onPointerUp(e);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
    };
  });

  function measure() {
    if (!wrapEl) {
      imgBox = null;
      return;
    }
    const img = wrapEl.querySelector("img");
    if (!img) {
      imgBox = null;
      return;
    }
    const wr = wrapEl.getBoundingClientRect();
    const ir = img.getBoundingClientRect();
    imgBox = {
      left: ir.left - wr.left,
      top: ir.top - wr.top,
      width: ir.width,
      height: ir.height,
    };
  }

  $effect(() => {
    const _c = crop;
    const _d = dims;
    const wrap = wrapEl;
    if (!wrap) return;
    measure();
    const obs = new ResizeObserver(() => measure());
    obs.observe(wrap);
    const img = wrap.querySelector("img");
    if (img) obs.observe(img);
    return () => obs.disconnect();
  });

  const frame = $derived.by(() => {
    if (!imgBox || imgBox.width < 4 || imgBox.height < 4) return null;
    const r = crop.rect;
    return {
      left: imgBox.left + r.left * imgBox.width,
      top: imgBox.top + r.top * imgBox.height,
      width: Math.max((r.right - r.left) * imgBox.width, 1),
      height: Math.max((r.bottom - r.top) * imgBox.height, 1),
    };
  });

  const handles: { id: HandleId; left: string; top: string; cursor: string }[] = [
    { id: "nw", left: "0%", top: "0%", cursor: "nwse-resize" },
    { id: "n", left: "50%", top: "0%", cursor: "ns-resize" },
    { id: "ne", left: "100%", top: "0%", cursor: "nesw-resize" },
    { id: "e", left: "100%", top: "50%", cursor: "ew-resize" },
    { id: "se", left: "100%", top: "100%", cursor: "nwse-resize" },
    { id: "s", left: "50%", top: "100%", cursor: "ns-resize" },
    { id: "sw", left: "0%", top: "100%", cursor: "nesw-resize" },
    { id: "w", left: "0%", top: "50%", cursor: "ew-resize" },
  ];

  async function commit(p: CropParams, live: boolean) {
    try {
      reconcile(await applyCropParams(p, live));
    } catch {
      /* ignore */
    }
  }

  function onHandleDown(e: PointerEvent, handle: HandleId) {
    e.stopPropagation();
    e.preventDefault();
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    drag = { kind: "handle", handle, start: structuredClone(crop), x: e.clientX, y: e.clientY };
  }

  function onMoveDown(e: PointerEvent) {
    e.stopPropagation();
    e.preventDefault();
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    drag = { kind: "move", start: structuredClone(crop), x: e.clientX, y: e.clientY };
  }

  function onPointerMove(e: PointerEvent) {
    if (!drag || !imgBox || !dims) return;
    const dxPx = e.clientX - drag.x;
    const dyPx = e.clientY - drag.y;
    const dx = dxPx / imgBox.width;
    const dy = dyPx / imgBox.height;
    if (drag.kind === "move") {
      const r = drag.start.rect;
      const w = r.right - r.left;
      const h = r.bottom - r.top;
      let left = r.left + dx;
      let top = r.top + dy;
      left = Math.max(0, Math.min(1 - w, left));
      top = Math.max(0, Math.min(1 - h, top));
      void commit(
        {
          ...drag.start,
          rect: { left, top, right: left + w, bottom: top + h },
        },
        true,
      );
      return;
    }
    const ratio =
      drag.start.aspectLocked && drag.start.aspectW > 0 && drag.start.aspectH > 0
        ? drag.start.aspectW / drag.start.aspectH
        : aspectRatio(drag.start.rect, dims.w, dims.h);
    const next = dragHandle(drag.start.rect, drag.handle, dx, dy, {
      imgW: dims.w,
      imgH: dims.h,
      aspectLocked: drag.start.aspectLocked,
      aspectRatio: ratio,
      fromCenter: e.altKey,
      tempAspect: e.shiftKey,
    });
    void commit({ ...drag.start, rect: next }, true);
  }

  function onPointerUp(e: PointerEvent) {
    if (!drag || !imgBox || !dims) {
      drag = null;
      return;
    }
    const dxPx = e.clientX - drag.x;
    const dyPx = e.clientY - drag.y;
    const dx = dxPx / imgBox.width;
    const dy = dyPx / imgBox.height;
    if (drag.kind === "move") {
      const r = drag.start.rect;
      const w = r.right - r.left;
      const h = r.bottom - r.top;
      let left = r.left + dx;
      let top = r.top + dy;
      left = Math.max(0, Math.min(1 - w, left));
      top = Math.max(0, Math.min(1 - h, top));
      void commit(
        {
          ...drag.start,
          rect: { left, top, right: left + w, bottom: top + h },
        },
        false,
      );
    } else {
      const ratio =
        drag.start.aspectLocked && drag.start.aspectW > 0 && drag.start.aspectH > 0
          ? drag.start.aspectW / drag.start.aspectH
          : aspectRatio(drag.start.rect, dims.w, dims.h);
      const next = dragHandle(drag.start.rect, drag.handle, dx, dy, {
        imgW: dims.w,
        imgH: dims.h,
        aspectLocked: drag.start.aspectLocked,
        aspectRatio: ratio,
        fromCenter: e.altKey,
        tempAspect: e.shiftKey,
      });
      void commit({ ...drag.start, rect: next }, false);
    }
    drag = null;
  }
</script>

{#if frame && imgBox}
  <div class="pointer-events-none absolute inset-0 z-20">
    <!-- Dim outside crop -->
    <div
      class="absolute bg-black/45"
      style="left: {imgBox.left}px; top: {imgBox.top}px; width: {imgBox.width}px; height: {Math.max(frame.top - imgBox.top, 0)}px;"
    ></div>
    <div
      class="absolute bg-black/45"
      style="left: {imgBox.left}px; top: {frame.top + frame.height}px; width: {imgBox.width}px; height: {Math.max(imgBox.top + imgBox.height - (frame.top + frame.height), 0)}px;"
    ></div>
    <div
      class="absolute bg-black/45"
      style="left: {imgBox.left}px; top: {frame.top}px; width: {Math.max(frame.left - imgBox.left, 0)}px; height: {frame.height}px;"
    ></div>
    <div
      class="absolute bg-black/45"
      style="left: {frame.left + frame.width}px; top: {frame.top}px; width: {Math.max(imgBox.left + imgBox.width - (frame.left + frame.width), 0)}px; height: {frame.height}px;"
    ></div>

    <!-- Crop frame -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="pointer-events-auto absolute border border-white/90"
      style="left: {frame.left}px; top: {frame.top}px; width: {frame.width}px; height: {frame.height}px; cursor: move;"
      onpointerdown={onMoveDown}
    >
      <!-- Rule of thirds -->
      <div class="pointer-events-none absolute inset-0">
        <div class="absolute left-1/3 top-0 bottom-0 w-px bg-white/35"></div>
        <div class="absolute left-2/3 top-0 bottom-0 w-px bg-white/35"></div>
        <div class="absolute top-1/3 left-0 right-0 h-px bg-white/35"></div>
        <div class="absolute top-2/3 left-0 right-0 h-px bg-white/35"></div>
      </div>

      {#each handles as h (h.id)}
        <button
          type="button"
          aria-label="Crop handle {h.id}"
          class="absolute size-[10px] -translate-x-1/2 -translate-y-1/2 rounded-[1px] border border-black/40 bg-white shadow"
          style="left: {h.left}; top: {h.top}; cursor: {h.cursor};"
          onpointerdown={(e) => onHandleDown(e, h.id)}
        ></button>
      {/each}
    </div>
  </div>
{/if}
