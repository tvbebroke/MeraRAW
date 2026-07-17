<script lang="ts">
  import GlassPanel from "../lib/components/primitives/GlassPanel.svelte";
  import FileBrowser from "../lib/components/file-browser/FileBrowser.svelte";
  import { leftRailCollapsed } from "../stores/editor";
  import {
    activePhoto,
    folder,
    openPhoto as browseOpenPhoto,
    photos,
    thumbUrl,
  } from "../stores/browse";
  import type { GridItem } from "../ipc/types";
  import { push } from "svelte-spa-router";
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";

  // Window dimensions for responsive boundaries
  let windowWidth = $state(0);

  // Left rail width
  let leftRailWidth = $state(220);
  let isResizingLeft = $state(false);

  const maxLeftRailWidth = $derived(Math.max(180, Math.min(400, windowWidth * 0.35)));

  $effect(() => {
    if (leftRailWidth > maxLeftRailWidth) leftRailWidth = maxLeftRailWidth;
    else if (leftRailWidth < 180) leftRailWidth = 180;
  });

  function handleLeftResizeStart(e: MouseEvent) {
    e.preventDefault();
    isResizingLeft = true;
    const startX = e.clientX;
    const startWidth = leftRailWidth;

    function handleMouseMove(moveEvent: MouseEvent) {
      const deltaX = moveEvent.clientX - startX;
      leftRailWidth = Math.max(180, Math.min(maxLeftRailWidth, startWidth + deltaX));
    }

    function handleMouseUp() {
      isResizingLeft = false;
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
      document.body.style.cursor = "";
    }

    document.body.style.cursor = "col-resize";
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
  }

  function handleLeftKeyDown(e: KeyboardEvent) {
    if (e.key === "ArrowRight") leftRailWidth = Math.max(180, Math.min(maxLeftRailWidth, leftRailWidth + 10));
    else if (e.key === "ArrowLeft") leftRailWidth = Math.max(180, Math.min(maxLeftRailWidth, leftRailWidth - 10));
  }

  // Sort / filter state
  type SortMode = "date-desc" | "date-asc" | "name-asc" | "name-desc" | "rating";
  let sortMode = $state<SortMode>("date-desc");
  let searchQuery = $state("");
  let showSortMenu = $state(false);

  const sortLabels: Record<SortMode, string> = {
    "date-desc": "Newest first",
    "date-asc":  "Oldest first",
    "name-asc":  "Name A–Z",
    "name-desc": "Name Z–A",
    "rating":    "Rating",
  };

  let selectedPath = $state<string | null>(null);

  const filteredPhotos = $derived.by(() => {
    let items = $photos.filter((p) =>
      p.filename.toLowerCase().includes(searchQuery.toLowerCase()),
    );
    switch (sortMode) {
      case "date-desc":
        items = [...items].sort((a, b) =>
          (b.capturedAt ?? "").localeCompare(a.capturedAt ?? ""),
        );
        break;
      case "date-asc":
        items = [...items].sort((a, b) =>
          (a.capturedAt ?? "").localeCompare(b.capturedAt ?? ""),
        );
        break;
      case "name-asc":
        items = [...items].sort((a, b) => a.filename.localeCompare(b.filename));
        break;
      case "name-desc":
        items = [...items].sort((a, b) => b.filename.localeCompare(a.filename));
        break;
      case "rating":
        items = [...items].sort((a, b) => b.rating - a.rating);
        break;
    }
    return items;
  });

  function selectPhoto(photo: GridItem) {
    selectedPath = photo.path;
  }

  async function openPhoto(photo: GridItem) {
    selectedPath = photo.path;
    await browseOpenPhoto(photo);
    if (typeof document !== "undefined" && (document as any).startViewTransition) {
      (document as any).startViewTransition(() => {
        push("/edit");
      });
    } else {
      push("/edit");
    }
  }

  const highlightPath = $derived(selectedPath ?? $activePhoto?.path ?? null);
</script>

<svelte:window bind:innerWidth={windowWidth} />

