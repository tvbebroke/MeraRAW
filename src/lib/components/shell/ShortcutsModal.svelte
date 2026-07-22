<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { isShortcutsOpen } from "../../../stores/ui";
  import { shortcutLabels } from "../../shortcuts";

  function close() {
    isShortcutsOpen.set(false);
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }

  const shortcutGroups = [
    {
      title: "Navigation & Layout",
      items: [
        { label: "Go to Library View", key: shortcutLabels.library },
        { label: "Go to Editor View", key: shortcutLabels.edit },
        { label: "Toggle Left Sidebar", key: shortcutLabels.sidebar },
        { label: "Toggle Details Panel (Library)", key: shortcutLabels.details },
        { label: "Toggle Filmstrip (Editor)", key: shortcutLabels.filmstrip },
        { label: "Toggle Zen / Expert Mode", key: shortcutLabels.zenMode },
        { label: "Toggle Fullscreen", key: shortcutLabels.fullscreen },
      ]
    },
    {
      title: "Tool Switching",
      items: [
        { label: "Switch to Edit Panel", key: shortcutLabels.toolEdit },
        { label: "Switch to Crop Panel", key: shortcutLabels.toolCrop },
        { label: "Switch to Mask Panel", key: shortcutLabels.toolMask },
        { label: "Switch to AI Panel", key: shortcutLabels.toolAi },
        { label: "Switch to Presets Panel", key: shortcutLabels.toolPresets },
      ]
    },
    {
      title: "Image Adjustment & Modals",
      items: [
        { label: "Toggle Zoom 1:1 / Fit", key: shortcutLabels.zoom },
        { label: "Toggle Before/After Compare", key: shortcutLabels.compare },
        { label: "Open Export Modal", key: shortcutLabels.export },
        { label: "Open Settings Modal", key: shortcutLabels.settings },
        { label: "Undo last change", key: shortcutLabels.undo },
        { label: "Redo last change", key: shortcutLabels.redo },
        { label: "Next / Previous Photo", key: "← / →" },
        { label: "Open Selected Photo", key: shortcutLabels.openPhoto },
        { label: "Close active dialog / Deselect", key: shortcutLabels.escape },
      ]
    }
  ];
</script>

<svelte:window onkeydown={handleKeyDown} />

<!-- Modal Backdrop -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  transition:fade={{ duration: 200 }}
  class="backdrop absolute inset-0 z-[100] flex items-center justify-center bg-black/60 px-4 py-6"
  onclick={close}
>
  <!-- Modal Content Card -->
  <div
    transition:scale={{ duration: 250, start: 0.95 }}
    class="modal-card relative flex w-[640px] max-h-[90%] flex-col overflow-hidden rounded-[20px] border border-white/[0.06] bg-[#171717]/95 text-white backdrop-blur-md"
    onclick={(e) => e.stopPropagation()}
  >
    <!-- Modal Header -->
    <div class="flex shrink-0 items-center justify-between border-b border-white/[0.04] px-8 py-5">
      <h2 class="content-title m-0">Keyboard Shortcuts</h2>
      <button 
        class="flex size-[26px] items-center justify-center rounded-full border border-white/5 bg-white/[0.04] text-white/80 hover:bg-white/[0.12] hover:text-white active:scale-95 transition-all cursor-pointer"
        onclick={close}
        aria-label="Close dialog"
      >
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.5">
          <path d="M1.5 1.5L8.5 8.5M8.5 1.5L1.5 8.5" stroke-linecap="round" />
        </svg>
      </button>
    </div>

    <!-- Scrollable content -->
    <div class="flex-1 overflow-y-auto px-8 py-6 custom-scrollbar flex flex-col gap-6">
      {#each shortcutGroups as group}
        <section class="group-section">
          <h3 class="group-title">{group.title}</h3>
          <div class="shortcuts-grid">
            {#each group.items as item}
              <div class="shortcut-row">
                <span class="shortcut-label">{item.label}</span>
                <kbd class="shortcut-kbd">{item.key}</kbd>
              </div>
            {/each}
          </div>
        </section>
      {/each}
    </div>

    <!-- Footer Actions -->
    <footer class="flex shrink-0 items-center justify-end gap-3 border-t border-white/[0.04] px-8 py-4">
      <button class="footer-btn footer-btn--primary" onclick={close}>
        Close
      </button>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    background: rgba(0, 0, 0, 0.35);
    -webkit-backdrop-filter: blur(16px) saturate(120%);
    backdrop-filter: blur(16px) saturate(120%);
  }

  .modal-card {
    background: rgba(23, 23, 23, 0.96);
    box-shadow: 
      0 0 0 0.5px rgba(255, 255, 255, 0.06),
      0 8px 24px rgba(0, 0, 0, 0.3),
      0 24px 48px rgba(0, 0, 0, 0.25);
  }

  .content-title {
    font-size: 15px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
    letter-spacing: -0.01em;
  }

  /* Scrollbar matching modern design language */
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

  /* Shortcut Groups */
  .group-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .group-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: rgba(255, 255, 255, 0.35);
    margin: 0;
  }

  .shortcuts-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px 24px;
  }

  .shortcut-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
  }

  .shortcut-label {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.7);
  }

  .shortcut-kbd {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 24px;
    height: 20px;
    padding: 0 6px;
    font-family: inherit;
    font-size: 10px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.85);
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 4px;
    box-shadow: 0 1px 0 rgba(0, 0, 0, 0.2);
  }

  /* Footer Action Button */
  .footer-btn {
    appearance: none;
    -webkit-appearance: none;
    border: none;
    border-radius: 10px;
    padding: 8px 20px;
    font-size: 12px;
    font-family: inherit;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 150ms ease, transform 150ms ease;
  }

  .footer-btn--primary {
    background: var(--color-accent, #ffffff);
    color: #000;
  }

  .footer-btn--primary:hover {
    background: #e4e4e7;
    transform: translateY(-0.5px);
  }

  .footer-btn--primary:active {
    transform: scale(0.98) translateY(0);
  }
</style>
