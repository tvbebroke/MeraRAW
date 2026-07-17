<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import {
    denoiseAiReset,
    denoiseAiStart,
    setParam,
  } from "../../../ipc/commands";
  import { reconcile, docParam, doc } from "../../../stores/doc";
  import { imageMeta } from "../../../stores/app";

  const aiOn = $derived((docParam($doc, "detail", "ai_enabled") ?? 0) >= 0.5);
  const iso = $derived($imageMeta?.iso ?? null);
  let status = $state("");

  async function runAi() {
    status = "Starting…";
    try {
      await setParam("detail.ai_enabled", 1).then(reconcile);
      await denoiseAiStart();
      status = "AI denoise running…";
    } catch (e) {
      status = String(e);
    }
  }

  async function resetAi() {
    try {
      await denoiseAiReset();
      await setParam("detail.ai_enabled", 0).then(reconcile);
      status = "Reset";
    } catch (e) {
      status = String(e);
    }
  }
</script>

<CollapsibleSection id="aiSelect" title="AI Adjustments">
  <div class="flex flex-col gap-[12px]">
    <p class="px-1 text-[9px] text-white/45">
      AI Denoise rewrites the working master. Amount lives under Detail too.
    </p>
    {#if iso != null && iso >= 12800}
      <p class="px-1 text-[8px] text-amber-200/70">ISO {iso} — AI Denoise recommended</p>
    {/if}

    <ParamRow path="detail.ai_amount" label="AI Amount" />

    <div class="grid grid-cols-2 gap-[6px]">
      <button
        onclick={() => void runAi()}
        class="h-[30px] rounded-[10px] border border-white/5 bg-white/[0.02] text-[9px] font-medium text-white/80 transition-all hover:bg-white/5"
      >
        {aiOn ? "Re-run AI" : "Run AI Denoise"}
      </button>
      <button
        onclick={() => void resetAi()}
        class="h-[30px] rounded-[10px] border border-white/5 bg-white/[0.02] text-[9px] font-medium text-white/80 transition-all hover:bg-white/5"
      >
        Reset
      </button>
    </div>

    {#if status}
      <p class="px-1 text-[8px] text-white/40">{status}</p>
    {/if}
  </div>
</CollapsibleSection>