<div
  in:fade={{ duration: 200, delay: 100 }}
  out:fade={{ duration: 150 }}
  style="grid-template-columns: auto minmax(0, 1fr);"
  class="grid h-full min-h-0 gap-[11px] px-[11px] pb-[11px]"
>
  <!-- Animated Left Rail Container -->
  <div 
    style="
      width: {$leftRailCollapsed ? '0px' : `${leftRailWidth}px`};
      margin-right: {$leftRailCollapsed ? '-11px' : '0px'};
      transition: {isResizingLeft ? 'none' : 'width 350ms cubic-bezier(0.16, 1, 0.3, 1), margin-right 350ms cubic-bezier(0.16, 1, 0.3, 1)'};
    "
    class="relative min-h-0 {$leftRailCollapsed ? 'overflow-hidden pointer-events-none' : 'overflow-visible'}"
  >
    <!-- Left Rail: Folder Browser -->
    <FileBrowser class="h-full {isResizingLeft ? 'transition-none' : ''}" />
    <!-- Resize Handle -->
    {#if !$leftRailCollapsed}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        role="separator"
        aria-orientation="vertical"
        aria-valuenow={leftRailWidth}
        aria-valuemin={180}
        aria-valuemax={maxLeftRailWidth}
        tabindex="0"
        class="group absolute -right-[11px] top-0 bottom-0 w-[11px] cursor-col-resize z-50 flex items-center justify-center focus:outline-none"
        onmousedown={handleLeftResizeStart}
        onkeydown={handleLeftKeyDown}
      >
        <div class="w-[2px] h-[40px] rounded-full bg-white/5 group-hover:bg-white/25 group-active:bg-accent transition-all duration-200"></div>
      </div>
    {/if}
  </div>

  <!-- Main Content Area Wrapper -->
  <div class="relative min-h-0 flex flex-col">
    <GlassPanel class="flex-1 flex min-h-0 flex-col overflow-hidden">
      <!-- Toolbar -->
      <div class="flex shrink-0 items-center gap-[8px] px-[18px] pt-[18px] pb-[12px]">
        <!-- Sort / Filter button -->
        <div class="relative">
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <button
            class="toolbar-btn"
            onclick={() => (showSortMenu = !showSortMenu)}
            aria-label="Sort and filter"
            title="Sort and filter"
          >
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
              <path d="M2 3.5H14L9 9V13.5L7 14.5V9L2 3.5Z" stroke="rgba(255,255,255,0.45)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </button>

          {#if showSortMenu}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <!-- svelte-ignore a11y_interactive_supports_focus -->
            <div
              transition:scale={{ duration: 180, start: 0.95, easing: cubicOut }}
              class="sort-menu"
              role="menu"
              tabindex="-1"
              onclick={() => (showSortMenu = false)}
              onkeydown={(e) => e.key === 'Escape' && (showSortMenu = false)}
            >
              {#each Object.entries(sortLabels) as [mode, label]}
                <button
                  role="menuitem"
                  class="sort-item {sortMode === mode ? 'sort-item--active' : ''}"
                  onclick={() => { sortMode = mode as SortMode; showSortMenu = false; }}
                >
                  {label}
                  {#if sortMode === mode}
                    <svg class="ml-auto" width="10" height="8" viewBox="0 0 10 8" fill="none">
                      <path d="M1 4L3.5 6.5L9 1" stroke="rgba(255,255,255,0.95)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                  {/if}
                </button>
              {/each}
            </div>
          {/if}
        </div>

        <div class="flex-1"></div>

        <!-- Search bar -->
        <label class="search-wrap">
          <input
            type="search"
            placeholder="Search"
            bind:value={searchQuery}
            class="search-input"
          />
          <div class="search-icon-circle">
            <svg width="15" height="15" viewBox="0 0 15 15" fill="none" class="search-icon">
              <circle cx="6.5" cy="6.5" r="4.5" stroke="rgba(255,255,255,0.4)" stroke-width="1.5"/>
              <path d="M10 10L13.5 13.5" stroke="rgba(255,255,255,0.4)" stroke-width="1.5" stroke-linecap="round"/>
            </svg>
          </div>
        </label>
      </div>

      <!-- Photo Grid -->
      <div class="min-h-0 flex-1 overflow-y-auto px-[18px] pb-[18px] pt-[6px]">
        {#if !$folder}
          <!-- Empty state: no folder selected -->
          <div class="flex h-full flex-col items-center justify-center gap-3 text-center">
            <div class="empty-icon-wrap">
              <svg width="36" height="36" viewBox="0 0 36 36" fill="none" xmlns="http://www.w3.org/2000/svg">
                <rect x="1.5" y="8.5" width="33" height="24" rx="4.5" stroke="rgba(255,255,255,0.12)" stroke-width="1.5"/>
                <path d="M1.5 15H34.5" stroke="rgba(255,255,255,0.08)" stroke-width="1.5"/>
                <path d="M10 3.5H6a4.5 4.5 0 0 0-4.5 4.5V15" stroke="rgba(255,255,255,0.12)" stroke-width="1.5" stroke-linecap="round"/>
                <path d="M12 3.5H14" stroke="rgba(255,255,255,0.12)" stroke-width="1.5" stroke-linecap="round"/>
                <circle cx="18" cy="23" r="4" stroke="rgba(255,255,255,0.1)" stroke-width="1.5"/>
                <path d="M30 20.5L25.5 25" stroke="rgba(255,255,255,0.08)" stroke-width="1.5" stroke-linecap="round"/>
              </svg>
            </div>
            <p class="text-[13px] font-medium text-white/40">No folder open</p>
            <p class="text-[11px] text-white/20">Select a folder from the sidebar to browse your photos</p>
          </div>
        {:else if filteredPhotos.length === 0}
          <!-- Empty state: folder open but no matching photos -->
          <div class="flex h-full flex-col items-center justify-center gap-3 text-center">
            <p class="text-[13px] font-medium text-white/40">No photos found</p>
            {#if searchQuery}
              <p class="text-[11px] text-white/20">No results for "{searchQuery}"</p>
            {/if}
          </div>
        {:else}
          <!-- Photo Grid -->
          <div class="photo-grid">
            {#each filteredPhotos as photo (photo.path)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <button
                class="photo-card {highlightPath === photo.path ? 'photo-card--active' : ''}"
                onclick={() => selectPhoto(photo)}
                ondblclick={() => openPhoto(photo)}
                title={photo.filename}
              >
                <div class="photo-thumb-wrap">
                  {#if photo.hasThumb}
                    <img
                      src={thumbUrl(photo.id)}
                      alt={photo.filename}
                      loading="lazy"
                      class="photo-thumb"
                      style={highlightPath === photo.path ? 'view-transition-name: active-image;' : ''}
                    />
                  {:else}
                    <div class="photo-thumb-placeholder">
                      <span class="text-[10px] font-semibold tracking-widest text-white/20">RAW</span>
                    </div>
                  {/if}
                </div>
                <div class="photo-meta">
                  <span class="photo-name">{photo.filename}</span>
                </div>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </GlassPanel>
    {#if $leftRailCollapsed}
      <button 
        onclick={() => leftRailCollapsed.set(false)}
        aria-label="Expand Sidebar"
        class="absolute left-3 top-1/2 -translate-y-1/2 flex size-[26px] items-center justify-center rounded-full border border-white/5 bg-panel-2 backdrop-blur-md text-white/80 hover:bg-white/[0.12] hover:text-white active:scale-95 transition-all cursor-pointer z-50 shadow-md"
      >
        <svg width="6" height="10" viewBox="0 0 6 10" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="pointer-events-none">
          <path d="M1.5 1.5L5 5L1.5 8.5" />
        </svg>
      </button>
    {/if}
  </div>
</div>

<!-- Click-outside to close sort menu -->
{#if showSortMenu}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-30"
    onclick={() => (showSortMenu = false)}
    onkeydown={(e) => e.key === 'Escape' && (showSortMenu = false)}
    role="presentation"
  ></div>
{/if}

<style>
  /* ── Toolbar ──────────────────────────────────────────── */
  .toolbar-btn {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.08);
    padding: 0;
    margin: 0;
    cursor: pointer;

    display: flex;
    align-items: center;
    justify-content: center;

    width: 36px;
    height: 36px;
    border-radius: 9999px;
    background: #1e1e20;
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      inset 0 -1px 0 rgba(0, 0, 0, 0.2),
      0 2px 6px rgba(0, 0, 0, 0.25);
    transition: background 200ms ease, transform 200ms cubic-bezier(0.25, 1, 0.5, 1);
  }

  .toolbar-btn:hover {
    background: #2a2a2d;
    transform: translateY(-0.5px);
  }

  .toolbar-btn:active {
    transform: scale(0.96);
    background: #1a1a1c;
  }

  /* ── Sort Menu ────────────────────────────────────────── */
  .sort-menu {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 40;
    min-width: 150px;
    background: rgba(22, 22, 24, 0.92);
    -webkit-backdrop-filter: blur(20px);
    backdrop-filter: blur(20px);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    padding: 5px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  }

  .sort-item {
    appearance: none;
    -webkit-appearance: none;
    border: none;
    background: transparent;
    font: inherit;
    color: rgba(255, 255, 255, 0.6);
    cursor: pointer;

    display: flex;
    align-items: center;
    width: 100%;
    padding: 7px 10px;
    border-radius: 8px;
    font-size: 12px;
    text-align: left;
    transition: background 150ms ease, color 150ms ease;
  }

  .sort-item:hover {
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.9);
  }

  .sort-item--active {
    color: rgba(255, 255, 255, 0.9);
    font-weight: 500;
  }

  /* ── Search ───────────────────────────────────────────── */
  .search-wrap {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    height: 36px;
    width: 240px;
    padding: 0 4px 0 16px;
    border-radius: 9999px;
    border: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(33, 33, 35, 0.65);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.05),
      inset 0 -1px 0 rgba(0, 0, 0, 0.2),
      0 2px 6px rgba(0, 0, 0, 0.25);
    transition: background 150ms cubic-bezier(0.4, 0, 0.2, 1), border-color 150ms cubic-bezier(0.4, 0, 0.2, 1);
    cursor: text;
  }

  .search-wrap:focus-within {
    background: rgba(47, 47, 49, 0.85);
    border-color: rgba(255, 255, 255, 0.15);
  }

  .search-icon-circle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.25);
  }

  .search-icon {
    flex-shrink: 0;
  }

  .search-input {
    appearance: none;
    -webkit-appearance: none;
    border: none;
    background: transparent;
    font: inherit;
    color: rgba(255, 255, 255, 0.85);
    font-size: 12px;
    flex: 1;
    outline: none;
  }

  .search-input::placeholder {
    color: rgba(255, 255, 255, 0.25);
  }

  /* Remove native search clear button */
  .search-input::-webkit-search-cancel-button {
    display: none;
  }

  /* ── Empty State ──────────────────────────────────────── */
  .empty-icon-wrap {
    width: 64px;
    height: 64px;
    border-radius: 18px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  /* ── Photo Grid ───────────────────────────────────────── */
  .photo-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 10px;
  }

  .photo-card {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.06);
    padding: 0;
    margin: 0;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 14px;
    cursor: pointer;
    overflow: hidden;
    text-align: left;
    transition: border-color 180ms ease, box-shadow 200ms ease;
  }

  .photo-card:hover {
    border-color: rgba(255, 255, 255, 0.2);
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.45);
  }

  .photo-card:active {
    transform: scale(0.98);
  }

  .photo-card--active {
    border-color: rgba(255, 255, 255, 0.85);
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.3), 0 8px 24px rgba(0, 0, 0, 0.4);
    background: rgba(255, 255, 255, 0.05);
  }

  .photo-thumb-wrap {
    width: 100%;
    aspect-ratio: 3 / 2;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.02);
  }

  .photo-thumb {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .photo-thumb-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.025);
  }

  .photo-meta {
    padding: 7px 10px 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.04);
  }

  .photo-name {
    font-size: 10px;
    color: rgba(255, 255, 255, 0.5);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    display: block;
    letter-spacing: 0.01em;
  }
</style>

