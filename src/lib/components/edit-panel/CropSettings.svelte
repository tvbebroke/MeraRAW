<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import { applyCropParams, resetCropModule } from "../../../crop/cropActions";
  import { readCropFromDoc, type CropParams } from "../../../crop/cropMath";
  import { doc, reconcile } from "../../../stores/doc";
  import { autoLevel } from "../../../ipc/commands";

  const crop = $derived(readCropFromDoc($doc?.modules));

  const aspectRatios: { id: string; label: string; w: number; h: number }[] = [
    { id: "free", label: "Free", w: 0, h: 0 },
    { id: "original", label: "Original", w: 0, h: 0 },
    { id: "1:1", label: "1:1", w: 1, h: 1 },
    { id: "16:9", label: "16:9", w: 16, h: 9 },
    { id: "4:3", label: "4:3", w: 4, h: 3 },
    { id: "5:7", label: "5:7", w: 5, h: 7 },
  ];

  let activeRatio = $state("original");

  async function patch(partial: Partial<CropParams>, live = false) {
    try {
      reconcile(await applyCropParams({ ...crop, ...partial }, live));
    } catch {
      /* ignore */
    }
  }

  function setRatio(id: string) {
    activeRatio = id;
    const r = aspectRatios.find((a) => a.id === id);
    if (!r) return;
    if (id === "free") {
      void patch({ aspectLocked: false });
    } else if (id === "original") {
      void patch({ aspectLocked: true, aspectW: 0, aspectH: 0 });
    } else {
      void patch({ aspectLocked: true, aspectW: r.w, aspectH: r.h });
    }
  }

  async function onAutoLevel() {
    try {
      const deg = await autoLevel();
      if (deg) void patch({ angle: crop.angle + deg });
    } catch {
      /* ignore */
    }
  }

  async function reset() {
    try {
      reconcile(await resetCropModule());
      activeRatio = "original";
    } catch {
      /* ignore */
    }
  }
</script>

<CollapsibleSection id="crop" title="Crop">
  <p class="group-label">Aspect</p>
  <div class="grid3">
    {#each aspectRatios as ratio (ratio.id)}
      <button
        type="button"
        class="rail-chip"
        class:is-active={activeRatio === ratio.id}
        onclick={() => setRatio(ratio.id)}
      >
        <span class={/\d/.test(ratio.label) ? "num" : ""}>{ratio.label}</span>
      </button>
    {/each}
  </div>

  <p class="group-label">Transform</p>
  <ParamRow path="crop.angle" label="Rotate" />
  <div class="grid2">
    <button type="button" class="rail-btn" onclick={() => void patch({ flipH: !crop.flipH })}>
      Flip Horizontal
    </button>
    <button type="button" class="rail-btn" onclick={() => void patch({ flipV: !crop.flipV })}>
      Flip Vertical
    </button>
    <button type="button" class="rail-btn" onclick={() => void patch({ rotate90: (crop.rotate90 + 1) % 4 })}>
      Rotate 90°
    </button>
    <button type="button" class="rail-btn" onclick={() => void onAutoLevel()}>
      Auto Level
    </button>
  </div>
  <button type="button" class="rail-btn full" onclick={() => void reset()}>Reset Crop</button>

  <p class="group-label">Perspective</p>
  <ParamRow path="crop.persp_vertical" label="Vertical" />
  <ParamRow path="crop.persp_horizontal" label="Horizontal" />
  <button
    type="button"
    class="rail-btn full"
    onclick={() => void patch({ perspVertical: 0, perspHorizontal: 0 })}
  >
    Reset Perspective
  </button>
</CollapsibleSection>

<style>
  .grid3 { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: var(--space-2); }
  .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-2); }
  .full { width: 100%; margin-top: var(--space-2); }
</style>
