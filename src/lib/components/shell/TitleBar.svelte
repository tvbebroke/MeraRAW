<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { router, push } from "svelte-spa-router";
  import IconButton from "../primitives/IconButton.svelte";
  import ToggleSwitch from "../primitives/ToggleSwitch.svelte";
  import settingsIcon from "../../icons/settings.svg";
  import libraryIcon from "../../icons/library.svg";
  import helpIcon from "../../icons/help.svg";
  import exportIcon from "../../icons/export.svg";
  import pencilIcon from "../../icons/pencil.svg";
  import logoIcon from "../../icons/logo.png";
  import { isSettingsOpen, isExportOpen, classicLook, isShortcutsOpen } from "../../../stores/ui";
  import { isZenMode } from "../../../stores/editor";
  import { pickFile } from "../../fs";
  import { openPath } from "../../engine/boot";
  import { shortcutLabels } from "../../shortcuts";

  // Lazy so the page still renders in a plain browser (no Tauri runtime)
  const win = () => getCurrentWindow();
  const isLibrary = $derived(router.location === "/library");

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
    const path = await pickFile();
    if (path) void openPath(path);
  }
</script>

<header
  data-tauri-drag-region
  class="relative flex h-[44px] shrink-0 items-center pr-4 {isFullscreen ? 'pl-4' : 'pl-[88px]'}"
>
  <img
    src={logoIcon}
    alt=""
    class="size-[20px] select-none rounded-[4px]"
    draggable="false"
    data-tauri-drag-region
  />
  <span data-tauri-drag-region class="ml-[7px] text-[14px] text-white"
    >MeraRAW</span
  >
  <span
    class="ml-[7px] rounded-[8px] border border-[#d0af36] px-[5px] pt-[4px] pb-[3px] text-[8px] leading-none text-[#d0af36]"
    >BETA</span
  >

  <div class="ml-auto flex items-center gap-[7px]">
    <button
      type="button"
      class="rounded-full border border-white/5 bg-white/[0.04] px-3 py-1.5 text-[11px] font-medium text-white/80 transition-all hover:bg-white/[0.12] hover:text-white"
      onclick={() => void openRaw()}
    >
      Open
    </button>
    <IconButton
      icon={settingsIcon}
      label="Settings"
      title="Settings ({shortcutLabels.settings})"
      iconClass="size-[20px]"
      onclick={() => isSettingsOpen.set(true)}
    />
    <IconButton
      icon={helpIcon}
      label="Help"
      title="Keyboard Shortcuts"
      iconClass="h-[18px] w-[12px]"
      onclick={() => isShortcutsOpen.set(true)}
    />

    {#if isLibrary}
      <!-- Library page: show Edit button to go back to editor -->
      <button
        onclick={() => safePush("/edit")}
        aria-label="Edit"
        title="Edit ({shortcutLabels.edit})"
        class="edit-pill-btn"
      >
        <span class="edit-pill-label">Edit</span>
        <div class="edit-pill-icon-circle">
          <img src={pencilIcon} alt="" class="edit-pill-icon" />
        </div>
      </button>
    {:else}
      <!-- Edit pages: show Library icon + Zen toggle + Export -->
      <IconButton
        icon={libraryIcon}
        label="Library"
        title="Library ({shortcutLabels.library})"
        iconClass="h-[17px] w-[21px]"
        onclick={() => safePush("/library")}
      />
      {#if !$classicLook}
        <ToggleSwitch
          checked={$isZenMode}
          label="Zen / Expert mode"
          title="Zen / Expert mode ({shortcutLabels.zenMode})"
          onchange={(zen) => isZenMode.set(zen)}
        />
      {/if}
      <IconButton
        icon={exportIcon}
        label="Export"
        title="Export ({shortcutLabels.export})"
        iconClass="size-[17px]"
        wide={true}
        onclick={() => isExportOpen.set(true)}
      />
    {/if}
  </div>
</header>

<style>
  .edit-pill-btn {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.06);
    margin: 0;
    font: inherit;
    color: rgba(255, 255, 255, 0.85);
    cursor: pointer;

    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;

    height: 36px;
    padding: 0 4px 0 16px;
    border-radius: 9999px;

    background: rgba(33, 33, 35, 0.65);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      inset 0 -1px 0 rgba(0, 0, 0, 0.2),
      0 2px 6px rgba(0, 0, 0, 0.25);
    transition: background 150ms cubic-bezier(0.4, 0, 0.2, 1), border-color 150ms cubic-bezier(0.4, 0, 0.2, 1);
    white-space: nowrap;
  }

  .edit-pill-btn:hover {
    background: rgba(47, 47, 49, 0.85);
  }

  .edit-pill-btn:active {
    background: rgba(65, 65, 68, 0.9);
  }

  .edit-pill-label {
    font-size: 13px;
    font-weight: 500;
    letter-spacing: -0.01em;
  }

  .edit-pill-icon-circle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.1);
  }

  .edit-pill-icon {
    width: 14px;
    height: 14px;
    opacity: 0.9;
  }
</style>
