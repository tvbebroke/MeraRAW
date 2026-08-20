<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { router, push } from "svelte-spa-router";
  import IconButton from "../primitives/IconButton.svelte";
  import settingsIcon from "../../icons/settings.svg";
  import libraryIcon from "../../icons/library.svg";
  import helpIcon from "../../icons/help.svg";
  import logoIcon from "../../icons/logo.png";
  import { isSettingsOpen, isExportOpen, isShortcutsOpen } from "../../../stores/ui";
  import JobsPill from "./JobsPill.svelte";
  import VersionsButton from "./VersionsButton.svelte";
  import { commandPaletteOpen } from "../../../stores/editor";
  import { pickFile } from "../../fs";
  import { openPath } from "../../engine/boot";
  import { shortcutLabels } from "../../shortcuts";
  import { lastOpenedPath, imageMeta } from "../../../stores/app";
  import { folder, folders, activePhoto } from "../../../stores/browse";
  import { undo, redo } from "../../../ipc/commands";
  import { reconcile } from "../../../stores/doc";
  import {
    isLibraryRoute,
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

  const folderName = $derived(
    $folders.find((f) => f.root === $folder)?.name || basename($folder) || "",
  );
  const fileName = $derived(
    $activePhoto?.filename || basename($imageMeta?.path) || basename($lastOpenedPath),
  );
</script>

<header
  data-tauri-drag-region
  class="relative flex h-[42px] shrink-0 items-center pr-3 {isFullscreen ? 'pl-4' : 'pl-[76px]'} border-b border-border/80"
>
  <img
    src={logoIcon}
    alt=""
    class="size-[16px] select-none rounded-[3px]"
    draggable="false"
    data-tauri-drag-region
  />
  <span data-tauri-drag-region class="ml-[8px] text-[13px] font-medium text-fg">MeraRAW</span>
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
      role="tab"
      class="ws-chip"
      class:is-on={isVideo}
      aria-selected={isVideo}
      onclick={() => setWorkspace("video")}
    >Video</button>
  </div>
  {#if folderName || fileName}
    <span class="crumb-sep" aria-hidden="true">/</span>
    {#if folderName}
      <span class="crumb-muted truncate max-w-[10rem]" title={$folder ?? ""}>{folderName}</span>
    {/if}
    {#if fileName}
      <span class="crumb-sep" aria-hidden="true">/</span>
      <span class="crumb-file truncate max-w-[16rem]" title={fileName}>{fileName}</span>
    {/if}
  {/if}

  <div class="ml-auto flex items-center gap-[4px]" data-tauri-drag-region="false">
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
      <IconButton
        icon={libraryIcon}
        label="Library"
        title={isVideo ? "Clips" : `Library (${shortcutLabels.library})`}
        iconClass="h-[15px] w-[19px]"
        onclick={() => safePush(isVideo ? "/clips" : "/library")}
      />
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
  .crumb-sep {
    margin: 0 7px;
    color: var(--color-subtle);
    opacity: 0.55;
    font-size: 13px;
  }
  .crumb-muted {
    font-size: 13px;
    color: var(--color-subtle);
  }
  .crumb-file {
    font-size: 13px;
    color: var(--color-fg);
  }
  .ws-switch {
    display: flex;
    gap: 2px;
    margin-left: 12px;
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
