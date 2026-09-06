<script lang="ts">
  import { onMount } from "svelte";
  import GlassPanel from "../primitives/GlassPanel.svelte";
  import ContextMenu from "../primitives/ContextMenu.svelte";
  import type { ContextMenuItem } from "../primitives/ContextMenu.svelte";
  import { leftRailCollapsed } from "../../../stores/editor";
  import {
    addFolderShortcut,
    addFolderShortcuts,
    browseBusy,
    folder,
    folderKinds,
    folderKindFor,
    folderSections,
    folders,
    isLibraryFileRoot,
    isNestedFolderShortcut,
    loadFolder,
    pickAndImportFolder,
    pickAndImportPhotos,
    refreshFolders,
    removeFolderShortcut,
    setFolderKind,
    sameFolderPath,
    type FolderMediaKind,
  } from "../../../stores/browse";
  import type { DiscoveredFolder, FolderItem } from "../../../ipc/types";
  import { discoverMediaFolders } from "../../../ipc/commands";
  import FolderNode from "./FolderNode.svelte";

  import { setWorkspace, workspace, VIDEO_WORKSPACE_ENABLED } from "../../../stores/workspace";
  import { isVideoPath } from "../../media";

  let { class: cls = "" }: { class?: string } = $props();
  const isVideo = $derived($workspace === "video");

  let searchQuery = $state("");
  let ctxMenu = $state<{ x: number; y: number; item: FolderItem } | null>(null);
  let finding = $state(false);
  let findError = $state<string | null>(null);
  let found = $state<DiscoveredFolder[]>([]);
  let findOpen = $state(false);
  let addingAll = $state(false);

  onMount(() => {
    void refreshFolders();
  });

  async function addFolder() {
    await pickAndImportFolder();
  }

  async function importPhotos() {
    await pickAndImportPhotos();
  }

  function selectFolderPath(path: string, section: "photo" | "video") {
    if (section === "video" && $workspace !== "video") setWorkspace("video");
    else if (section === "photo" && $workspace !== "photo") setWorkspace("photo");
    void loadFolder(path, { resumeImport: true });
  }

  function folderCtx(e: MouseEvent, path: string) {
    const item = $folders.find((f) => sameFolderPath(f.root, path));
    if (!item) return;
    openCtx(e, item);
  }

  function matchesSearch(f: FolderItem, q: string): boolean {
    if (!q) return true;
    return f.name.toLowerCase().includes(q) || f.root.toLowerCase().includes(q);
  }

  const searched = $derived(
    $folders.filter((f) => matchesSearch(f, searchQuery.toLowerCase())),
  );

  const photoFolders = $derived(
    searched.filter(
      (f) =>
        !isNestedFolderShortcut(f, searched) &&
        folderSections(f, folderKindFor(f.root, $folderKinds)).includes("photo"),
    ),
  );
  const videoFolders = $derived(
    searched.filter(
      (f) =>
        !isNestedFolderShortcut(f, searched) &&
        folderSections(f, folderKindFor(f.root, $folderKinds)).includes("video"),
    ),
  );

  const newFound = $derived(
    found.filter((f) => {
      if ($folders.some((p) => sameFolderPath(p.root, f.path))) return false;
      if (!searchQuery) return true;
      const q = searchQuery.toLowerCase();
      return f.name.toLowerCase().includes(q) || f.path.toLowerCase().includes(q);
    }),
  );

  async function findFolders() {
    finding = true;
    findError = null;
    findOpen = true;
    try {
      found = await discoverMediaFolders();
    } catch (e) {
      findError = e instanceof Error ? e.message : "Could not search this computer.";
      found = [];
    } finally {
      finding = false;
    }
  }

  async function pinFound(path: string) {
    found = found.filter((f) => !sameFolderPath(f.path, path));
    await addFolderShortcut(path);
  }

  async function pinAllFound() {
    const roots = newFound.map((f) => f.path);
    if (!roots.length) return;
    addingAll = true;
    try {
      await addFolderShortcuts(roots);
      found = [];
      findOpen = false;
    } finally {
      addingAll = false;
    }
  }

  const ctxItems = $derived.by((): ContextMenuItem[] => {
    if (!ctxMenu) return [];
    const item = ctxMenu.item;
    const current: FolderMediaKind = folderKindFor(item.root, $folderKinds);
    return [
      { type: "header", label: item.name },
      {
        type: "item",
        label: "Auto (from files)",
        onclick: () => setFolderKind(item.root, "auto"),
        disabled: current === "auto",
      },
      {
        type: "item",
        label: "Keep in Photos",
        onclick: () => setFolderKind(item.root, "photo"),
        disabled: current === "photo",
      },
      {
        type: "item",
        label: "Keep in Videos",
        onclick: () => setFolderKind(item.root, "video"),
        disabled: current === "video",
      },
      { type: "separator" },
      {
        type: "item",
        label: "Remove shortcut",
        danger: true,
        onclick: () => void removeFolderShortcut(item.root),
      },
    ];
  });

  function openCtx(e: MouseEvent, item: FolderItem) {
    e.preventDefault();
    e.stopPropagation();
    ctxMenu = { x: e.clientX, y: e.clientY, item };
  }
