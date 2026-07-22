<script lang="ts">
  import { scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";

  export type ContextMenuItem =
    | {
        type: "item";
        label: string;
        shortcut?: string;
        disabled?: boolean;
        danger?: boolean;
        icon?: string;
        hasSubmenu?: boolean;
        onclick: () => void;
      }
    | { type: "separator" }
    | { type: "header"; label: string }
    | { type: "search"; placeholder?: string; value: string; oninput: (val: string) => void };

  let {
    x,
    y,
    items,
    onclose,
  }: {
    x: number;
    y: number;
    items: ContextMenuItem[];
    onclose: () => void;
  } = $props();

  let menuEl = $state<HTMLDivElement | null>(null);
  let adjustedX = $state(0);
  let adjustedY = $state(0);

  $effect(() => {
    adjustedX = x;
    adjustedY = y;

    if (menuEl) {
      const rect = menuEl.getBoundingClientRect();
      const vw = window.innerWidth;
      const vh = window.innerHeight;

      if (x + rect.width > vw - 12) {
        adjustedX = Math.max(12, vw - rect.width - 12);
      }
      if (y + rect.height > vh - 12) {
        adjustedY = Math.max(12, vh - rect.height - 12);
      }
    }
  });

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    }
  }

  function handleItemClick(item: ContextMenuItem) {
    if (item.type !== "item") return;
    if (item.disabled) return;
    item.onclick();
    onclose();
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

<!-- Backdrop scrim — click to close -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="fixed inset-0 z-[200]"
  onclick={onclose}
  oncontextmenu={(e) => { e.preventDefault(); onclose(); }}
></div>

<!-- Context menu card -->
<div
  bind:this={menuEl}
  role="menu"
  tabindex="-1"
  transition:scale={{ duration: 130, start: 0.94, easing: cubicOut }}
  class="ctx-menu"
  style="left: {adjustedX}px; top: {adjustedY}px;"
>
  {#each items as item, i (i)}
    {#if item.type === "separator"}
      <div class="ctx-sep"></div>
    {:else if item.type === "header"}
      <div class="ctx-header">{item.label}</div>
    {:else if item.type === "search"}
      <div class="ctx-search-wrap">
        <svg class="ctx-search-icon" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8"/>
          <path d="m21 21-4.3-4.3"/>
        </svg>
        <input
          type="text"
          class="ctx-search-input"
          placeholder={item.placeholder || "Search..."}
          value={item.value}
          oninput={(e) => item.oninput((e.target as HTMLInputElement).value)}
        />
      </div>
    {:else}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <button
        role="menuitem"
        class="ctx-item {item.disabled ? 'ctx-item--disabled' : ''} {item.danger ? 'ctx-item--danger' : ''}"
        onclick={() => handleItemClick(item)}
        disabled={item.disabled}
      >
        <div class="ctx-item-left">
          {#if item.icon}
            <span class="ctx-item-icon">{@html item.icon}</span>
          {/if}
          <span class="ctx-item-label">{item.label}</span>
        </div>

        {#if item.shortcut}
          <span class="ctx-item-shortcut">{item.shortcut}</span>
        {:else if item.hasSubmenu}
          <svg class="ctx-item-arrow" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
            <path d="m9 18 6-6-6-6"/>
          </svg>
        {/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .ctx-menu {
    position: fixed;
    z-index: 210;
    min-width: 190px;
    max-width: 300px;
    background: rgba(23, 23, 23, 0.88);
    -webkit-backdrop-filter: blur(30px) saturate(140%);
    backdrop-filter: blur(30px) saturate(140%);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 14px;
    padding: 5px;
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.04),
      0 16px 36px -8px rgba(0, 0, 0, 0.65),
      0 0 0 1px rgba(255, 255, 255, 0.05);
    transform-origin: top left;
  }

  /* Support Classic Look theme */
  :global(.classic-look) .ctx-menu {
    border-radius: 6px !important;
    border: 1px solid var(--color-border-input) !important;
    background: var(--color-panel, #1e1e20) !important;
    backdrop-filter: none !important;
    -webkit-backdrop-filter: none !important;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5) !important;
  }

  .ctx-sep {
    height: 1px;
    margin: 4px 6px;
    background: rgba(255, 255, 255, 0.08);
  }

  .ctx-header {
    padding: 6px 10px 4px 10px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: rgba(255, 255, 255, 0.45);
  }

  .ctx-search-wrap {
    position: relative;
    display: flex;
    align-items: center;
    margin-bottom: 4px;
    padding: 0 2px;
  }

  .ctx-search-icon {
    position: absolute;
    left: 10px;
    color: rgba(255, 255, 255, 0.4);
    pointer-events: none;
  }

  .ctx-search-input {
    width: 100%;
    padding: 6px 10px 6px 28px;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    color: #ffffff;
    font-size: 12px;
    outline: none;
  }

  .ctx-search-input:focus {
    border-color: rgba(255, 255, 255, 0.25);
    background: rgba(0, 0, 0, 0.4);
  }

  .ctx-item {
    appearance: none;
    -webkit-appearance: none;
    border: none;
    background: transparent;
    font: inherit;
    color: #ffffff;
    cursor: pointer;

    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    width: 100%;
    padding: 7px 10px;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 450;
    text-align: left;
    letter-spacing: -0.01em;
    transition: background 120ms ease, color 120ms ease;
  }

  .ctx-item-left {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    flex: 1;
  }

  .ctx-item-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 15px;
    height: 15px;
    color: rgba(255, 255, 255, 0.7);
    flex-shrink: 0;
  }

  .ctx-item-label {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: #ffffff;
  }

  .ctx-item:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.09);
    color: #ffffff;
  }

  .ctx-item:hover:not(:disabled) .ctx-item-icon {
    color: #ffffff;
  }

  .ctx-item:active:not(:disabled) {
    background: rgba(255, 255, 255, 0.15);
  }

  .ctx-item--disabled {
    color: rgba(255, 255, 255, 0.3);
    cursor: default;
  }

  .ctx-item--disabled .ctx-item-label {
    color: rgba(255, 255, 255, 0.3);
  }

  .ctx-item--danger {
    color: #ff5555;
  }

  .ctx-item--danger:hover:not(:disabled) {
    background: rgba(255, 85, 85, 0.15);
    color: #ff6666;
  }

  .ctx-item-shortcut {
    flex-shrink: 0;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 10px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.5);
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 5px;
    padding: 1.5px 6px;
    letter-spacing: 0.02em;
    line-height: 1.2;
    transition: background 120ms ease, color 120ms ease, border-color 120ms ease;
  }

  .ctx-item:hover:not(:disabled) .ctx-item-shortcut {
    color: rgba(255, 255, 255, 0.85);
    background: rgba(255, 255, 255, 0.14);
    border-color: rgba(255, 255, 255, 0.15);
  }

  .ctx-item-arrow {
    flex-shrink: 0;
    color: rgba(255, 255, 255, 0.4);
    transition: color 120ms ease, transform 120ms ease;
  }

  .ctx-item:hover:not(:disabled) .ctx-item-arrow {
    color: rgba(255, 255, 255, 0.9);
    transform: translateX(1px);
  }
</style>
