<script lang="ts">
  import type { Snippet } from "svelte";
  import { slide } from "svelte/transition";
  import {
    DEFAULT_SECTIONS,
    openSections,
    toggleSection,
    type SectionId,
  } from "../../../stores/editor";
  import { registry } from "../../engine/params";
  import { doc } from "../../../stores/doc";
  import { imageMeta, selectedMask } from "../../../stores/app";
  import {
    resetSection,
    sectionCanReset,
    sectionIsModified,
  } from "../../editor/sectionState";

  let {
    id,
    title,
    children,
  }: { id: SectionId; title: string; children?: Snippet } = $props();

  const known = $derived(Object.prototype.hasOwnProperty.call(DEFAULT_SECTIONS, id));
  const open = $derived(known ? $openSections[id] : true);
  const modified = $derived(
    sectionIsModified(id, $doc, $registry, $imageMeta, $selectedMask),
  );

  $effect(() => {
    if (!known) {
      console.error(
        `[CollapsibleSection] unregistered id "${String(id)}" — section forced open`,
      );
    }
  });

  function onToggle() {
    toggleSection(id);
  }

  function onReset(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (!sectionCanReset(id)) return;
    void resetSection(id, $registry, $imageMeta, $selectedMask);
  }
</script>

<section class="acc" class:is-open={open} data-section={id}>
  <div class="acc-head">
    {#if modified}
      <span class="acc-dot" aria-hidden="true"></span>
    {/if}
    <button
      type="button"
      class="acc-main"
      onclick={onToggle}
      aria-expanded={open}
    >
      <span class="acc-title">{title}</span>
    </button>
    {#if modified && sectionCanReset(id)}
      <button type="button" class="acc-reset" onclick={onReset}>Reset</button>
    {/if}
    <button
      type="button"
      class="acc-chevron"
      class:open
      onclick={onToggle}
      tabindex="-1"
      aria-hidden="true"
    >›</button>
  </div>
  {#if import.meta.env.DEV && !known}
    <div class="px-[12px] pb-[4px] rail-empty">unregistered section: {id}</div>
  {/if}
  {#if open}
    <div class="acc-body" transition:slide={{ duration: 180 }}>
      {@render children?.()}
    </div>
  {/if}
</section>

<style>
  .acc {
    width: 100%;
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-border);
  }
  .acc-head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    min-height: 40px;
    padding: var(--space-2) var(--space-3);
  }
  .acc-head:hover { background: var(--color-hover); }
  .acc-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    min-height: 40px;
    padding: 0;
    border: 0;
    background: transparent;
    cursor: pointer;
    text-align: left;
  }
  .acc-dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--color-fg);
    flex: none;
  }
  .acc-title {
    flex: 1;
    min-width: 0;
    font-size: var(--text-title);
    font-weight: 500;
    color: var(--color-fg);
  }
  .acc-reset {
    flex: none;
    border: 0;
    background: transparent;
    font-size: var(--text-ui);
    color: var(--color-subtle);
    opacity: 0;
    pointer-events: none;
    cursor: pointer;
  }
  .acc-head:hover .acc-reset {
    opacity: 1;
    pointer-events: auto;
  }
  .acc-reset:hover { color: var(--color-fg); }
  .acc-chevron {
    flex: none;
    border: 0;
    background: transparent;
    padding: 0;
    font-size: 16px;
    line-height: 1;
    color: var(--color-subtle);
    cursor: pointer;
    transform: rotate(0deg);
    transition: transform 0.15s var(--ease-std);
  }
  .acc-chevron.open { transform: rotate(90deg); }
  .acc-body {
    padding: var(--space-1) var(--space-3) var(--space-4);
  }
</style>
