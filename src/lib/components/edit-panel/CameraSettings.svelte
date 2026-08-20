<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import Disclosure from "./Disclosure.svelte";
  import ProfileSettings from "./ProfileSettings.svelte";
  import DemosaicSettings from "./DemosaicSettings.svelte";
  import CalibrationSettings from "./CalibrationSettings.svelte";
  import { setParam, seekVideo } from "../../../ipc/commands";
  import { doc, docParam, reconcile } from "../../../stores/doc";
  import { imageMeta, statusMessage } from "../../../stores/app";

  let { mode = "photo" }: { mode?: "photo" | "video" } = $props();

  const transfers: { v: number; l: string }[] = [
    { v: 0, l: "Auto" },
    { v: 2, l: "Rec.709" },
    { v: 1, l: "sRGB" },
    { v: 7, l: "S-Log3" },
    { v: 5, l: "LogC3" },
    { v: 6, l: "LogC4" },
    { v: 8, l: "V-Log" },
    { v: 9, l: "PQ" },
    { v: 10, l: "HLG" },
  ];
  const gamuts: { v: number; l: string }[] = [
    { v: 0, l: "Auto" },
    { v: 1, l: "Rec.709" },
    { v: 2, l: "Rec.2020" },
    { v: 3, l: "P3" },
    { v: 4, l: "S-Gamut3.Cine" },
    { v: 5, l: "ARRI WG3" },
    { v: 6, l: "ARRI WG4" },
    { v: 7, l: "V-Gamut" },
  ];

  const tVal = $derived(docParam($doc, "input", "transfer") ?? 0);
  const pVal = $derived(docParam($doc, "input", "primaries") ?? 0);
  const assumed = $derived($imageMeta?.video?.inputTransform ?? $imageMeta?.inputColorSpace ?? null);
  const isVideo = $derived($imageMeta?.kind === "video");

  async function applyInput() {
    const frame = $imageMeta?.video?.frame ?? 0;
    if ($imageMeta?.kind === "video") {
      try {
        const meta = await seekVideo(frame);
        imageMeta.set(meta);
      } catch (e) {
        statusMessage.set(e instanceof Error ? e.message : String(e));
      }
    }
  }

  function setT(v: number) {
    void setParam("input.transfer", v)
      .then(reconcile)
      .then(() => applyInput());
  }
  function setP(v: number) {
    void setParam("input.primaries", v)
      .then(reconcile)
      .then(() => applyInput());
  }
</script>

<CollapsibleSection id="camera" title={mode === "video" ? "Camera Log" : "Camera"}>
  {#if mode === "photo"}
    <ProfileSettings />
    <DemosaicSettings />
  {/if}
  {#if mode === "video" || isVideo || assumed}
    <p class="group-label">{mode === "video" ? "Input transform" : "Camera Log"}</p>
    {#if tVal === 0}
      <p class="hint">Assumed input: {assumed ?? "Rec.709 display-referred"}</p>
    {/if}
    <div class="chips">
      {#each transfers as t (t.v)}
        <button type="button" class="rail-chip" class:is-active={tVal === t.v} onclick={() => setT(t.v)}>
          {t.l}
        </button>
      {/each}
    </div>
    <p class="mini">Gamut</p>
    <div class="chips">
      {#each gamuts as g (g.v)}
        <button type="button" class="rail-chip" class:is-active={pVal === g.v} onclick={() => setP(g.v)}>
          {g.l}
        </button>
      {/each}
    </div>
  {/if}
  {#if mode === "photo"}
    <Disclosure label="Calibration">
      <CalibrationSettings />
    </Disclosure>
  {/if}
</CollapsibleSection>

<style>
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
    margin: 4px 0 8px;
  }
  .hint,
  .mini {
    font-size: 10px;
    color: var(--color-muted, #888);
    margin: 0 0 4px;
  }
</style>
