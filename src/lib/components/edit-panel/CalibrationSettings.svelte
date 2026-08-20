<script lang="ts">
  import ParamRow from "./ParamRow.svelte";

  const bands = [
    { id: "red", label: "R", hue: "calibration.red_hue", sat: "calibration.red_sat", hex: "#e64f4f" },
    { id: "green", label: "G", hue: "calibration.green_hue", sat: "calibration.green_sat", hex: "#0fb327" },
    { id: "blue", label: "B", hue: "calibration.blue_hue", sat: "calibration.blue_sat", hex: "#3b82f6" },
  ] as const;

  let band = $state<(typeof bands)[number]["id"]>("red");
  const active = $derived(bands.find((b) => b.id === band) ?? bands[0]);
</script>

<div class="swatches">
  {#each bands as b (b.id)}
    <button
      type="button"
      class="swatch"
      class:on={band === b.id}
      style="background: {b.hex}"
      aria-label={b.label}
      onclick={() => (band = b.id)}
    ></button>
  {/each}
</div>
<ParamRow path="calibration.enabled" label="Calibration on" />
<ParamRow path={active.hue} label="Hue" />
<ParamRow path={active.sat} label="Sat" />
<ParamRow path="calibration.shadow_tint" label="Shadow Tint" />

<style>
  .swatches {
    display: flex;
    justify-content: center;
    gap: var(--space-3);
    min-height: 28px;
    align-items: center;
    margin-bottom: var(--space-1);
  }
  .swatch {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    border: 0;
    padding: 0;
    cursor: pointer;
    opacity: 0.7;
  }
  .swatch.on {
    opacity: 1;
    outline: 1px solid var(--color-fg);
    outline-offset: 2px;
  }
</style>
