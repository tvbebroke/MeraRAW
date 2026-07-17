<script lang="ts">
  import { onMount } from "svelte";
  import Router from "svelte-spa-router";
  import routes from "./routes";
  import TitleBar from "./lib/components/shell/TitleBar.svelte";
  import SettingsModal from "./lib/components/shell/SettingsModal.svelte";
  import ExportModal from "./lib/components/shell/ExportModal.svelte";
  import { isSettingsOpen, isExportOpen } from "./stores/ui";
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
  class="relative flex h-full flex-col overflow-hidden rounded-[33px] border border-black/40 bg-window"
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
</div>
