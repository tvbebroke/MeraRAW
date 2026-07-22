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
  import { classicLook } from "../../../stores/ui";

  const aiOn = $derived((docParam($doc, "detail", "ai_enabled") ?? 0) >= 0.5);
  const iso = $derived($imageMeta?.iso ?? null);
  let status = $state("");
  let promptText = $state("");

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

  function submitPrompt() {
    if (!promptText.trim()) return;
    console.log("AI prompt submitted via panel:", promptText);
    promptText = "";
  }
</script>

<CollapsibleSection id="aiSelect" title="AI Adjustments">
  <div class="flex flex-col gap-[12px]">
    {#if $classicLook}
      <!-- Classic Theme AI Prompt Box -->
      <div class="flex flex-col gap-[6px] mb-2 px-1">
        <label for="ai-panel-prompt" class="text-[9px] font-bold text-white/50 tracking-wider">AI edit prompt</label>
        <div class="flex gap-2">
          <input
            id="ai-panel-prompt"
            type="text"
            bind:value={promptText}
            placeholder="Describe an edit..."
            class="flex-1 h-[28px] border border-white/10 bg-white/[0.02] px-2 text-[10px] text-white outline-none focus:border-white/20"
            onkeydown={(e) => e.key === "Enter" && submitPrompt()}
          />
          <button
            onclick={submitPrompt}
            disabled={!promptText.trim()}
            class="h-[28px] px-3 bg-white text-black font-semibold text-[10px] flex items-center justify-center transition-all disabled:opacity-30 enabled:hover:bg-white/90 cursor-pointer"
          >
            Apply
          </button>
        </div>
      </div>
      <div class="h-[1px] bg-white/5 my-1"></div>
    {/if}

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

    <div class="h-[1px] bg-white/5 my-2"></div>

    <div class="text-[10px] font-bold text-white/50 mb-1 px-1">Auto masking</div>
    <div class="grid grid-cols-3 gap-[6px]">
      <button class="h-[30px] rounded-[10px] text-[9px] font-medium border border-white/5 bg-white/[0.02] text-white/80 hover:bg-white/5 active:scale-98 transition-all">
        Subject
      </button>
      <button class="h-[30px] rounded-[10px] text-[9px] font-medium border border-white/5 bg-white/[0.02] text-white/80 hover:bg-white/5 active:scale-98 transition-all">
        Sky
      </button>
      <button class="h-[30px] rounded-[10px] text-[9px] font-medium border border-white/5 bg-white/[0.02] text-white/80 hover:bg-white/5 active:scale-98 transition-all">
        Background
      </button>
    </div>
  </div>
</CollapsibleSection>
