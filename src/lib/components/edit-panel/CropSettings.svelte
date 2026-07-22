<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import Slider from "./Slider.svelte";
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

<CollapsibleSection id="cropAspect" title="Aspect Ratio">
  <div class="grid grid-cols-3 gap-[8px]">
    {#each aspectRatios as ratio (ratio.id)}
      <button
        onclick={() => setRatio(ratio.id)}
        class="h-[26px] rounded-[13px] border border-white/5 text-[10px] font-medium transition-all {activeRatio ===
        ratio.id
          ? 'bg-white/10 text-white'
          : 'bg-white/[0.02] text-white/60 hover:bg-white/5'}"
      >
        {ratio.label}
      </button>
    {/each}
  </div>
</CollapsibleSection>

<div class="h-2"></div>

<CollapsibleSection id="cropTransform" title="Transform">
  <div class="flex flex-col gap-[12px] pt-1">
    <div class="grid h-[15px] grid-cols-[48px_1fr] items-center gap-x-[10px]">
      <span class="text-[9px] text-white/70">Rotate</span>
      <Slider
        label="Rotate"
        min={-45}
        max={45}
        step={0.1}
        value={crop.angle}
        oninput={(v) => void patch({ angle: v }, true)}
        onchange={(v) => void patch({ angle: v }, false)}
      />
    </div>

    <div class="mt-1 grid grid-cols-2 gap-[8px]">
      <button
        onclick={() => void patch({ flipH: !crop.flipH })}
        class="h-[28px] rounded-[14px] border border-white/5 bg-white/[0.02] text-[10px] font-medium text-white/80 transition-all hover:bg-white/5"
      >
        Flip Horizontal
      </button>
      <button
        onclick={() => void patch({ flipV: !crop.flipV })}
        class="h-[28px] rounded-[14px] border border-white/5 bg-white/[0.02] text-[10px] font-medium text-white/80 transition-all hover:bg-white/5"
      >
        Flip Vertical
      </button>
      <button
        onclick={() => void patch({ rotate90: (crop.rotate90 + 1) % 4 })}
        class="h-[28px] rounded-[14px] border border-white/5 bg-white/[0.02] text-[10px] font-medium text-white/80 transition-all hover:bg-white/5"
      >
        Rotate 90°
      </button>
      <button
        onclick={() => void onAutoLevel()}
        class="h-[28px] rounded-[14px] border border-white/5 bg-white/[0.02] text-[10px] font-medium text-white/80 transition-all hover:bg-white/5"
      >
        Auto Level
      </button>
    </div>

    <button
      onclick={() => void reset()}
      class="h-[28px] rounded-[14px] border border-white/5 bg-white/[0.02] text-[10px] font-medium text-white/55 transition-all hover:bg-white/5"
    >
      Reset Crop
    </button>
  </div>
</CollapsibleSection>

<div class="h-2"></div>

<CollapsibleSection id="cropPerspective" title="Perspective">
  <div class="flex flex-col gap-[12px] pt-1">
    <div class="grid h-[15px] grid-cols-[64px_1fr] items-center gap-x-[10px]">
      <span class="text-[9px] text-white/70">Vertical</span>
      <Slider
        label="Vertical"
        min={-100}
        max={100}
        step={1}
        value={crop.perspVertical}
        resetValue={0}
        oninput={(v) => void patch({ perspVertical: v }, true)}
        onchange={(v) => void patch({ perspVertical: v }, false)}
      />
    </div>
    <div class="grid h-[15px] grid-cols-[64px_1fr] items-center gap-x-[10px]">
      <span class="text-[9px] text-white/70">Horizontal</span>
      <Slider
        label="Horizontal"
        min={-100}
        max={100}
        step={1}
        value={crop.perspHorizontal}
        resetValue={0}
        oninput={(v) => void patch({ perspHorizontal: v }, true)}
        onchange={(v) => void patch({ perspHorizontal: v }, false)}
      />
    </div>
    <button
      onclick={() => void patch({ perspVertical: 0, perspHorizontal: 0 })}
      class="h-[28px] rounded-[14px] border border-white/5 bg-white/[0.02] text-[10px] font-medium text-white/55 transition-all hover:bg-white/5"
    >
      Reset Perspective
    </button>
  </div>
</CollapsibleSection>
