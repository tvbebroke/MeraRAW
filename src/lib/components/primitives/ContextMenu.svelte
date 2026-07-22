<script lang="ts">
  import { scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";


  export type ContextMenuItem =
    | { type: "item"; label: string; shortcut?: string; disabled?: boolean; onclick: () => void }
    | { type: "separator" };

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

      if (x + rect.width > vw - 8) {
        adjustedX = vw - rect.width - 8;
      }
      if (y + rect.height > vh - 8) {
        adjustedY = vh - rect.height - 8;
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
    if (item.type === "separator") return;
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
  transition:scale={{ duration: 140, start: 0.92, easing: cubicOut }}
  class="ctx-menu"
  style="left: {adjustedX}px; top: {adjustedY}px;"
>
  {#each items as item, i (i)}
    {#if item.type === "separator"}
      <div class="ctx-sep"></div>
    {:else}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <button
        role="menuitem"
        class="ctx-item {item.disabled ? 'ctx-item--disabled' : ''}"
        onclick={() => handleItemClick(item)}
        disabled={item.disabled}
      >
        <span class="ctx-item-label">{item.label}</span>
        {#if item.shortcut}
          <span class="ctx-item-shortcut">{item.shortcut}</span>
        {/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .ctx-menu {
    position: fixed;
    z-index: 210;
    min-width: 180px;
    max-width: 280px;
    background: rgba(22, 22, 24, 0.94);
    -webkit-backdrop-filter: blur(24px) saturate(140%);
    backdrop-filter: blur(24px) saturate(140%);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    padding: 4px;
    box-shadow:
      0 0 0 0.5px rgba(0, 0, 0, 0.3),
      0 8px 30px rgba(0, 0, 0, 0.45),
      0 2px 8px rgba(0, 0, 0, 0.2);
    transform-origin: top left;
  }

  .ctx-sep {
    height: 1px;
    margin: 3px 8px;
    background: rgba(255, 255, 255, 0.06);
  }

  .ctx-item {
    appearance: none;
    -webkit-appearance: none;
    border: none;
    background: transparent;
    font: inherit;
    color: rgba(255, 255, 255, 0.8);
    cursor: pointer;

    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    width: 100%;
    padding: 6px 10px;
    border-radius: 6px;
    font-size: 12px;
    text-align: left;
    transition: background 100ms ease, color 100ms ease;
  }

  .ctx-item:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.95);
  }

  .ctx-item:active:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
  }

  .ctx-item--disabled {
    color: rgba(255, 255, 255, 0.25);
    cursor: default;
  }

  .ctx-item-label {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ctx-item-shortcut {
    flex-shrink: 0;
    font-size: 10px;
    color: rgba(255, 255, 255, 0.3);
    font-weight: 500;
    letter-spacing: 0.02em;
  }
</style>
