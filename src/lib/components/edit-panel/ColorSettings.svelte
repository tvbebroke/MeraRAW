<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import { viewportTool } from "../../../stores/app";

  const mixerColors = [
    { id: "red", hex: "#ff2a2a", label: "Red" },
    { id: "orange", hex: "#ff9f2a", label: "Orange" },
    { id: "yellow", hex: "#ffff2a", label: "Yellow" },
    { id: "green", hex: "#2aff2a", label: "Green" },
    { id: "aqua", hex: "#2affff", label: "Aqua" },
    { id: "blue", hex: "#2a7fff", label: "Blue" },
    { id: "purple", hex: "#9f2aff", label: "Purple" },
    { id: "magenta", hex: "#ff2aff", label: "Magenta" },
  ] as const;

  let activeColor = $state<(typeof mixerColors)[number]["id"]>("red");

  function toggleWb() {
    viewportTool.set($viewportTool === "wb" ? "pan" : "wb");
  }
</script>

<CollapsibleSection id="color" title="Color">
  <div class="flex flex-col gap-[6px]">
    <div class="flex items-center gap-2">
      <div class="min-w-0 flex-1">
        <ParamRow path="white_balance.temp" label="Temp" labelWidth={48} />
      </div>
      <button
        type="button"
        title="White balance eyedropper"
        aria-label="White balance eyedropper"
        class="flex size-[22px] shrink-0 items-center justify-center rounded-full border text-[10px] transition-all {$viewportTool === 'wb'
          ? 'border-white/40 bg-white/15 text-white'
          : 'border-white/5 bg-white/[0.03] text-white/60 hover:bg-white/10'}"
        onclick={toggleWb}
      >
        ⌖
      </button>
    </div>
    <ParamRow path="white_balance.tint" label="Tint" labelWidth={48} />
    <ParamRow path="color_grade.perceptual_sat" label="Vibrance" labelWidth={48} />
    <ParamRow path="color_grade.global_chroma" label="Saturation" labelWidth={48} />
  </div>

  <div
    class="mt-[12px] rounded-[8px] border border-[rgba(103,103,103,0.05)] bg-[rgba(103,103,103,0.47)] p-[8px] backdrop-blur-[2px] flex flex-col items-center"
  >
    <div class="mb-[8px] flex w-full items-center justify-between">
      <span class="text-[6px] font-medium text-white">Color Mixer</span>
    </div>

    <div
      class="mb-[10px] flex h-[18px] w-[169px] items-center justify-between rounded-[22px] bg-[rgba(103,103,103,0.53)] px-[8px] py-[2px]"
    >
      {#each mixerColors as c (c.id)}
        <button
          aria-label={c.label}
          title={c.label}
          class="relative size-[9px] rounded-full transition-all duration-150 {activeColor === c.id
            ? 'scale-125'
            : 'opacity-80 hover:scale-110 hover:opacity-100'}"
          style="background: {c.hex};"
          onclick={() => (activeColor = c.id)}
        >
          {#if activeColor === c.id}
            <span class="absolute inset-0 scale-125 rounded-full border border-white/80"></span>
          {/if}
        </button>
      {/each}
    </div>

    <div class="flex w-full flex-col gap-[6px]">
      <ParamRow path={`hsl.${activeColor}.hue`} label="Hue" labelWidth={72} />
      <ParamRow path={`hsl.${activeColor}.sat`} label="Sat" labelWidth={72} />
      <ParamRow path={`hsl.${activeColor}.lum`} label="Lum" labelWidth={72} />
    </div>
  </div>
</CollapsibleSection>