</script>

<GlassPanel
  variant="quiet"
  class="flex h-full min-h-0 min-w-0 w-full flex-col overflow-hidden {cls}"
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
    <input class="search-input" type="search" placeholder="Search folders" bind:value={searchQuery} />
    {#if searchQuery}
      <button class="mr-icon-btn" onclick={() => (searchQuery = "")} aria-label="Clear search">
        <svg width="9" height="9" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
          <path d="M1 1l8 8M9 1l-8 8" />
        </svg>
      </button>
    {/if}
  </div>

  <div class="min-h-0 flex-1 overflow-y-auto p-[6px] custom-scrollbar">
    {#if findOpen}
      <p class="eyebrow px-[10px] pt-[8px] pb-[4px]">Found</p>
      {#if finding}
        <p class="empty-state">Searching Pictures, Movies, Downloads…</p>
      {:else if findError}
        <p class="empty-state">{findError}</p>
      {:else if newFound.length === 0}
        <p class="empty-state">{searchQuery ? `No folders matching “${searchQuery}”` : "No new photo or video folders on this computer."}</p>
      {:else}
        <ul>
          {#each newFound as item (item.path)}
            <li>
              <FolderNode
                name={item.name}
                path={item.path}
                count={item.photoCount + item.videoCount}
                section="found"
                selectOnClick={false}
                showPin={true}
                onPin={() => void pinFound(item.path)}
                onSelectFolder={selectFolderPath}
              />
            </li>
          {/each}
        </ul>
        <div class="found-actions">
          <button type="button" class="import-btn" onclick={() => void pinAllFound()} disabled={addingAll || $browseBusy}>
            {addingAll ? "Adding…" : `Add all (${newFound.length})`}
          </button>
          <button type="button" class="import-btn" onclick={() => (findOpen = false)}>Hide</button>
        </div>
      {/if}
    {/if}

    {#if $browseBusy && searched.length === 0 && !findOpen}
      <p class="empty-state">Loading…</p>
    {:else if searched.length === 0 && !findOpen}
      <p class="empty-state">
        {searchQuery ? `No folders matching “${searchQuery}”` : isVideo ? "No clips here. Import a clip or add a folder to begin." : "No photos here. Import a photo or add a folder to begin."}
      </p>
    {:else}
      {#if photoFolders.length}
        <p class="eyebrow px-[10px] pt-[8px] pb-[4px]">Photos</p>
        <ul>
          {#each photoFolders as item (item.root)}
            <li>
              <FolderNode
                name={item.name}
                path={item.root}
                isDir={!isLibraryFileRoot(item)}
                kind={isLibraryFileRoot(item) ? (isVideoPath(item.root) ? "video" : "photo") : null}
                count={isLibraryFileRoot(item) ? null : (item.photoCount ?? 0)}
                accessible={item.accessible}
                section="photo"
                activePath={$folder}
                onSelectFolder={selectFolderPath}
                onContextMenu={folderCtx}
              />
            </li>
          {/each}
        </ul>
      {/if}

      {#if VIDEO_WORKSPACE_ENABLED && videoFolders.length}
        <p class="eyebrow px-[10px] pt-[12px] pb-[4px]">Videos</p>
        <ul>
          {#each videoFolders as item (item.root)}
            <li>
              <FolderNode
                name={item.name}
                path={item.root}
                isDir={!isLibraryFileRoot(item)}
                kind={isLibraryFileRoot(item) ? (isVideoPath(item.root) ? "video" : "photo") : null}
                count={isLibraryFileRoot(item) ? null : (item.videoCount ?? 0)}
                accessible={item.accessible}
                section="video"
                activePath={$folder}
                onSelectFolder={selectFolderPath}
                onContextMenu={folderCtx}
              />
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  </div>
  <div class="import-row">
    <button type="button" class="import-btn" onclick={() => void findFolders()} disabled={finding}>
      {finding ? "Finding folders…" : "Find folders"}
    </button>
    <button type="button" class="import-btn" onclick={() => void importPhotos()} disabled={$browseBusy}>
      + Import {$workspace === "video" ? "clips" : "photos"}
    </button>
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
  .import-row {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px 12px;
  }
  .found-actions {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px 0 8px;
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
