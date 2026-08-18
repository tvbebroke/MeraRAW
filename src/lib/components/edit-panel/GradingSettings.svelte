<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import { setParam } from "../../../ipc/commands";
  import { reconcile, docParam, doc } from "../../../stores/doc";

  type Mode = "perceptual" | "classic" | "light";
  const modes: { id: Mode; label: string; value: number }[] = [
    { id: "perceptual", label: "Perceptual", value: 0 },
    { id: "classic", label: "Classic", value: 1 },
    { id: "light", label: "Light", value: 2 },
  ];

  type Zone = "shadows" | "midtones" | "highlights";
  const zones: {
    id: Zone;
    label: string;
    hue: string;
    sat: string;
    lum: string;
    range: string | null;
  }[] = [
    {
      id: "shadows",
      label: "Shadows",
      hue: "color_grade.shadows_hue",
      sat: "color_grade.shadows_sat",
      lum: "color_grade.shadows_lum",
      range: "color_grade.shadow_range",
    },
    {
      id: "midtones",
      label: "Midtones",
      hue: "color_grade.midtones_hue",
      sat: "color_grade.midtones_sat",
      lum: "color_grade.midtones_lum",
      range: null,
    },
    {
      id: "highlights",
      label: "Highlights",
      hue: "color_grade.highlights_hue",
      sat: "color_grade.highlights_sat",
      lum: "color_grade.highlights_lum",
      range: "color_grade.highlight_range",
    },
  ];

  let zone = $state<Zone>("shadows");
  const active = $derived(zones.find((z) => z.id === zone) ?? zones[0]);

  const model = $derived(docParam($doc, "color_grade", "model") ?? 0);
  const activeMode = $derived(modes.find((m) => m.value === model)?.id ?? "perceptual");

  function setMode(m: Mode) {
    const v = modes.find((x) => x.id === m)?.value ?? 0;
    void setParam("color_grade.model", v).then(reconcile).catch(() => {});
  }
</script>

<CollapsibleSection id="grading" title="Grading">
  <div class="pill">
    {#each modes as m (m.id)}
      <button
        type="button"
        class="rail-chip"
        class:is-active={activeMode === m.id}
        onclick={() => setMode(m.id)}
      >
        {m.label}
      </button>
    {/each}
  </div>

  <div class="pill" style="margin-top: var(--space-2)">
    {#each zones as z (z.id)}
      <button
        type="button"
        class="rail-chip"
        class:is-active={zone === z.id}
        onclick={() => (zone = z.id)}
      >
        {z.label}
      </button>
    {/each}
  </div>

  <ParamRow path={active.hue} label="Hue" />
  <ParamRow path={active.sat} label="Sat" />
  <ParamRow path={active.lum} label="Lum" />
  {#if active.range}
    <ParamRow path={active.range} label="Range" />
  {/if}
</CollapsibleSection>

<style>
  .pill {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 2px;
    padding: 3px;
    border-radius: 99px;
    background: var(--color-sunken);
    border: 1px solid var(--color-border-strong);
  }
  .pill :global(.rail-chip) {
    border: 0;
    border-radius: 99px;
    background: transparent;
  }
  .pill :global(.rail-chip.is-active) {
    background: var(--color-active);
  }
</style>
