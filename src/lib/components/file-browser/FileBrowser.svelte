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

  let { class: cls = "" }: { class?: string } = $props();

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
  class="flex min-h-0 flex-1 flex-col overflow-hidden p-[10px] {cls}"
  style="--glass-bg: #171717;"
>
  <div class="flex shrink-0 items-center justify-between px-[6px] pb-[6px] mb-2">
    <span class="text-[11px] font-semibold text-white/90 tracking-wide">Folders</span>
    <div class="flex items-center gap-[6px]">
      <button
        type="button"
        onclick={addFolder}
        disabled={$browseBusy}
        aria-label="Add Folder"
        class="flex size-[26px] items-center justify-center rounded-full border border-white/5 bg-white/[0.04] text-white/80 hover:bg-white/[0.12] hover:text-white active:scale-95 transition-all cursor-pointer disabled:opacity-40"
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
        class="flex size-[26px] items-center justify-center rounded-full border border-white/5 bg-white/[0.04] text-white/80 hover:bg-white/[0.12] hover:text-white active:scale-95 transition-all cursor-pointer"
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

  <div class="px-[4px] mb-3 shrink-0">
    <label class="search-wrap-sidebar">
      <input
        type="search"
        placeholder="Search directories..."
        bind:value={searchQuery}
        class="search-input-sidebar"
      />
      <div class="search-icon-circle-sidebar">
        <svg width="13" height="13" viewBox="0 0 15 15" fill="none" class="search-icon-sidebar">
          <circle cx="6.5" cy="6.5" r="4.5" stroke="rgba(255,255,255,0.4)" stroke-width="1.5" />
          <path
            d="M10 10L13.5 13.5"
            stroke="rgba(255,255,255,0.4)"
            stroke-width="1.5"
            stroke-linecap="round"
          />
        </svg>
      </div>
    </label>
  </div>

  <div class="min-h-0 flex-1 overflow-y-auto px-[2px] custom-scrollbar">
    {#if $browseBusy && filteredFolders.length === 0}
      <p class="px-[8px] py-[6px] text-[11px] text-white/40">Loading…</p>
    {:else if filteredFolders.length === 0}
      <p class="px-[8px] py-[6px] text-[11px] text-white/40">
        No catalog folders — use + to import
      </p>
    {:else}
      <ul class="flex flex-col gap-[2px]">
        {#each filteredFolders as item (item.root)}
          <li>
            <button
              type="button"
              class="flex w-full items-center gap-[6px] py-[4px] px-[6px] rounded-[8px] hover:bg-white/[0.03] transition-all cursor-pointer select-none text-[11px] text-left {$folder === item.root ? 'bg-white/[0.08] shadow-sm border border-white/[0.03]' : 'border border-transparent'}"
              onclick={() => selectFolder(item.root)}
            >
              <span class="w-[15px] h-[15px] flex items-center justify-center shrink-0">
                {#if $folder === item.root}
                  <svg width="14.5" height="14.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="text-white">
                    <path d="M6 14l1.45-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.55 6A2 2 0 0 1 18.45 20H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h7a2 2 0 0 1 2 2v2"/>
                  </svg>
                {:else}
                  <svg width="14.5" height="14.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="text-white/50">
                    <path d="M20 20H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2z"/>
                  </svg>
                {/if}
              </span>
              <span class="min-w-0 flex-1 truncate text-white/80 {$folder === item.root ? 'text-white font-medium' : ''}">
                {item.name}
              </span>
              <span class="shrink-0 text-[10px] text-white/40">{item.photoCount}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</GlassPanel>

<style>
  .search-wrap-sidebar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    height: 30px;
    width: 100%;
    padding: 0 3px 0 12px;
    border-radius: 9999px;
    border: 1px solid var(--color-border-input, rgba(255, 255, 255, 0.06));
    background: var(--color-surface-input, rgba(33, 33, 35, 0.65));
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.05),
      inset 0 -1px 0 rgba(0, 0, 0, 0.2),
      0 2px 6px rgba(0, 0, 0, 0.25);
    transition:
      background 150ms cubic-bezier(0.4, 0, 0.2, 1),
      border-color 150ms cubic-bezier(0.4, 0, 0.2, 1);
    cursor: text;
  }

  .search-wrap-sidebar:focus-within {
    background: rgba(0, 0, 0, 0.4);
    border-color: var(--color-border-hover, rgba(255, 255, 255, 0.15));
  }

  .search-icon-circle-sidebar {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.25);
  }

  .search-icon-sidebar {
    flex-shrink: 0;
  }

  .search-input-sidebar {
    appearance: none;
    -webkit-appearance: none;
    border: none;
    background: transparent;
    font: inherit;
    color: rgba(255, 255, 255, 0.85);
    font-size: 10px;
    flex: 1;
    outline: none;
  }

  .search-input-sidebar::placeholder {
    color: rgba(255, 255, 255, 0.25);
  }

  .search-input-sidebar::-webkit-search-cancel-button {
    display: none;
  }

  .custom-scrollbar::-webkit-scrollbar {
    width: 4px;
  }

  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }

  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 99px;
  }

  .custom-scrollbar::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.15);
  }
</style>
