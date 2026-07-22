<script lang="ts">
  import { onMount } from "svelte";
  import GlassPanel from "../primitives/GlassPanel.svelte";
  import ContextMenu from "../primitives/ContextMenu.svelte";
  import type { ContextMenuItem } from "../primitives/ContextMenu.svelte";
  import { imageBrowserCollapsed } from "../../../stores/editor";
  import { isExportOpen } from "../../../stores/ui";
  import { shortcutLabels } from "../../shortcuts";
  import { push } from "svelte-spa-router";
  import {
    activePhoto,
    openPhoto,
    photos,
    thumbUrl,
  } from "../../../stores/browse";

  let {
    onResizeStart,
    class: cls = "",
  }: {
    onResizeStart?: (e: MouseEvent) => void;
    class?: string;
  } = $props();

  let scrollEl = $state<HTMLDivElement | null>(null);

  function handleWheel(e: WheelEvent) {
    if (!scrollEl) return;
    const delta = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
    if (delta === 0) return;
    e.preventDefault();
    scrollEl.scrollLeft += delta;
  }

  function isEditableTarget(target: EventTarget | null) {
    if (!(target instanceof HTMLElement)) return false;
    return (
      target.isContentEditable ||
      ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)
    );
  }

  function handleKeydown(e: KeyboardEvent) {
    if (isEditableTarget(e.target)) return;
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    const list = photos.get();
    if (!list.length) return;
    const current = activePhoto.get();
    const currentIndex = current ? list.findIndex((p) => p.path === current.path) : -1;
    const nextIndex =
      e.key === "ArrowLeft"
        ? Math.max(0, currentIndex - 1)
        : Math.min(list.length - 1, currentIndex + 1);
    if (nextIndex === currentIndex || nextIndex < 0) return;
    e.preventDefault();
    void openPhoto(list[nextIndex]);
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  });

  // Automatically scroll filmstrip to keep selected photo centered
  $effect(() => {
    const current = $activePhoto;
    if (!current || !scrollEl) return;
    const activeEl = scrollEl.querySelector<HTMLElement>(".filmstrip-card--active");
    if (activeEl) {
      activeEl.scrollIntoView({ behavior: "smooth", block: "nearest", inline: "center" });
    }
  });

  // ── Context Menu ─────────────────────────────────────────────
  let ctxMenu = $state<{ x: number; y: number } | null>(null);
  let ctxPhoto = $state<(typeof $photos)[0] | null>(null);

  function handleThumbContextMenu(e: MouseEvent, photo: (typeof $photos)[0]) {
    e.preventDefault();
    e.stopPropagation();
    ctxPhoto = photo;
    ctxMenu = { x: e.clientX, y: e.clientY };
  }

  function safePush(path: string) {
    if (typeof document !== "undefined" && (document as any).startViewTransition) {
      (document as any).startViewTransition(() => push(path));
    } else {
      push(path);
    }
  }

  const ctxItems = $derived<ContextMenuItem[]>([
    {
      type: "item",
      label: "Open in editor",
      shortcut: shortcutLabels.openPhoto,
      onclick: () => {
        if (ctxPhoto) { void openPhoto(ctxPhoto); safePush("/edit"); }
      },
    },
    {
      type: "item",
      label: "Export…",
      shortcut: shortcutLabels.export,
      onclick: () => isExportOpen.set(true),
    },
    { type: "separator" },
    {
      type: "item",
      label: "Previous photo",
      shortcut: shortcutLabels.prevPhoto,
      onclick: () => {
        const list = photos.get();
        const current = activePhoto.get();
        const idx = current ? list.findIndex((p) => p.path === current.path) : -1;
        if (idx > 0) void openPhoto(list[idx - 1]);
      },
    },
    {
      type: "item",
      label: "Next photo",
      shortcut: shortcutLabels.nextPhoto,
      onclick: () => {
        const list = photos.get();
        const current = activePhoto.get();
        const idx = current ? list.findIndex((p) => p.path === current.path) : -1;
        if (idx >= 0 && idx < list.length - 1) void openPhoto(list[idx + 1]);
      },
    },
  ]);
