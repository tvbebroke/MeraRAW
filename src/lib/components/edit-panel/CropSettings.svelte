<script lang="ts">
  import ParamRow from "./ParamRow.svelte";
  import { applyCropParams, resetCropModule } from "../../../crop/cropActions";
  import {
    applyAspectPreset,
    cropWithDraft,
    flipCropOrientation,
    type CropParams,
  } from "../../../crop/cropMath";
  import { cropDraft, setCropDraft } from "../../../crop/cropSession";
  import { doc, reconcile } from "../../../stores/doc";
  import { imageDims } from "../../../stores/app";
  import { autoLevel } from "../../../ipc/commands";
  import { leaveCropTool } from "../../editor/focus";

  const crop = $derived(cropWithDraft($doc?.modules, $cropDraft));
  const dims = $derived($imageDims);

  const labeledRatios = [
    { id: "free", label: "Free", w: 0, h: 0 },
    { id: "original", label: "Original", w: 0, h: 0 },
    { id: "1:1", label: "1:1", w: 1, h: 1 },
    { id: "2:3", label: "2:3", w: 2, h: 3 },
    { id: "4:5", label: "4:5", w: 4, h: 5 },
    { id: "4:3", label: "4:3", w: 4, h: 3 },
    { id: "5:7", label: "5:7", w: 5, h: 7 },
    { id: "16:9", label: "16:9", w: 16, h: 9 },
  ];

  const activeRatio = $derived.by(() => {
    if (!crop.aspectLocked) return "free";
    if (crop.aspectW <= 0 || crop.aspectH <= 0) return "original";
    const id = `${crop.aspectW}:${crop.aspectH}`;
    if (labeledRatios.some((a) => a.id === id)) return id;
    const flip = `${crop.aspectH}:${crop.aspectW}`;
    if (labeledRatios.some((a) => a.id === flip)) return flip;
    return id;
  });

  async function patch(next: CropParams, live = false) {
    try {
      reconcile(await applyCropParams(next, live));
      setCropDraft(null);
    } catch {
      /* ignore */
    }
  }

  function setRatio(preset: (typeof labeledRatios)[number]) {
    if (!dims) {
      void patch({
        ...crop,
        aspectLocked: preset.id !== "free",
        aspectW: preset.id === "original" ? 0 : preset.w,
        aspectH: preset.id === "original" ? 0 : preset.h,
      });
      return;
    }
    void patch(applyAspectPreset(crop, preset, dims.w, dims.h));
  }

  function flipAspect() {
    if (!dims) return;
    void patch(flipCropOrientation(crop, dims.w, dims.h));
  }

  async function onAutoLevel() {
    try {
      const deg = await autoLevel();
      if (deg) void patch({ ...crop, angle: crop.angle + deg });
    } catch {
      /* ignore */
    }
  }

  async function reset() {
    try {
      setCropDraft(null);
      reconcile(await resetCropModule());
    } catch {
      /* ignore */
    }
  }
</script>

<div class="crop-pane custom-scrollbar">
  <p class="group-label">Aspect</p>
  <div class="grid3">
    {#each labeledRatios as ratio (ratio.id)}
      <button
        type="button"
        class="rail-chip"
        class:is-active={activeRatio === ratio.id}
        onclick={() => setRatio(ratio)}
      >
        <span class={/\d/.test(ratio.label) ? "num" : ""}>{ratio.label}</span>
      </button>
    {/each}
  </div>
  <button type="button" class="rail-btn full" onclick={flipAspect} title="Flip aspect (X)">
    Flip aspect
  </button>

  <p class="group-label">Transform</p>
  <ParamRow path="crop.angle" label="Straighten" />
  <div class="grid2">
    <button type="button" class="rail-btn" onclick={() => void patch({ ...crop, flipH: !crop.flipH })}>
      Flip Horizontal
    </button>
    <button type="button" class="rail-btn" onclick={() => void patch({ ...crop, flipV: !crop.flipV })}>
      Flip Vertical
    </button>
    <button type="button" class="rail-btn" onclick={() => void patch({ ...crop, rotate90: (crop.rotate90 + 1) % 4 })}>
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
    onclick={() => void patch({ ...crop, perspVertical: 0, perspHorizontal: 0 })}
  >
    Reset Perspective
  </button>

  <button type="button" class="rail-btn done" onclick={() => leaveCropTool()}>
    Done
  </button>
</div>

<style>
  .crop-pane {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: var(--space-3);
    padding-bottom: var(--space-4);
  }
  .grid3 { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: var(--space-2); }
  .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-2); }
  .full { width: 100%; margin-top: var(--space-2); }
  .done { width: 100%; margin-top: var(--space-4); }
</style>
