<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { router, push } from "svelte-spa-router";
  import IconButton from "../primitives/IconButton.svelte";
  import settingsIcon from "../../icons/settings.svg";
  import helpIcon from "../../icons/help.svg";
  import { isSettingsOpen, isExportOpen, isShortcutsOpen } from "../../../stores/ui";
  import JobsPill from "./JobsPill.svelte";
  import VersionsButton from "./VersionsButton.svelte";
  import { commandPaletteOpen } from "../../../stores/editor";
  import { pickFile } from "../../fs";
  import { openPath } from "../../engine/boot";
  import { shortcutLabels } from "../../shortcuts";
  import { lastOpenedPath, imageMeta } from "../../../stores/app";
  import {
    folder,
    folders,
    activePhoto,
    folderLeafName,
    libraryPinFor,
    loadFolder,
    sameFolderPath,
  } from "../../../stores/browse";
  import { undo, redo } from "../../../ipc/commands";
  import { reconcile } from "../../../stores/doc";
  import {
    isLibraryRoute,
    libraryRoute,
    setWorkspace,
    workspace,
  } from "../../../stores/workspace";

  // Lazy so the page still renders in a plain browser (no Tauri runtime)
  const isLibrary = $derived(isLibraryRoute(router.location));
  const isVideo = $derived($workspace === "video");

  let isFullscreen = $state(false);

  onMount(() => {
    if ((window as any).__TAURI_INTERNALS__) {
      const windowInstance = getCurrentWindow();
      
      // Initial check
      windowInstance.isFullscreen().then((val) => {
        isFullscreen = val;
      });

      // Update fullscreen state on window resize
      const unlistenPromise = windowInstance.onResized(() => {
        windowInstance.isFullscreen().then((val) => {
          isFullscreen = val;
        });
      });

      return () => {
        unlistenPromise.then((unlisten) => unlisten());
      };
    }
  });

  function safePush(path: string) {
    if (typeof document !== "undefined" && (document as any).startViewTransition) {
      (document as any).startViewTransition(() => {
        push(path);
      });
    } else {
      push(path);
    }
  }

  async function openRaw() {
    const path = await pickFile($workspace);
    if (path) void openPath(path);
  }

  function basename(path: string | null | undefined): string {
    if (!path) return "";
    const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
    return parts.at(-1) ?? path;
  }

  const pin = $derived(libraryPinFor($folder, $folders));
  const nested = $derived(
    Boolean($folder && pin && !sameFolderPath($folder, pin.root)),
  );
  const dayName = $derived($folder ? folderLeafName($folder) : "");
  const fileName = $derived(
    $activePhoto?.filename || basename($imageMeta?.path) || basename($lastOpenedPath),
  );

  function goToLibrary(root?: string | null) {
    if (root) void loadFolder(root);
    safePush(libraryRoute());
  }

  function beginDrag(e: MouseEvent) {
    if (e.button !== 0) return;
    if (!(window as any).__TAURI_INTERNALS__) return;
    const t = e.target as HTMLElement | null;
    if (!t) return;
    if (t.closest("[data-tauri-drag-region='false']")) return;
    if (t.closest("button, input, a, select, textarea")) return;
    void getCurrentWindow().startDragging();
  }
</script>

<header
  data-tauri-drag-region
  class="titlebar relative z-20 flex h-full min-h-0 items-center bg-bg pr-3 {isFullscreen ? 'pl-4' : 'pl-[100px]'} border-b border-border/80"
  onmousedown={beginDrag}
