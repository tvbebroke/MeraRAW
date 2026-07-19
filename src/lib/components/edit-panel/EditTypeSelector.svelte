<script lang="ts">
  import { activeTool, type Tool } from "../../../stores/editor";
  import { classicLook } from "../../../stores/ui";
  import { cropActive, viewportTool, selectedMask } from "../../../stores/app";
  import { setMaskOverlay } from "../../../ipc/commands";
  import GlassPanel from "../primitives/GlassPanel.svelte";
  import editIcon from "../../icons/tool-edit.svg";
  import cropIcon from "../../icons/tool-crop.svg";
  import maskIcon from "../../icons/tool-mask.svg";
  import aiIcon from "../../icons/tool-ai.svg";
  import presetsIcon from "../../icons/tool-presets.svg";

  const tools: { id: Tool; icon: string; label: string; iconClass: string }[] =
    [
      { id: "edit", icon: editIcon, label: "Edit", iconClass: "h-[21px] w-[24px]" },
      { id: "crop", icon: cropIcon, label: "Crop", iconClass: "size-[24px]" },
      { id: "mask", icon: maskIcon, label: "Mask", iconClass: "size-[24px]" },
      { id: "ai", icon: aiIcon, label: "AI", iconClass: "size-[24px]" },
      { id: "presets", icon: presetsIcon, label: "Presets", iconClass: "size-[24px]" },
    ];

  const activeIndex = $derived(tools.findIndex(t => t.id === $activeTool));

  function selectTool(id: Tool) {
    activeTool.set(id);
    cropActive.set(id === "crop");
    if (id === "crop") {
      viewportTool.set("crop");
      void setMaskOverlay(null);
    } else if (id === "mask") {
      viewportTool.set("brush");
      const mid = selectedMask.get();
      void setMaskOverlay(mid);
    } else {
      viewportTool.set("pan");
      void setMaskOverlay(null);
    }
  }
</script>

{#if $classicLook}
  <!-- Classic vertical tool strip docked on the right side of the screen -->
  <div class="flex flex-col w-[42px] shrink-0 bg-[#1e1e20] border-l border-[#121212] pt-[12px] items-center gap-[6px] z-10">
    {#each tools as t (t.id)}
      <button
        title={t.label}
        aria-label={t.label}
        aria-pressed={$activeTool === t.id}
        class="flex size-[30px] items-center justify-center rounded-full border transition-all duration-150 cursor-pointer
          {$activeTool === t.id
            ? 'border-[#555] bg-[#3a3a3c] text-white'
            : 'border-transparent bg-transparent hover:bg-white/[0.04] text-white/60'
          }"
        onclick={() => selectTool(t.id)}
      >
        <img src={t.icon} alt="" class="size-[16px] object-contain opacity-80" />
      </button>
    {/each}
  </div>
{:else}
  <GlassPanel
    variant="liquid"
    class="relative grid h-[95px] shrink-0 grid-cols-5 items-center gap-x-[6px] py-[16px] px-[12px]"
  >
    <!-- Sliding active background pill -->
    <div class="absolute inset-y-[16px] left-[12px] right-[12px] grid grid-cols-5 gap-x-[6px] pointer-events-none z-0">
      <div
        class="h-[63px] w-full rounded-[12px] border border-white/12 bg-[#2a2a2d]/85 shadow-[inset_0_1px_0_rgba(255,255,255,0.08),0_4px_12px_rgba(0,0,0,0.4)] transition-all duration-300 ease-[cubic-bezier(0.25,1,0.5,1)]"
        style="transform: translateX(calc({activeIndex} * 100% + {activeIndex} * 6px));"
      ></div>
    </div>

    {#each tools as t (t.id)}
      <button
        title={t.label}
        aria-label={t.label}
        aria-pressed={$activeTool === t.id}
        class="flex h-[63px] w-full items-center justify-center rounded-[12px] border transition-all duration-200 active:scale-95 z-10
          {$activeTool === t.id
            ? 'border-transparent bg-transparent'
            : 'border-white/[0.04] bg-[#2b2b2b]/15 hover:bg-[#2b2b2b]/35 cursor-pointer'
          }"
        onclick={() => selectTool(t.id)}
      >
        <img src={t.icon} alt="" class={t.iconClass} />
      </button>
    {/each}
  </GlassPanel>
{/if}

