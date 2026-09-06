<script lang="ts">
  import ContextMenu from "../primitives/ContextMenu.svelte";
  import type { ContextMenuItem } from "../primitives/ContextMenu.svelte";

  let {
    value = 0,
    min = -100,
    max = 100,
    step = 1,
    label,
    resetValue = 0,
    disabled = false,
    onchange,
    oninput,
  }: {
    value?: number;
    min?: number;
    max?: number;
    step?: number;
    label: string;
    resetValue?: number;
    disabled?: boolean;
    /** Final committed value (pointer release / keyboard / reset). */
    onchange?: (v: number) => void;
    /** Live mid-drag value (throttled engine-side). */
    oninput?: (v: number) => void;
  } = $props();

  let track = $state<HTMLDivElement | null>(null);
  let isHovered = $state(false);
  let isDragging = $state(false);
  let localValue = $state(0);

  const displayValue = $derived(isDragging ? localValue : value);
  const frac = $derived((displayValue - min) / (max - min));

  // Determine highlight track geometry
  const zeroFrac = $derived(min < 0 ? -min / (max - min) : 0);
  const leftPercent = $derived(min < 0
    ? (frac >= zeroFrac ? zeroFrac : frac) * 100
    : 0);
  const widthPercent = $derived(min < 0
    ? Math.abs(frac - zeroFrac) * 100
    : frac * 100);

  function quantize(raw: number): number {
    const q = Math.round((raw - min) / step) * step + min;
    const v = Math.min(max, Math.max(min, q));
    // avoid float dust like 0.30000000000000004
    return parseFloat(v.toFixed(6));
  }

  function valueFromEvent(e: PointerEvent): number {
    if (!track) return value;
    const r = track.getBoundingClientRect();
    const f = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    return quantize(min + f * (max - min));
  }

  function onpointerdown(e: PointerEvent) {
    if (disabled || !track) return;
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    isDragging = true;
    const val = valueFromEvent(e);
    localValue = val;
    if (oninput) oninput(val);
    else onchange?.(val);
  }

  function onpointermove(e: PointerEvent) {
    if (isDragging && !disabled) {
      const val = valueFromEvent(e);
      localValue = val;
      if (oninput) oninput(val);
      else onchange?.(val);
    }
  }

  function onpointerup(e: PointerEvent) {
    if (isDragging && !disabled) {
      const val = valueFromEvent(e);
      localValue = val;
      onchange?.(val);
    }
    isDragging = false;
  }

  function onkeydown(e: KeyboardEvent) {
    if (disabled) return;
    let nextVal = value;
    if (e.key === "ArrowLeft") nextVal = quantize(value - step);
    if (e.key === "ArrowRight") nextVal = quantize(value + step);
    if (nextVal !== value) {
      localValue = nextVal;
      if (onchange) onchange(nextVal);
      else oninput?.(nextVal);
    }
  }

  function fmt(v: number): string {
    if (Math.abs(v) >= 10 || Number.isInteger(v)) return v.toFixed(0);
    return v.toFixed(2);
  }

  function resetToDefault() {
    const v = quantize(resetValue);
    localValue = v;
    if (onchange) onchange(v);
    else oninput?.(v);
  }

  let ctxMenu = $state<{ x: number; y: number } | null>(null);

  function oncontextmenu(e: MouseEvent) {
    if (disabled) return;
    e.preventDefault();
    e.stopPropagation();
    ctxMenu = { x: e.clientX, y: e.clientY };
  }

  const ctxItems: ContextMenuItem[] = [
    { type: "item", label: "Reset to Default", onclick: resetToDefault },
  ];
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
<div
  bind:this={track}
  role="slider"
  tabindex={disabled ? -1 : 0}
  aria-label={label}
  aria-valuenow={displayValue}
  aria-valuemin={min}
  aria-valuemax={max}
  aria-disabled={disabled}
  title="{label}: {fmt(displayValue)}"
  class="group relative h-[20px] -my-[4px] w-full touch-none outline-none select-none {disabled ? 'opacity-35 cursor-default' : 'cursor-pointer'}"
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  onpointercancel={onpointerup}
  onlostpointercapture={onpointerup}
  onpointerenter={() => (isHovered = !disabled)}
  onpointerleave={() => (isHovered = false)}
  {onkeydown}
  {oncontextmenu}
  ondblclick={() => {
    if (!disabled) resetToDefault();
  }}
>
  <!-- Background Track -->
  <div
    class="absolute inset-x-0 top-1/2 h-[2px] -translate-y-1/2 rounded-full bg-white/[0.08] group-hover:bg-white/[0.12] transition-colors duration-150"
  ></div>

  <!-- Highlight Track -->
  <div
    class="absolute top-1/2 h-[2px] -translate-y-1/2 rounded-full bg-secondary group-hover:bg-fg transition-colors duration-150"
    style="left: {leftPercent}%; width: {Math.max(0, widthPercent)}%;"
  ></div>

  <!-- Handle -->
  <div
    class="absolute top-1/2 size-[10px] rounded-full bg-[#ecece8] shadow-[0_0_0_1px_rgba(0,0,0,0.35)] transition-transform duration-150 ease-[cubic-bezier(0.25,1,0.5,1)]"
    style="
      left: calc({frac} * (100% - 10px));
      transform: translate(0, -50%) scale({isHovered || isDragging ? 1.15 : 1});
    "
  ></div>
</div>

{#if ctxMenu}
  <ContextMenu
    x={ctxMenu.x}
    y={ctxMenu.y}
    items={ctxItems}
    onclose={() => (ctxMenu = null)}
  />
{/if}
