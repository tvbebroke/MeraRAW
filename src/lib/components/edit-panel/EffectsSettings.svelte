<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import Slider from "./Slider.svelte";
  import { setParam } from "../../../ipc/commands";
  import { reconcile, docParam, doc } from "../../../stores/doc";
  import wheelShadows from "../../icons/wheel-shadows.svg";
  import wheelMidtones from "../../icons/wheel-midtones.svg";
  import wheelHighlights from "../../icons/wheel-highlights.svg";
  import divider from "../../icons/effects-divider.svg";

  type Mode = "perceptual" | "classic" | "light";
  const modes: { id: Mode; label: string; value: number }[] = [
    { id: "perceptual", label: "Perceptual", value: 0 },
    { id: "classic", label: "Classic", value: 1 },
    { id: "light", label: "Light", value: 2 },
  ];

  const model = $derived(docParam($doc, "color_grade", "model") ?? 0);
  const activeMode = $derived(
    modes.find((m) => m.value === model)?.id ?? "perceptual",
  );

  function setMode(m: Mode) {
    const v = modes.find((x) => x.id === m)?.value ?? 0;
    void setParam("color_grade.model", v).then(reconcile).catch(() => {});
  }

  const wheels = [
    { icon: wheelShadows, label: "Shadows", hue: "color_grade.shadows_hue", sat: "color_grade.shadows_sat" },
    { icon: wheelMidtones, label: "Midtones", hue: "color_grade.midtones_hue", sat: "color_grade.midtones_sat" },
    { icon: wheelHighlights, label: "Highlights", hue: "color_grade.highlights_hue", sat: "color_grade.highlights_sat" },
  ];

  // NOTE: the upstream reference UI (meraraw-ui-svelte) added an interactive
  // draggable-crosshair color wheel here. It maps drag position to an
  // unsigned 0-100 saturation radius, but the real engine params
  // (color_grade.*_sat) are signed -100..100, so the drag math can't be
  // ported without redefining that param's semantics. Left as the static
  // icon + ParamRow sliders below (kept from local) until that's resolved.

  // --- Placeholder sub-panels adopted from upstream restyle ---------------
  // These sections (Denoise / Presence / Vignette) exist in the upstream
  // reference UI but have no corresponding engine params in this app yet
  // (verified against src-tauri/core/src/registry.rs). They're wired to
  // local component state only, purely cosmetic, and don't dispatch
  // anything to the engine. See final report for what registry/store
  // support would be needed to make them real.
  let denoiseAmount = $state(0);
  let denoiseDetail = $state(50);
  let denoiseContrast = $state(0);

  let texture = $state(0);
  let clarity = $state(0);
  let dehaze = $state(0);

  let vignetteAmount = $state(0);
  let vignetteMidpoint = $state(50);
  let vignetteFeather = $state(50);
  let vignetteRoundness = $state(0);
</script>

