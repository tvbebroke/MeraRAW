<script lang="ts">
  import { onMount } from "svelte";
  import GlassPanel from "../primitives/GlassPanel.svelte";
  import { leftRailCollapsed } from "../../../stores/editor";
  import {
    browseBusy,
    folder,
    folders,
    loadFolder,
    pickAndImportFolder,
    refreshFolders,
  } from "../../../stores/browse";

  import { workspace } from "../../../stores/workspace";

  let { class: cls = "" }: { class?: string } = $props();
  const isVideo = $derived($workspace === "video");

  let searchQuery = $state("");

  onMount(() => {
    void refreshFolders();
  });

  async function addFolder() {
    await pickAndImportFolder();
  }

  function selectFolder(root: string) {
    void loadFolder(root);
  }

  const filteredFolders = $derived(
    $folders.filter((f) => {
      if (!searchQuery) return true;
      const q = searchQuery.toLowerCase();
      return (
        f.name.toLowerCase().includes(q) || f.root.toLowerCase().includes(q)
      );
    }),
  );
</script>

<GlassPanel
  variant="quiet"
  class="flex min-h-0 flex-1 flex-col overflow-hidden {cls}"
>
  <!-- 34px header: title + affordances on one line, no separate toolbar row. -->
  <div class="panel-header">
    <span class="panel-title">Library</span>
    <div class="flex items-center gap-[2px]">
      <button
        type="button"
        onclick={addFolder}
        disabled={$browseBusy}
        aria-label="Add Folder"
        title="Import a folder"
        class="mr-icon-btn"
      >
        <svg
          width="10"
          height="10"
          viewBox="0 0 10 10"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          class="pointer-events-none"
        >
          <line x1="5" y1="1.5" x2="5" y2="8.5" stroke-linecap="round" />
          <line x1="1.5" y1="5" x2="8.5" y2="5" stroke-linecap="round" />
        </svg>
      </button>
      <button
        type="button"
        onclick={() => leftRailCollapsed.set(true)}
        aria-label="Collapse Sidebar"
        title="Collapse sidebar"
        class="mr-icon-btn"
      >
        <svg
          width="6"
          height="10"
          viewBox="0 0 6 10"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="pointer-events-none"
        >
          <path d="M4.5 8.5L1 5L4.5 1.5" />
        </svg>
      </button>
    </div>
  </div>

  <!-- Search-first: no box of its own, the hairline below is its underline. -->
  <div class="panel-header">
    <svg width="13" height="13" viewBox="0 0 15 15" fill="none" class="shrink-0">
      <circle cx="6.5" cy="6.5" r="4.5" stroke="currentColor" stroke-width="1.5" />
      <path d="M10 10L13.5 13.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
    </svg>
    <input class="search-input" type="search" placeholder={isVideo ? "Search folders" : "Search photos"} bind:value={searchQuery} />
    {#if searchQuery}
      <button class="mr-icon-btn" onclick={() => (searchQuery = "")} aria-label="Clear search">
        <svg width="9" height="9" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <path d="M1 1l8 8M9 1l-8 8" />
        </svg>
      </button>
    {/if}
  </div>

  <div class="min-h-0 flex-1 overflow-y-auto p-[6px] custom-scrollbar">
    {#if $browseBusy && filteredFolders.length === 0}
      <p class="empty-state">Loading…</p>
    {:else if filteredFolders.length === 0}
      <p class="empty-state">
        {searchQuery ? `No folders matching “${searchQuery}”` : isVideo ? "No clips here. Import a folder to begin." : "No photos here. Import a folder to begin."}
      </p>
    {:else}
      <p class="eyebrow px-[10px] pt-[8px] pb-[4px]">Folders</p>
      <ul>
        {#each filteredFolders as item (item.root)}
          <li>
            <!-- Rail row: fixed icon column, flexible label, mono count.
                 Active = sunken + weight 500, never an accent bar. -->
            <button
              type="button"
              title={item.root}
              class="rail-item {$folder === item.root ? 'rail-item--active' : ''}"
              onclick={() => selectFolder(item.root)}
            >
              <span class="rail-icon">
                {#if $folder === item.root}
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M6 14l1.45-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.55 6A2 2 0 0 1 18.45 20H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h7a2 2 0 0 1 2 2v2"/>
                  </svg>
                {:else}
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M20 20H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2z"/>
                  </svg>
                {/if}
              </span>
              <span class="rail-label">{item.name}</span>
              <span class="rail-count">{item.photoCount}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
  <div class="import-row">
    <button type="button" class="import-btn" onclick={addFolder} disabled={$browseBusy}>
      + Import {$workspace === "video" ? "clips" : "photos"}
    </button>
  </div>
</GlassPanel>

<style>
  .import-row {
    flex: none;
    padding: 8px 10px 12px;
  }
  .import-btn {
    width: 100%;
    height: 28px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--color-secondary);
    font-size: 12px;
    text-align: left;
    padding: 0 10px;
    cursor: pointer;
  }
  .import-btn:hover { background: var(--color-hover); color: var(--color-fg); }

  .custom-scrollbar::-webkit-scrollbar {
    width: 4px;
  }

  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }

  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: var(--color-active);
    border-radius: 99px;
  }

  .custom-scrollbar::-webkit-scrollbar-thumb:hover {
    background: var(--color-active);
  }
</style>
