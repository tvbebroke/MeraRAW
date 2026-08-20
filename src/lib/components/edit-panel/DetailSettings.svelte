<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import Disclosure from "./Disclosure.svelte";
  import ToggleSwitch from "../primitives/ToggleSwitch.svelte";
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
  let aiDetail = $state("");
  let aiJob = $state<number | null>(null);
  let standIn = $state(false);

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
        aiStatus = "Denoising…";
        aiDetail = `${Math.round(p.pct)}% · tile ${p.tile}/${p.tiles}`;
      }),
      onDenoiseDone(() => {
        aiBusy = false;
        aiJob = null;
        aiStatus = "AI denoise ready";
        aiDetail = "";
      }),
      onDenoiseError((p) => {
        aiBusy = false;
        aiJob = null;
        aiStatus = `AI error: ${p.message}`;
        aiDetail = "";
      }),
    ];
    return () => un.forEach((u) => u.then((f) => f()));
  });

  async function toggleAi(on: boolean) {
    if (!on) {
      await resetAi();
      return;
    }
    await setParam("detail.ai_enabled", 1).then(reconcile).catch(() => {});
    await runAi();
  }

  async function runAi() {
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

  async function resetAi() {
    if (aiJob != null) void denoiseAiCancel(aiJob).catch(() => {});
    aiBusy = false;
    aiJob = null;
    await denoiseAiReset().catch(() => {});
    await setParam("detail.ai_enabled", 0).then(reconcile).catch(() => {});
    aiStatus = "";
    aiDetail = "";
  }
</script>

<CollapsibleSection id="detail" title="Detail">
  <ParamRow path="detail.enabled" label="Detail on" />
  <p class="group-label">Sharpening</p>
  <ParamRow path="detail.sharpen_amount" label="Amount" />
  <ParamRow path="detail.sharpen_radius" label="Radius" />
  <ParamRow path="detail.sharpen_detail" label="Detail" />

  <p class="group-label">Noise Reduction</p>
  <ParamRow path="detail.noise_luma" label="Luminance" />
  <ParamRow path="detail.noise_chroma" label="Color" />
  <Disclosure label="Advanced">
    <ParamRow path="detail.detail_preserve" label="Detail" />
    <ParamRow path="detail.nr_strength" label="Strength" />
    <ParamRow path="detail.impulse" label="Impulse" />
    <ParamRow path="detail.hot_pixels" label="Hot Pixels" />
  </Disclosure>

  <div class="control-row">
    <span>AI Denoise</span>
    <ToggleSwitch
      checked={aiOn}
      label="AI Denoise"
      onchange={(v) => void toggleAi(v)}
    />
  </div>
  {#if iso != null && iso >= 12800}
    <p class="rail-empty" style="color: var(--color-warn)">ISO {iso} — AI Denoise recommended</p>
  {/if}
  {#if standIn}
    <p class="rail-empty">Using stand-in model weights</p>
  {/if}
  <ParamRow path="detail.ai_amount" label="Amount" disabled={!aiOn} />
  <div class="actions">
    <button type="button" class="rail-btn" disabled={aiBusy} onclick={() => void runAi()}>
      {aiOn ? "Re-run" : "Run"}
    </button>
    <button type="button" class="rail-btn" onclick={() => void resetAi()}>Reset</button>
  </div>
  {#if aiStatus}
    <p class="rail-empty">
      {aiStatus}{#if aiDetail}&nbsp;<span class="num">{aiDetail}</span>{/if}
    </p>
  {/if}
</CollapsibleSection>

<style>
  .actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }
</style>