>
  <div class="ws-switch" data-tauri-drag-region="false" role="tablist" aria-label="Editor">
    <button
      type="button"
      role="tab"
      class="ws-chip"
      class:is-on={!isVideo}
      aria-selected={!isVideo}
      onclick={() => setWorkspace("photo")}
    >Photo</button>
    <button
      type="button"
      class="ws-chip is-soon"
      disabled
      aria-disabled="true"
      title="Video editor coming soon"
    >Coming soon</button>
  </div>
  {#if pin || $folder || fileName}
    {#if pin}
      <span class="crumb-sep" data-tauri-drag-region aria-hidden="true">/</span>
      {#if isLibrary && !nested}
        <span class="crumb-file truncate max-w-[10rem]" data-tauri-drag-region title={pin.root}>{pin.name}</span>
      {:else}
        <button
          type="button"
          class="crumb-link truncate max-w-[10rem]"
          title="All photos in {pin.name}"
          onclick={() => goToLibrary(pin.root)}
        >{pin.name}</button>
      {/if}
    {:else if $folder}
      <span class="crumb-sep" data-tauri-drag-region aria-hidden="true">/</span>
      <button
        type="button"
        class="crumb-link truncate max-w-[10rem]"
        title={$folder}
        onclick={() => goToLibrary($folder)}
      >{dayName}</button>
    {/if}
    {#if nested && $folder}
      <span class="crumb-sep" data-tauri-drag-region aria-hidden="true">/</span>
      {#if isLibrary}
        <span class="crumb-file truncate max-w-[10rem]" data-tauri-drag-region title={$folder}>{dayName}</span>
      {:else}
        <button
          type="button"
          class="crumb-link truncate max-w-[10rem]"
          title="Photos in {dayName}"
          onclick={() => goToLibrary($folder)}
        >{dayName}</button>
      {/if}
    {/if}
    {#if fileName && !isLibrary}
      <span class="crumb-sep" data-tauri-drag-region aria-hidden="true">/</span>
      <span class="crumb-file truncate max-w-[16rem]" data-tauri-drag-region title={fileName}>{fileName}</span>
    {/if}
  {/if}

  <div class="titlebar-grip" data-tauri-drag-region></div>

  <div class="flex items-center gap-[4px]" data-tauri-drag-region="false">
    <JobsPill />
    <VersionsButton />
    <button type="button" class="quiet-btn" onclick={() => void undo().then(reconcile).catch(() => {})} title="Undo ({shortcutLabels.undo})">Undo</button>
    <button type="button" class="quiet-btn" onclick={() => void redo().then(reconcile).catch(() => {})} title="Redo ({shortcutLabels.redo})">Redo</button>
    <button type="button" class="quiet-btn" onclick={() => commandPaletteOpen.set(true)} title="Commands (⌘K)">⌘K</button>
    <button type="button" class="quiet-btn" onclick={() => void openRaw()}>Open</button>
    <IconButton
      icon={settingsIcon}
      label="Settings"
      title="Settings ({shortcutLabels.settings})"
      iconClass="size-[18px]"
      onclick={() => isSettingsOpen.set(true)}
    />
    <IconButton
      icon={helpIcon}
      label="Help"
      title="Keyboard Shortcuts"
      iconClass="h-[16px] w-[11px]"
      onclick={() => isShortcutsOpen.set(true)}
    />

    {#if isLibrary}
      <button
        onclick={() => safePush(isVideo ? "/grade" : "/edit")}
        aria-label={isVideo ? "Grade" : "Edit"}
        title={isVideo ? "Grade" : `Edit (${shortcutLabels.edit})`}
        class="quiet-btn"
        data-tauri-drag-region="false"
      >
        {isVideo ? "Grade" : "Edit"}
      </button>
    {:else}
      <button
        type="button"
        class="quiet-btn"
        title={nested && pin ? `All photos in ${pin.name}` : isVideo ? "Clips (G)" : `Library (${shortcutLabels.library})`}
        onclick={() => goToLibrary(nested && pin ? pin.root : null)}
      >
        {nested ? "All photos" : isVideo ? "Clips" : "Library"}
      </button>
      <button
        type="button"
        class="export-btn"
        title="Export ({shortcutLabels.export})"
        onclick={() => isExportOpen.set(true)}
      >
        Export
      </button>
    {/if}
  </div>
</header>

<style>
  .titlebar {
    -webkit-app-region: drag;
    app-region: drag;
  }
  .titlebar :global(button),
  .titlebar .ws-switch,
  .titlebar [data-tauri-drag-region="false"] {
    -webkit-app-region: no-drag;
    app-region: no-drag;
  }
  .titlebar-grip {
    flex: 1 1 auto;
    align-self: stretch;
    min-width: 48px;
    min-height: 100%;
  }
  .crumb-sep {
    margin: 0 7px;
    color: var(--color-subtle);
    opacity: 0.55;
    font-size: 13px;
  }
  .crumb-link {
    appearance: none;
    border: 0;
    background: transparent;
    padding: 0;
    font: inherit;
    font-size: 13px;
    color: var(--color-subtle);
    cursor: pointer;
    max-width: 10rem;
  }
  .crumb-link:hover {
    color: var(--color-fg);
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .crumb-file {
    font-size: 13px;
    color: var(--color-fg);
  }
  .ws-switch {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
    background: var(--color-sunken);
    border: 1px solid var(--color-border);
  }
  .ws-chip {
    height: 22px;
    padding: 0 9px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--color-secondary);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .ws-chip:hover { color: var(--color-fg); }
  .ws-chip.is-on {
    background: var(--color-active);
    color: var(--color-fg);
    font-weight: 500;
  }
  .ws-chip.is-soon,
  .ws-chip:disabled {
    opacity: 0.5;
    cursor: default;
    color: var(--color-subtle);
  }
  .quiet-btn {
    height: 26px;
    padding: 0 8px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--color-secondary);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .quiet-btn:hover {
    background: var(--color-hover);
    color: var(--color-fg);
  }
  .export-btn {
    height: 26px;
    padding: 0 11px;
    border: 1px solid var(--color-border);
    border-radius: 7px;
    background: var(--color-fg);
    color: var(--color-panel);
    font: inherit;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
  }
  .export-btn:hover { opacity: 0.88; }
</style>
