<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import Disclosure from "./Disclosure.svelte";
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
  <div class="temp-row">
    <div class="temp-slider">
      <ParamRow path="white_balance.temp" label="Temp" />
    </div>
    <button
      type="button"
      title="White balance eyedropper"
      aria-label="White balance eyedropper"
      class="eyedrop"
      class:is-active={$viewportTool === "wb"}
      onclick={toggleWb}
    >
      ⌖
    </button>
  </div>
  <ParamRow path="white_balance.tint" label="Tint" />
  <ParamRow path="color_grade.perceptual_sat" label="Vibrance" />
  <ParamRow path="color_grade.global_chroma" label="Saturation" />
  <Disclosure label="Color Mixer">
    <div class="swatches">
      {#each mixerColors as c (c.id)}
        <button
          type="button"
          aria-label={c.label}
          title={c.label}
          class="swatch"
          class:on={activeColor === c.id}
          style="background: {c.hex}"
          onclick={() => (activeColor = c.id)}
        ></button>
      {/each}
    </div>
    <ParamRow path={`hsl.${activeColor}.hue`} label="Hue" />
    <ParamRow path={`hsl.${activeColor}.sat`} label="Sat" />
    <ParamRow path={`hsl.${activeColor}.lum`} label="Lum" />
  </Disclosure>
</CollapsibleSection>

<style>
  .temp-row {
    display: flex;
    align-items: flex-end;
    gap: var(--space-2);
  }
  .temp-slider { flex: 1; min-width: 0; }
  .eyedrop {
    flex: none;
    width: 28px;
    height: 28px;
    margin-bottom: 6px;
    border-radius: 6px;
    border: 1px solid var(--color-border);
    background: var(--color-hover);
    color: var(--color-subtle);
    font-size: var(--text-ui);
    cursor: pointer;
  }
  .eyedrop:hover, .eyedrop.is-active {
    background: var(--color-active);
    color: var(--color-fg);
    border-color: var(--color-fg);
  }
  .swatches {
    display: flex;
    justify-content: space-between;
    max-width: 180px;
    margin: var(--space-2) auto;
    padding: 0 var(--space-1);
  }
  .swatch {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    border: 0;
    padding: 0;
    cursor: pointer;
    opacity: 0.8;
  }
  .swatch.on {
    opacity: 1;
    outline: 1px solid var(--color-fg);
    outline-offset: 2px;
  }
</style>
