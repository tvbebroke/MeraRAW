<script lang="ts">
  import FileBrowser from "../lib/components/file-browser/FileBrowser.svelte";
  import MainViewport from "../lib/components/viewport/MainViewport.svelte";
  import EditTypeSelector from "../lib/components/edit-panel/EditTypeSelector.svelte";
  import ProfileSettings from "../lib/components/edit-panel/ProfileSettings.svelte";
  import LightSettings from "../lib/components/edit-panel/LightSettings.svelte";
  import ColorSettings from "../lib/components/edit-panel/ColorSettings.svelte";
  import DetailSettings from "../lib/components/edit-panel/DetailSettings.svelte";
  import EffectsSettings from "../lib/components/edit-panel/EffectsSettings.svelte";
  import OpticsSettings from "../lib/components/edit-panel/OpticsSettings.svelte";
  import DemosaicSettings from "../lib/components/edit-panel/DemosaicSettings.svelte";
  import LutSettings from "../lib/components/edit-panel/LutSettings.svelte";
  import CropSettings from "../lib/components/edit-panel/CropSettings.svelte";
  import MaskSettings from "../lib/components/edit-panel/MaskSettings.svelte";
  import AiSettings from "../lib/components/edit-panel/AiSettings.svelte";
  import PresetSettings from "../lib/components/edit-panel/PresetSettings.svelte";
  import ImageBrowser from "../lib/components/image-browser/ImageBrowser.svelte";
  import Histogram from "../lib/components/histogram/Histogram.svelte";
  import BottomBar from "../lib/components/shell/BottomBar.svelte";
  import GlassPanel from "../lib/components/primitives/GlassPanel.svelte";
  import { activeTool, leftRailCollapsed, isZenMode, imageBrowserCollapsed } from "../stores/editor";
  import { fade, fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import aiIcon from "../lib/icons/tool-ai.svg";

  const isZen = $derived($isZenMode);

  let promptText = $state("");

  // ponytail: stubbed until the agentic edit engine exists — wire up when it does
  function submitPrompt() {
    if (!promptText.trim()) return;
    console.log("Zen prompt submitted:", promptText);
    promptText = "";
  }

  // Window dimensions for responsive boundaries
  let windowWidth = $state(0);
  let windowHeight = $state(0);

  // States for resizable rails
  let leftRailWidth = $state(316);
  let rightRailWidth = $state(316);
  let bottomRailHeight = $state(168);
  let histogramHeight = $state(168);

  // Bounds for responsiveness
  const maxLeftRailWidth = $derived(Math.max(220, Math.min(500, windowWidth * 0.4)));
  const maxRightRailWidth = $derived(Math.max(220, Math.min(500, windowWidth * 0.4)));
  const maxBottomRailHeight = $derived(Math.max(80, Math.min(350, windowHeight * 0.45)));
  const maxHistogramHeight = $derived(Math.max(80, Math.min(350, windowHeight * 0.45)));

  // Constrain side/bottom rails dynamically when bounds or window changes size
  $effect(() => {
    if (leftRailWidth > maxLeftRailWidth) {
      leftRailWidth = maxLeftRailWidth;
    } else if (leftRailWidth < 220) {
      leftRailWidth = 220;
    }
  });

  $effect(() => {
    if (rightRailWidth > maxRightRailWidth) {
      rightRailWidth = maxRightRailWidth;
    } else if (rightRailWidth < 220) {
      rightRailWidth = 220;
    }
  });

  $effect(() => {
    if (bottomRailHeight > maxBottomRailHeight) {
      bottomRailHeight = maxBottomRailHeight;
    } else if (bottomRailHeight < 80) {
      bottomRailHeight = 80;
    }
  });

  $effect(() => {
    if (histogramHeight > maxHistogramHeight) {
      histogramHeight = maxHistogramHeight;
    } else if (histogramHeight < 80) {
      histogramHeight = 80;
    }
  });

  // Drag states & handlers
  let isResizingLeft = $state(false);
  let isResizingRight = $state(false);
  let isResizingBottom = $state(false);
  let isResizingHistogram = $state(false);

  function handleLeftResizeStart(e: MouseEvent) {
    e.preventDefault();
    isResizingLeft = true;
    const startX = e.clientX;
    const startWidth = leftRailWidth;

    function handleMouseMove(moveEvent: MouseEvent) {
      const deltaX = moveEvent.clientX - startX; // drag right increases width
      leftRailWidth = Math.max(220, Math.min(maxLeftRailWidth, startWidth + deltaX));
    }

    function handleMouseUp() {
      isResizingLeft = false;
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
      document.body.style.cursor = "";
    }

    document.body.style.cursor = "col-resize";
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
  }

  function handleLeftKeyDown(e: KeyboardEvent) {
    if (e.key === "ArrowRight") {
      leftRailWidth = Math.max(220, Math.min(maxLeftRailWidth, leftRailWidth + 10));
    } else if (e.key === "ArrowLeft") {
      leftRailWidth = Math.max(220, Math.min(maxLeftRailWidth, leftRailWidth - 10));
    }
  }

  function handleRightResizeStart(e: MouseEvent) {
    e.preventDefault();
    isResizingRight = true;
    const startX = e.clientX;
    const startWidth = rightRailWidth;

    function handleMouseMove(moveEvent: MouseEvent) {
      const deltaX = startX - moveEvent.clientX; // drag left increases width
      rightRailWidth = Math.max(220, Math.min(maxRightRailWidth, startWidth + deltaX));
    }

    function handleMouseUp() {
      isResizingRight = false;
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
      document.body.style.cursor = "";
    }

    document.body.style.cursor = "col-resize";
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
  }

  function handleRightKeyDown(e: KeyboardEvent) {
    if (e.key === "ArrowLeft") {
      rightRailWidth = Math.max(220, Math.min(maxRightRailWidth, rightRailWidth + 10));
    } else if (e.key === "ArrowRight") {
      rightRailWidth = Math.max(220, Math.min(maxRightRailWidth, rightRailWidth - 10));
    }
  }

  function handleBottomResizeStart(e: MouseEvent) {
    e.preventDefault();
    isResizingBottom = true;
    const startY = e.clientY;
    const startHeight = bottomRailHeight;

    function handleMouseMove(moveEvent: MouseEvent) {
      const deltaY = startY - moveEvent.clientY; // drag up increases height
      bottomRailHeight = Math.max(80, Math.min(maxBottomRailHeight, startHeight + deltaY));
    }

    function handleMouseUp() {
      isResizingBottom = false;
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
      document.body.style.cursor = "";
    }

    document.body.style.cursor = "row-resize";
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
  }

  function handleHistogramResizeStart(e: MouseEvent) {
    e.preventDefault();
    isResizingHistogram = true;
    const startY = e.clientY;
    const startHeight = histogramHeight;

    function handleMouseMove(moveEvent: MouseEvent) {
      const deltaY = startY - moveEvent.clientY; // drag up increases height
      histogramHeight = Math.max(80, Math.min(maxHistogramHeight, startHeight + deltaY));
    }

    function handleMouseUp() {
      isResizingHistogram = false;
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
      document.body.style.cursor = "";
    }

    document.body.style.cursor = "row-resize";
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
  }
</script>

<svelte:window bind:innerWidth={windowWidth} bind:innerHeight={windowHeight} />

<div
  in:fade={{ duration: 200, delay: 100 }}
  out:fade={{ duration: 150 }}
  style="
    grid-template-columns: auto minmax(0, 1fr) {isZen ? 0 : rightRailWidth}px;
    grid-template-rows: minmax(0, 1fr) auto 20px;
    transition: {isResizingRight || isResizingBottom || isResizingHistogram ? 'none' : 'grid-template-columns 350ms cubic-bezier(0.16, 1, 0.3, 1)'};
  "
  class="grid h-full min-h-0 gap-[11px] px-[11px] pb-[9px]"
>
  <!-- Animated Left Rail Container -->
  <div 
    style="
      width: {$leftRailCollapsed ? '0px' : `${leftRailWidth}px`};
      margin-right: {$leftRailCollapsed ? '-11px' : '0px'};
      transition: {isResizingLeft ? 'none' : 'width 350ms cubic-bezier(0.16, 1, 0.3, 1), margin-right 350ms cubic-bezier(0.16, 1, 0.3, 1)'};
    "
    class="relative min-h-0 {$leftRailCollapsed ? 'overflow-hidden pointer-events-none' : 'overflow-visible'}"
  >
    <FileBrowser class="h-full {isResizingBottom || isResizingLeft ? 'transition-none' : ''}" />
    <!-- Left Resize Handle -->
    {#if !$leftRailCollapsed}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        role="separator"
        aria-orientation="vertical"
        aria-valuenow={leftRailWidth}
        aria-valuemin={220}
        aria-valuemax={maxLeftRailWidth}
        tabindex="0"
        class="group absolute -right-[11px] top-0 bottom-0 w-[11px] cursor-col-resize z-50 flex items-center justify-center focus:outline-none"
        onmousedown={handleLeftResizeStart}
        onkeydown={handleLeftKeyDown}
      >
        <div class="w-[2px] h-[40px] rounded-full bg-white/5 group-hover:bg-white/25 group-active:bg-accent transition-all duration-200"></div>
      </div>
    {/if}
  </div>

  <div class="relative min-h-0 flex flex-col">
    <MainViewport />
    {#if $leftRailCollapsed}
      <button
        onclick={() => leftRailCollapsed.set(false)}
        aria-label="Expand Sidebar"
        class="absolute left-3 top-1/2 -translate-y-1/2 flex size-[26px] items-center justify-center rounded-full border border-white/5 bg-panel-2 backdrop-blur-md text-white/80 hover:bg-white/[0.12] hover:text-white active:scale-[0.96] transition-[background-color,color,transform] duration-200 cursor-pointer z-50 shadow-md"
      >
        <svg width="6" height="10" viewBox="0 0 6 10" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="pointer-events-none">
          <path d="M1.5 1.5L5 5L1.5 8.5" />
        </svg>
      </button>
    {/if}

    {#if isZen}
      <!-- Agentic edit prompt bar -->
      <div
        in:fly={{ y: 12, duration: 250, delay: 120, easing: cubicOut }}
        out:fade={{ duration: 120 }}
        class="pointer-events-none absolute inset-x-0 bottom-[52px] flex justify-center px-[10%]"
      >
        <div class="pointer-events-auto flex h-[44px] w-full max-w-[480px] items-center gap-[8px] rounded-full border border-white/10 bg-[#171717] px-[6px] shadow-[0_8px_24px_rgba(0,0,0,0.35)]">
          <div class="flex size-[30px] shrink-0 items-center justify-center rounded-full bg-white/[0.08]">
            <img src={aiIcon} alt="" class="size-[15px]" />
          </div>
          <input
            type="text"
            bind:value={promptText}
            placeholder="Describe an edit"
            class="min-w-0 flex-1 bg-transparent text-[12px] text-white placeholder:text-white/35 outline-none"
            onkeydown={(e) => e.key === "Enter" && submitPrompt()}
          />
          <button
            aria-label="Send"
            disabled={!promptText.trim()}
            onclick={submitPrompt}
            class="flex size-[30px] shrink-0 items-center justify-center rounded-full bg-white text-black transition-all disabled:cursor-default disabled:opacity-30 enabled:cursor-pointer enabled:hover:bg-white/90 active:scale-95"
          >
            <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
              <path d="M2 6H10M10 6L6.5 2.5M10 6L6.5 9.5" />
            </svg>
          </button>
        </div>
      </div>
    {/if}
  </div>
  <div class="relative min-h-0 row-span-2 {isZen ? 'overflow-hidden pointer-events-none' : ''}">
    {#if !isZen}
      <!-- Vertical Resize Handle -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        role="separator"
        aria-orientation="vertical"
        aria-valuenow={rightRailWidth}
        aria-valuemin={220}
        aria-valuemax={maxRightRailWidth}
        tabindex="0"
        class="group absolute -left-[11px] top-0 bottom-0 w-[11px] cursor-col-resize z-50 flex items-center justify-center focus:outline-none"
        onmousedown={handleRightResizeStart}
        onkeydown={handleRightKeyDown}
      >
        <div class="w-[2px] h-[40px] rounded-full bg-white/5 group-hover:bg-white/25 group-active:bg-accent transition-all duration-200"></div>
      </div>
    {/if}
    <div
      style="transition: opacity 350ms cubic-bezier(0.16, 1, 0.3, 1);"
      class="flex h-full min-h-0 flex-col gap-[11px] {isZen ? 'opacity-0' : 'opacity-100'}"
    >
      <EditTypeSelector />
      <GlassPanel
        class="flex min-h-0 flex-1 flex-col overflow-hidden {isResizingRight || isResizingHistogram ? 'transition-none' : ''}"
        style="--glass-bg: #171717;"
      >
        <!-- Grid/Cell overlay strategy for zero layout jump settings transitions -->
        <div class="flex-1 min-h-0 overflow-y-auto p-[7px] grid grid-cols-1 grid-rows-1">
          {#key $activeTool}
            <div
              in:fly={{ y: 8, duration: 220, delay: 80, easing: cubicOut }}
              out:fade={{ duration: 100 }}
              class="col-start-1 row-start-1 flex flex-col gap-0 min-h-0"
            >
              {#if $activeTool === "edit"}
                <div class="flex items-center justify-between mb-[10px] mt-[4px] px-[8px] shrink-0">
                  <span class="text-[12px] font-semibold text-white/90">Edit</span>
                  <div class="flex items-center gap-[6px]">
                    <button class="h-[20px] rounded-[10px] bg-white/[0.06] border border-white/[0.04] px-[8px] text-[8px] font-semibold text-white/80 hover:bg-white/[0.12] hover:text-white active:scale-95 transition-all">Auto</button>
                    <button class="h-[20px] rounded-[10px] bg-white/[0.06] border border-white/[0.04] px-[8px] text-[8px] font-semibold text-white/80 hover:bg-white/[0.12] hover:text-white active:scale-95 transition-all">B&W</button>
                  </div>
                </div>
                <ProfileSettings />
                <div class="h-2 shrink-0"></div>
                <LightSettings />
                <div class="h-2 shrink-0"></div>
                <ColorSettings />
                <div class="h-2 shrink-0"></div>
                <DetailSettings />
                <div class="h-2 shrink-0"></div>
                <EffectsSettings />
                <div class="h-2 shrink-0"></div>
                <OpticsSettings />
                <div class="h-2 shrink-0"></div>
                <DemosaicSettings />
                <div class="h-2 shrink-0"></div>
                <LutSettings />
              {:else if $activeTool === "crop"}
                <CropSettings />
              {:else if $activeTool === "mask"}
                <MaskSettings />
              {:else if $activeTool === "ai"}
                <AiSettings />
              {:else if $activeTool === "presets"}
                <PresetSettings />
              {/if}
            </div>
          {/key}
        </div>
      </GlassPanel>
      <Histogram onResizeStart={handleHistogramResizeStart} height={histogramHeight} class={isResizingHistogram ? 'transition-none' : ''} />
    </div>
  </div>
  <!-- Filmstrip: collapses to a slim bar so the expand control stays bottom-left, not over the photo -->
  <div
    style="
      height: {$imageBrowserCollapsed ? '42px' : `${bottomRailHeight}px`};
      transition: {isResizingBottom ? 'none' : 'height 350ms cubic-bezier(0.16, 1, 0.3, 1)'};
    "
    class="relative z-10 col-span-2 min-w-0 {$imageBrowserCollapsed ? 'overflow-hidden' : 'overflow-visible'}"
  >
    <ImageBrowser
      onResizeStart={$imageBrowserCollapsed ? undefined : handleBottomResizeStart}
      class="h-full {isResizingBottom ? 'transition-none' : ''}"
    />
  </div>
  <BottomBar />
</div>

