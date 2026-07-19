<script lang="ts">
  import { onMount } from "svelte";
  import Router from "svelte-spa-router";
  import routes from "./routes";
  import TitleBar from "./lib/components/shell/TitleBar.svelte";
  import SettingsModal from "./lib/components/shell/SettingsModal.svelte";
  import ExportModal from "./lib/components/shell/ExportModal.svelte";
  import {
    isSettingsOpen,
    isExportOpen,
    classicLook,
    themeTransitionActive,
    themeFlashActive,
    themeTransitionTarget,
  } from "./stores/ui";
  import { fade } from "svelte/transition";
  import { initEngineBridge } from "./lib/engine/boot";
  import { undo, redo } from "./ipc/commands";
  import { reconcile } from "./stores/doc";
  import { refreshFolders } from "./stores/browse";

  onMount(() => {
    // If not running in Tauri (e.g. running in standard browser preview),
    // set a dark background for the page so the rounded transparent corners don't show white.
    // In Tauri, the backing window is transparent, so we leave it transparent.
    if (!(window as any).__TAURI_INTERNALS__) {
      document.documentElement.style.backgroundColor = "#111113";
    }

    const teardown = initEngineBridge();
    void refreshFolders();

    function onKey(e: KeyboardEvent) {
      const t = e.target as HTMLElement | null;
      if (
        t &&
        (t.isContentEditable ||
          ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName))
      ) {
        return;
      }
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === "z" && !e.shiftKey) {
        e.preventDefault();
        void undo().then(reconcile).catch(() => {});
      } else if (
        mod &&
        (e.key.toLowerCase() === "y" ||
          (e.key.toLowerCase() === "z" && e.shiftKey))
      ) {
        e.preventDefault();
        void redo().then(reconcile).catch(() => {});
      }
    }
    window.addEventListener("keydown", onKey);

    return () => {
      teardown();
      window.removeEventListener("keydown", onKey);
    };
  });
</script>

<div
  class="relative flex h-full flex-col overflow-hidden rounded-[33px] border border-black/40 bg-window transition-all duration-300"
  class:classic-look={$classicLook}
>
  <TitleBar />
  <main class="min-h-0 flex-1">
    <Router {routes} />
  </main>

  {#if $isSettingsOpen}
    <SettingsModal />
  {/if}

  {#if $isExportOpen}
    <ExportModal />
  {/if}

  <!-- Shutter flash: CSS opacity transition, not {#if}, so it fades instead of popping -->
  <div
    class="absolute inset-0 z-[1000] pointer-events-none transition-opacity duration-[80ms] ease-in-out {$themeFlashActive
      ? 'opacity-70'
      : 'opacity-0'}"
    style="background: white;"
  ></div>

  {#if $themeTransitionActive}
    <div
      class="absolute inset-0 z-[999] flex flex-col items-center justify-center bg-black/95 text-white pointer-events-auto"
      transition:fade={{ duration: 250 }}
    >
      <div class="relative w-[120px] h-[90px] mb-2">
        <div class="absolute size-[64px] left-[15px] top-[5px] animate-gear-cw">
          <svg class="size-full text-white/90" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.1a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
            <circle cx="12" cy="12" r="3"/>
          </svg>
        </div>

        <div class="absolute size-[40px] left-[63px] top-[39px] animate-gear-ccw">
          <svg class="size-full text-white/50" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.1a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
            <circle cx="12" cy="12" r="3"/>
          </svg>
        </div>
      </div>

      <div class="mt-4 text-[10px] font-bold tracking-widest text-white/40">
        {#if $themeTransitionTarget === "classic"}
          Switching to classic theme...
        {:else}
          Switching to standard theme...
        {/if}
      </div>
    </div>
  {/if}
</div>
