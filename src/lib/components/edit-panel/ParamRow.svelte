<!-- Registry-driven slider row: label + value + thin track. -->
<script lang="ts">
  import Slider from "./Slider.svelte";
  import { registry, effectiveValue, defaultValue, setParamLive, commitParam } from "../../engine/params";
  import { doc } from "../../../stores/doc";
  import { imageMeta, selectedMask } from "../../../stores/app";

  let {
    path,
    label,
    disabled = false,
    value: valueProp,
    min: minProp,
    max: maxProp,
    step: stepProp,
    resetValue: resetProp,
    oninput,
    onchange,
  }: {
    path?: string;
    label?: string;
    disabled?: boolean;
    value?: number;
    min?: number;
    max?: number;
    step?: number;
    resetValue?: number;
    oninput?: (v: number) => void;
    onchange?: (v: number) => void;
  } = $props();

  const spec = $derived(path ? $registry.find((s) => s.path === path) : undefined);
  const registryReady = $derived($registry.length > 0);
  const bound = $derived(!!path);
  const value = $derived(
    bound
      ? spec
        ? effectiveValue($doc, spec, $imageMeta, $selectedMask)
        : 0
      : (valueProp ?? 0),
  );
  const reset = $derived(
    bound
      ? spec
        ? defaultValue(spec, $imageMeta, $selectedMask)
        : 0
      : (resetProp ?? 0),
  );
  const min = $derived(bound ? (spec?.min ?? 0) : (minProp ?? 0));
  const max = $derived(bound ? (spec?.max ?? 100) : (maxProp ?? 100));
  const step = $derived(bound ? (spec?.ui.step ?? 1) : (stepProp ?? 1));
  const displayLabel = $derived(label ?? spec?.ui.label ?? "");

  let editing = $state(false);
  let draft = $state("");
  let numEl = $state<HTMLInputElement | null>(null);

  function fmt(v: number): string {
    const sign = v > 0 && min < 0 ? "+" : "";
    if (Math.abs(v) >= 10 || Number.isInteger(v)) return `${sign}${v.toFixed(0)}`;
    return `${sign}${v.toFixed(2)}`;
  }

  function startEdit() {
    if (disabled || (bound && !spec)) return;
    editing = true;
    draft = String(value);
    requestAnimationFrame(() => {
      numEl?.focus();
      numEl?.select();
    });
  }

  function commitDraft() {
    editing = false;
    const n = Number(draft);
    if (!Number.isFinite(n)) return;
    if (bound && path) void commitParam(path, n);
    else onchange?.(n);
  }

  $effect(() => {
    if (bound && registryReady && !spec) {
      console.error(`[ParamRow] no registry entry for "${path}"`);
    }
  });
</script>

{#if bound && !spec}
  {#if import.meta.env.DEV && registryReady}
    <div class="rail-empty">missing param: {path}</div>
  {/if}
{:else}
  <div class="param-row">
    <span class="label" title={displayLabel}>{displayLabel}</span>
    {#if editing}
      <input
        bind:this={numEl}
        class="value num"
        bind:value={draft}
        onblur={commitDraft}
        onkeydown={(e) => {
          if (e.key === "Enter") commitDraft();
          if (e.key === "Escape") editing = false;
        }}
      />
    {:else}
      <button type="button" class="value num" onclick={startEdit} disabled={disabled}>
        {fmt(value)}
      </button>
    {/if}
    <div class="slider">
      <Slider
        label={displayLabel}
        {min}
        {max}
        {step}
        {value}
        resetValue={reset}
        {disabled}
        oninput={(v) => {
          if (bound && path) setParamLive(path, v);
          else oninput?.(v);
        }}
        onchange={(v) => {
          if (bound && path) void commitParam(path, v);
          else onchange?.(v);
        }}
      />
    </div>
  </div>
{/if}

<style>
  .param-row {
    display: grid;
    grid-template-columns: 1fr auto;
    grid-template-areas: "label value" "track track";
    row-gap: 6px;
    padding: var(--space-1) 0;
  }
  .label {
    grid-area: label;
    font-size: var(--text-ui);
    font-weight: 400;
    color: var(--color-fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .value {
    grid-area: value;
    font-size: var(--text-num);
    color: var(--color-subtle);
    border: 0;
    background: transparent;
    padding: 0;
    cursor: text;
    text-align: right;
  }
  .value:hover:not(:disabled) { color: var(--color-fg); }
  input.value {
    width: 56px;
    border-bottom: 1px solid var(--color-border-strong);
    outline: none;
  }
  .slider {
    grid-area: track;
  }
</style>
