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
  import { doc, docParam, reconcile } from "../../../stores/doc";
  import { imageMeta, selectedMask } from "../../../stores/app";
  import {
    resetSection,
    sectionCanReset,
    sectionIsModified,
  } from "../../editor/sectionState";
  import { setParam } from "../../../ipc/commands";

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

  const enabled = $derived.by(() => {
    if (id === "light") return (docParam($doc, "exposure", "enabled") ?? 1) >= 0.5;
    if (id === "color") return (docParam($doc, "white_balance", "enabled") ?? 1) >= 0.5;
    if (id === "curve") return (docParam($doc, "tone_curve", "enabled") ?? 1) >= 0.5;
    if (id === "detail") return (docParam($doc, "detail", "enabled") ?? 1) >= 0.5;
    if (id === "grading") return (docParam($doc, "color_grade", "enabled") ?? 1) >= 0.5;
    if (id === "camera") return (docParam($doc, "calibration", "enabled") ?? 1) >= 0.5;
    return true;
  });
  const hasPower = $derived(
    id === "light" || id === "color" || id === "curve" || id === "detail" || id === "grading" || id === "camera",
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

  function onPower(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    const next = enabled ? 0 : 1;
    if (id === "light") void setParam("exposure.enabled", next).then(reconcile);
    else if (id === "color") void setParam("white_balance.enabled", next).then(reconcile);
    else if (id === "curve") void setParam("tone_curve.enabled", next).then(reconcile);
    else if (id === "detail") void setParam("detail.enabled", next).then(reconcile);
    else if (id === "grading") void setParam("color_grade.enabled", next).then(reconcile);
    else if (id === "camera") void setParam("calibration.enabled", next).then(reconcile);
  }
</script>

<section class="acc" class:is-open={open} class:is-off={hasPower && !enabled} data-section={id}>
  <div class="acc-head">
    {#if hasPower}
      <button
        type="button"
        class="acc-power"
        class:on={enabled}
        onclick={onPower}
        title={enabled ? "Disable module" : "Enable module"}
        aria-pressed={enabled}
      ></button>
    {:else if modified}
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
  .acc.is-off { opacity: 0.45; }
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
  .acc-power {
    flex: none;
    width: 10px;
    height: 10px;
    padding: 0;
    border: 1px solid var(--color-subtle);
    border-radius: 50%;
    background: transparent;
    cursor: pointer;
  }
  .acc-power.on {
    background: var(--color-fg);
    border-color: var(--color-fg);
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
