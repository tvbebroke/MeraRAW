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
  /* Flat popover: hairline border + one shadow layer, no blur.
     Radius follows --radius so .classic-look squares it off for free. */
  .ctx-menu {
    position: fixed;
    z-index: 210;
    min-width: 190px;
    max-width: 300px;
    background: var(--color-panel);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius);
    padding: 5px;
    box-shadow: var(--shadow-popover);
    transform-origin: top left;
  }

  .ctx-sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--color-border);
  }

  /* Menu section headers are sentence case + semibold.
     Uppercase-tracked caps are reserved for panel eyebrows. */
  .ctx-header {
    padding: 6px 10px 4px 10px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--color-fg);
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
    color: var(--color-subtle);
    pointer-events: none;
  }

  .ctx-search-input {
    width: 100%;
    padding: 6px 10px 6px 28px;
    background: var(--color-sunken);
    border: 1px solid var(--color-border);
    border-radius: 7px;
    color: var(--color-fg);
    font-size: 12px;
    outline: none;
    transition: border-color 0.15s var(--ease-std);
  }

  /* Focus = border colour change only. No ring, no glow. */
  .ctx-search-input:focus {
    border-color: var(--color-accent);
  }

  .ctx-search-input::placeholder {
    color: var(--color-subtle);
  }

  .ctx-item {
    appearance: none;
    -webkit-appearance: none;
    border: none;
    background: transparent;
    font: inherit;
    color: var(--color-fg);
    cursor: pointer;

    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    width: 100%;
    padding: 6px 10px;
    border-radius: 6px;
    font-size: 12.5px;
    font-weight: 450;
    text-align: left;
    transition: background-color 0.12s var(--ease-std), color 0.12s var(--ease-std);
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
    color: var(--color-secondary);
    flex-shrink: 0;
  }

  .ctx-item-label {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: inherit;
  }

  .ctx-item:hover:not(:disabled) {
    background: var(--color-hover);
    color: var(--color-fg);
  }

  .ctx-item:hover:not(:disabled) .ctx-item-icon {
    color: var(--color-fg);
  }

  .ctx-item:active:not(:disabled) {
    background: var(--color-active);
  }

  .ctx-item--disabled {
    color: var(--color-subtle);
    cursor: default;
    opacity: 0.6;
  }

  /* Neutral at rest — destructive intent only shows on hover. */
  .ctx-item--danger {
    color: var(--color-secondary);
  }

  .ctx-item--danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-destructive) 12%, transparent);
    color: var(--color-destructive);
  }

  /* Shortcut hint: bare mono text, right-aligned. No key-cap chrome —
     the box was competing with the label for attention. */
  .ctx-item-shortcut {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--color-subtle);
    line-height: 1.2;
    transition: color 0.12s var(--ease-std);
  }

  .ctx-item:hover:not(:disabled) .ctx-item-shortcut {
    color: var(--color-secondary);
  }

  .ctx-item-arrow {
    flex-shrink: 0;
    color: var(--color-subtle);
    transition: color 0.12s var(--ease-std);
  }

  .ctx-item:hover:not(:disabled) .ctx-item-arrow {
    color: var(--color-fg);
  }
</style>
