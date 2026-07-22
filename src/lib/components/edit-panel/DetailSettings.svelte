<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import Slider from "./Slider.svelte";
  import {
    denoiseAiCancel,
    denoiseAiReset,
    denoiseAiStart,
    denoiseModelsList,
    setParam,
  } from "../../../ipc/commands";
  import { onDenoiseDone, onDenoiseError, onDenoiseProgress } from "../../../ipc/events";
  import { reconcile, docParam, doc } from "../../../stores/doc";
  import { imageMeta } from "../../../stores/app";
  import { onMount } from "svelte";

  let aiBusy = $state(false);
  let aiStatus = $state("");
  let aiJob = $state<number | null>(null);
  let standIn = $state(false);

  // Upstream (meraraw-ui-svelte) adds a "Masking" slider under Sharpening.
  // No corresponding engine param exists (checked src-tauri/core/src/registry.rs
  // — only sharpen_amount/radius/detail are registered), so this is kept as a
  // cosmetic, unwired placeholder like upstream's own mockup implementation.
  let sharpenMasking = $state(0);

  const aiOn = $derived((docParam($doc, "detail", "ai_enabled") ?? 0) >= 0.5);
  const iso = $derived($imageMeta?.iso ?? null);

  onMount(() => {
    void denoiseModelsList()
      .then((models) => {
        standIn = models.some((m) => m.standIn);
      })
      .catch(() => {});

    const un = [
      onDenoiseProgress((p) => {
        aiStatus = `Denoising… ${Math.round(p.pct)}% (tile ${p.tile}/${p.tiles})`;
      }),
      onDenoiseDone(() => {
        aiBusy = false;
        aiJob = null;
        aiStatus = "AI denoise ready";
      }),
      onDenoiseError((p) => {
        aiBusy = false;
        aiJob = null;
        aiStatus = `AI error: ${p.message}`;
      }),
    ];
    return () => un.forEach((u) => u.then((f) => f()));
  });

  async function toggleAi(on: boolean) {
    if (!on) {
      if (aiJob != null) void denoiseAiCancel(aiJob).catch(() => {});
      aiBusy = false;
      aiJob = null;
      await denoiseAiReset().catch(() => {});
      await setParam("detail.ai_enabled", 0).then(reconcile).catch(() => {});
      aiStatus = "";
      return;
    }
    await setParam("detail.ai_enabled", 1).then(reconcile).catch(() => {});
    aiBusy = true;
    aiStatus = "Starting AI denoise…";
    try {
      aiJob = await denoiseAiStart();
    } catch (e) {
      aiBusy = false;
      aiStatus = `AI failed: ${e}`;
    }
  }
</script>

<CollapsibleSection id="detail" title="Detail">
  <div class="flex flex-col gap-[10px]">
    <div>
      <div class="mb-2 text-[11px] font-bold tracking-wider text-white/60">
        Sharpening
      </div>
      <div class="flex flex-col gap-[6px]">
        <ParamRow path="detail.sharpen_amount" label="Amount" />
        <ParamRow path="detail.sharpen_radius" label="Radius" />
        <ParamRow path="detail.sharpen_detail" label="Detail" />
        <div class="grid h-[16px] grid-cols-[72px_1fr] items-center gap-x-[10px]">
          <span class="text-[11px] font-medium text-white/90">Masking</span>
          <Slider
            label="Sharpening Masking"
            min={0}
            max={100}
            value={sharpenMasking}
            onchange={(v) => (sharpenMasking = v)}
          />
        </div>
      </div>
    </div>

    <div class="my-1 h-[1px] bg-white/5"></div>

    <div>
      <div class="mb-2 text-[11px] font-bold tracking-wider text-white/60">
        Noise Reduction
      </div>
      <div class="flex flex-col gap-[6px]">
        <ParamRow path="detail.noise_luma" label="Luminance" />
        <ParamRow path="detail.detail_preserve" label="Detail" />
        <ParamRow path="detail.noise_chroma" label="Color" />
        <ParamRow path="detail.nr_strength" label="Strength" />
        <ParamRow path="detail.impulse" label="Impulse" />
        <ParamRow path="detail.hot_pixels" label="Hot Pixels" />
      </div>
    </div>

    <div class="my-1 h-[1px] bg-white/5"></div>

    <div>
      <div class="mb-2 flex items-center justify-between">
        <div class="text-[11px] font-bold tracking-wider text-white/60">
          AI Denoise
        </div>
        <label class="flex items-center gap-2 text-[11px] text-white/70">
          <input
            type="checkbox"
            checked={aiOn}
            disabled={aiBusy}
            onchange={(e) => void toggleAi((e.currentTarget as HTMLInputElement).checked)}
          />
          Enabled
        </label>
      </div>
      {#if iso != null && iso >= 12800}
        <p class="mb-2 text-[8px] text-amber-200/70">ISO {iso} — AI Denoise recommended</p>
      {/if}
      {#if standIn}
        <p class="mb-2 text-[8px] text-white/35">Using stand-in model weights</p>
      {/if}
      <div class="flex flex-col gap-[6px]">
        <ParamRow path="detail.ai_amount" label="Amount" disabled={!aiOn} />
      </div>
      {#if aiStatus}
        <p class="mt-2 text-[8px] text-white/45">{aiStatus}</p>
      {/if}
    </div>
  </div>
</CollapsibleSection>