</script>

<GlassPanel
  class="relative z-10 flex h-full min-w-0 {$imageBrowserCollapsed ? 'items-center px-[10px] py-0' : 'p-[10px]'} {cls}"
  style="--glass-bg: #171717;"
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
        class="h-[2px] w-[40px] rounded-full bg-white/5 group-hover:bg-white/25 group-active:bg-accent transition-[background-color] duration-200"
      ></div>
    </div>
  {/if}

  <button
    type="button"
    onclick={() => imageBrowserCollapsed.set(!$imageBrowserCollapsed)}
    aria-label={$imageBrowserCollapsed ? "Expand Filmstrip" : "Collapse Filmstrip"}
    class="z-20 flex size-[22px] shrink-0 items-center justify-center rounded-full border border-white/5 bg-white/[0.06] text-white/70 hover:bg-white/[0.14] hover:text-white active:scale-[0.96] transition-[background-color,color,transform] duration-200 cursor-pointer {$imageBrowserCollapsed ? '' : 'absolute left-[10px] top-[10px]'}"
  >
    <svg
      width="8"
      height="8"
      viewBox="0 0 8 8"
      fill="none"
      stroke="currentColor"
      stroke-width="1.4"
      stroke-linecap="round"
      stroke-linejoin="round"
      class="pointer-events-none transition-transform duration-200 {$imageBrowserCollapsed ? 'rotate-180' : ''}"
    >
      <path d="M1 3L4 6L7 3" />
    </svg>
  </button>

  <div
    bind:this={scrollEl}
    onwheel={handleWheel}
    class="flex min-w-0 flex-1 items-center gap-[24px] overflow-x-auto pl-[28px] transition-[opacity,transform] duration-200 {$imageBrowserCollapsed ? 'pointer-events-none opacity-0' : 'opacity-100'}"
  >
    {#each $photos as photo (photo.path)}
      <button
        type="button"
        class="filmstrip-card {$activePhoto?.path === photo.path ? 'filmstrip-card--active' : ''}"
        title={photo.filename}
        onclick={() => openPhoto(photo)}
        oncontextmenu={(e) => handleThumbContextMenu(e, photo)}
      >
        {#if photo.hasThumb}
          <img
            src={thumbUrl(photo.id)}
            alt={photo.filename}
            loading="lazy"
            class="filmstrip-img"
          />
        {:else}
          <span class="filmstrip-placeholder text-[11px] text-white/60">RAW</span>
        {/if}
      </button>
    {/each}
  </div>
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
  .filmstrip-card {
    appearance: none;
    -webkit-appearance: none;
    outline: none;
    position: relative;
    height: 92%;
    aspect-ratio: 3 / 2;
    flex-shrink: 0;
    cursor: pointer;
    overflow: hidden;
    border-radius: 12px;
    border: 1px solid transparent;
    background: transparent;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 14px;
    margin: 0;
    box-shadow: none;
    transition:
      border-color 150ms ease,
      background-color 150ms ease,
      opacity 150ms ease;
  }

  .filmstrip-card:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.06);
    box-shadow: none;
  }

  .filmstrip-card:active {
    opacity: 0.85;
    box-shadow: none;
  }

  .filmstrip-card--active {
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.12);
    box-shadow: none;
  }

  .filmstrip-img {
    height: 100%;
    width: 100%;
    object-fit: cover;
    display: block;
    border-radius: 0px;
  }

  .filmstrip-placeholder {
    display: flex;
    height: 100%;
    width: 100%;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 0px;
    transition: background-color 200ms ease;
  }

  .filmstrip-card:hover .filmstrip-placeholder {
    background: rgba(255, 255, 255, 0.05);
  }
</style>
