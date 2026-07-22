<script lang="ts">
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";

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

  let track: HTMLDivElement;
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
    const r = track.getBoundingClientRect();
    const f = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    return quantize(min + f * (max - min));
  }

  function onpointerdown(e: PointerEvent) {
    if (disabled || !track) return;
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
  class="group relative h-[13px] w-full touch-none outline-none select-none {disabled ? 'opacity-35 cursor-default' : 'cursor-pointer'}"
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  onpointercancel={onpointerup}
  onlostpointercapture={onpointerup}
  onpointerenter={() => (isHovered = !disabled)}
  onpointerleave={() => (isHovered = false)}
  {onkeydown}
  ondblclick={() => {
    if (!disabled) {
      const reset = quantize(resetValue);
      localValue = reset;
      if (onchange) onchange(reset);
      else oninput?.(reset);
    }
  }}
>
  <!-- Background Track -->
  <div
    class="absolute inset-x-[2px] top-1/2 h-[3px] -translate-y-1/2 rounded-full bg-[#6f6f6f]/40 group-hover:bg-[#6f6f6f]/55 transition-colors duration-200"
  ></div>

  <!-- Highlight Track (fills from center/0 for bipolar sliders, or from left for monopolar) -->
  <div
    class="absolute top-1/2 h-[3px] -translate-y-1/2 rounded-full bg-white/70 group-hover:bg-white/85 transition-colors duration-200"
    style="left: calc({leftPercent}% + 2px); width: calc({Math.max(0, widthPercent)}% - 4px);"
  ></div>

  <!-- Handle (scales up dynamically on hover or dragging) -->
  <div
    class="absolute top-1/2 h-[8px] w-[17px] rounded-full bg-[#d9d9d9] shadow-md transition-all duration-150 ease-[cubic-bezier(0.25,1,0.5,1)]"
    style="
      left: calc({frac} * (100% - 17px));
      transform: translate(0, -50%) scale({isHovered || isDragging ? 1.25 : 1});
      background-color: {isHovered || isDragging ? '#ffffff' : '#d9d9d9'};
    "
  ></div>

  <!-- Floating Tactile Tooltip -->
  {#if isHovered || isDragging}
    <div
      transition:fly={{ y: 4, duration: 150, easing: cubicOut }}
      class="absolute bottom-[16px] -translate-x-1/2 px-2 py-0.5 rounded-[4px] bg-[#1a1a1c] border border-white/10 text-[9px] font-mono font-bold text-white shadow-[0_4px_12px_rgba(0,0,0,0.5)] pointer-events-none z-[100]"
      style="left: calc({frac} * (100% - 17px) + 8.5px);"
    >
      {displayValue > 0 && min < 0 ? "+" : ""}{fmt(displayValue)}
    </div>
  {/if}
</div>