<CollapsibleSection id="effects" title="Effects">
  <div class="flex flex-col gap-[12px]">
    <div
      class="rounded-[8px] border border-[rgba(103,103,103,0.05)] bg-[rgba(103,103,103,0.47)] p-[8px] backdrop-blur-[2px]"
    >
      <div class="flex items-center justify-between gap-2">
        <p class="text-[11px] font-medium text-white shrink-0">Split Toning</p>
        
        <div class="split-toning-bar relative flex h-[28px] w-[205px] items-center rounded-full bg-black/50 p-[3px] border border-white/10 select-none">
          <div
            class="absolute top-[3px] bottom-[3px] rounded-full bg-white/20 border border-white/10 shadow-[0_1px_4px_rgba(0,0,0,0.3)] transition-all duration-300 ease-[cubic-bezier(0.16,1,0.3,1)] pointer-events-none z-0"
            style="
              width: calc((100% - 6px) / 3);
              left: calc(3px + {modes.findIndex((m) => m.id === activeMode)} * (100% - 6px) / 3);
            "
          ></div>

          {#each modes as m (m.id)}
            <button
              class="relative z-10 flex h-full flex-1 items-center justify-center rounded-full text-[11px] font-light transition-colors duration-200 cursor-pointer {activeMode ===
              m.id
                ? 'text-white font-medium'
                : 'text-white/50 hover:text-white/85'}"
              onclick={() => setMode(m.id)}
            >
              {m.label}
            </button>
          {/each}
        </div>
      </div>

      <div class="mt-[12px] grid grid-cols-3 gap-x-[12px] px-[6px]">
        {#each wheels as w (w.label)}
          <img src={w.icon} alt="{w.label} wheel" class="mx-auto size-[72px]" />
        {/each}
        {#each wheels as w (w.label)}
          <span class="mt-[6px] text-center text-[11px] text-white/90">{w.label}</span>
        {/each}
      </div>

      <div class="mt-[12px] flex w-full flex-col gap-[6px]">
        <ParamRow path="color_grade.shadows_lum" label="Shadow Lum" labelWidth={84} />
        <ParamRow path="color_grade.midtones_lum" label="Midtones Lum" labelWidth={84} />
        <ParamRow path="color_grade.highlights_lum" label="Highlights Lum" labelWidth={84} />
        <ParamRow path="color_grade.shadow_range" label="Shadow Range" labelWidth={84} />
        <ParamRow path="color_grade.highlight_range" label="Highlight Range" labelWidth={84} />
      </div>
    </div>

    <div class="my-1 h-[1px] bg-white/5"></div>

    <div
      class="rounded-[8px] border border-[rgba(103,103,103,0.05)] bg-[rgba(103,103,103,0.47)] p-[8px] backdrop-blur-[2px]"
    >
      <p class="mb-[8px] text-[11px] font-medium text-white">Zone Hue / Sat</p>
      <div class="flex w-full flex-col gap-[6px]">
        {#each wheels as w (w.label)}
          <ParamRow path={w.hue} label="{w.label} Hue" labelWidth={84} />
          <ParamRow path={w.sat} label="{w.label} Sat" labelWidth={84} />
        {/each}
      </div>
    </div>

    <div class="my-1 h-[1px] bg-white/5"></div>

    <!-- Denoise (cosmetic UI) -->
    <div
      class="rounded-[8px] border border-[rgba(103,103,103,0.05)] bg-[rgba(103,103,103,0.47)] p-[8px] backdrop-blur-[2px]"
    >
      <p class="mb-[8px] text-[11px] font-medium text-white">Denoise</p>
      <div class="flex w-full flex-col gap-[6px]">
        <div class="grid h-[16px] grid-cols-[84px_1fr] items-center gap-x-[8px]">
          <span class="text-[11px] text-white whitespace-nowrap">Denoise Amt</span>
          <Slider label="Denoise Amount" min={0} max={100} value={denoiseAmount} onchange={(v) => (denoiseAmount = v)} />
        </div>
        <div class="grid h-[16px] grid-cols-[84px_1fr] items-center gap-x-[8px]">
          <span class="text-[11px] text-white whitespace-nowrap">Detail</span>
          <Slider label="Denoise Detail" min={0} max={100} value={denoiseDetail} onchange={(v) => (denoiseDetail = v)} />
        </div>
        <div class="grid h-[16px] grid-cols-[84px_1fr] items-center gap-x-[8px]">
          <span class="text-[11px] text-white whitespace-nowrap">Contrast</span>
          <Slider label="Denoise Contrast" min={0} max={100} value={denoiseContrast} onchange={(v) => (denoiseContrast = v)} />
        </div>
      </div>
    </div>

    <div class="my-1 h-[1px] bg-white/5"></div>

    <!-- Presence (cosmetic UI) -->
    <div
      class="rounded-[8px] border border-[rgba(103,103,103,0.05)] bg-[rgba(103,103,103,0.47)] p-[8px] backdrop-blur-[2px]"
    >
      <p class="mb-[8px] text-[11px] font-medium text-white">Presence</p>
      <div class="flex w-full flex-col gap-[6px]">
        <div class="grid h-[16px] grid-cols-[84px_1fr] items-center gap-x-[8px]">
          <span class="text-[11px] text-white whitespace-nowrap">Texture</span>
          <Slider label="Texture" min={-100} max={100} value={texture} onchange={(v) => (texture = v)} />
        </div>
        <div class="grid h-[16px] grid-cols-[84px_1fr] items-center gap-x-[8px]">
          <span class="text-[11px] text-white whitespace-nowrap">Clarity</span>
          <Slider label="Clarity" min={-100} max={100} value={clarity} onchange={(v) => (clarity = v)} />
        </div>
        <div class="grid h-[16px] grid-cols-[84px_1fr] items-center gap-x-[8px]">
          <span class="text-[11px] text-white whitespace-nowrap">Dehaze</span>
          <Slider label="Dehaze" min={-100} max={100} value={dehaze} onchange={(v) => (dehaze = v)} />
        </div>
      </div>
    </div>

    <div class="my-1 h-[1px] bg-white/5"></div>

    <!-- Vignette (cosmetic UI) -->
    <div
      class="rounded-[8px] border border-[rgba(103,103,103,0.05)] bg-[rgba(103,103,103,0.47)] p-[8px] backdrop-blur-[2px]"
    >
      <p class="mb-[8px] text-[11px] font-medium text-white">Vignette</p>
      <div class="flex w-full flex-col gap-[6px]">
        <div class="grid h-[16px] grid-cols-[84px_1fr] items-center gap-x-[8px]">
          <span class="text-[11px] text-white whitespace-nowrap">Amount</span>
          <Slider label="Vignette Amount" min={-100} max={100} value={vignetteAmount} onchange={(v) => (vignetteAmount = v)} />
        </div>
        <div class="grid h-[16px] grid-cols-[84px_1fr] items-center gap-x-[8px]">
          <span class="text-[11px] text-white whitespace-nowrap">Midpoint</span>
          <Slider label="Vignette Midpoint" min={0} max={100} value={vignetteMidpoint} onchange={(v) => (vignetteMidpoint = v)} />
        </div>
        <div class="grid h-[16px] grid-cols-[84px_1fr] items-center gap-x-[8px]">
          <span class="text-[11px] text-white whitespace-nowrap">Feather</span>
          <Slider label="Vignette Feather" min={0} max={100} value={vignetteFeather} onchange={(v) => (vignetteFeather = v)} />
        </div>
        <div class="grid h-[16px] grid-cols-[84px_1fr] items-center gap-x-[8px]">
          <span class="text-[11px] text-white whitespace-nowrap">Roundness</span>
          <Slider label="Vignette Roundness" min={-100} max={100} value={vignetteRoundness} onchange={(v) => (vignetteRoundness = v)} />
        </div>
      </div>
    </div>
  </div>

  <img src={divider} alt="" class="mt-[10px] h-[10px] w-full" />
</CollapsibleSection>
