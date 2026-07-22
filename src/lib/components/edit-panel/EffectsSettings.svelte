<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
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
</script>

<CollapsibleSection id="effects" title="Effects">
  <div class="flex flex-col gap-[12px]">
    <div
      class="rounded-[8px] border border-[rgba(103,103,103,0.05)] bg-[rgba(103,103,103,0.47)] p-[8px] backdrop-blur-[2px]"
    >
      <div class="flex items-center justify-between">
        <p class="text-[6px] font-medium text-white">Split Toning</p>
        <div
          class="grid h-[18px] w-[169px] grid-cols-3 items-center rounded-[22px] bg-black/50 px-[8px]"
        >
          {#each modes as m (m.id)}
            <button
              class="justify-self-center rounded-[22px] px-[4px] pt-[2px] pb-[3px] text-[6px] text-white {activeMode ===
              m.id
                ? 'bg-[#3d3d3d]/80 font-medium'
                : 'cursor-pointer hover:bg-[#3d3d3d]/40'}"
              onclick={() => setMode(m.id)}
            >
              {m.label}
            </button>
          {/each}
        </div>
      </div>

      <div class="mt-[10px] grid grid-cols-3 gap-x-[10px] px-[10px]">
        {#each wheels as w (w.label)}
          <img src={w.icon} alt="{w.label} wheel" class="mx-auto size-[69px]" />
        {/each}
        {#each wheels as w (w.label)}
          <span class="mt-[4px] text-center text-[6px] text-white">{w.label}</span>
        {/each}
      </div>

      <div class="mt-[10px] flex w-full flex-col gap-[6px]">
        <ParamRow path="color_grade.shadows_lum" label="Shadow Lum" labelWidth={72} />
        <ParamRow path="color_grade.midtones_lum" label="Midtones Lum" labelWidth={72} />
        <ParamRow path="color_grade.highlights_lum" label="Highlights Lum" labelWidth={72} />
        <ParamRow path="color_grade.shadow_range" label="Shadow Range" labelWidth={72} />
        <ParamRow path="color_grade.highlight_range" label="Highlight Range" labelWidth={72} />
      </div>
    </div>

    <div class="my-1 h-[1px] bg-white/5"></div>

    <div
      class="rounded-[8px] border border-[rgba(103,103,103,0.05)] bg-[rgba(103,103,103,0.47)] p-[8px] backdrop-blur-[2px]"
    >
      <p class="mb-[8px] text-[6px] font-medium text-white">Zone Hue / Sat</p>
      <div class="flex w-full flex-col gap-[6px]">
        {#each wheels as w (w.label)}
          <ParamRow path={w.hue} label="{w.label} Hue" labelWidth={72} />
          <ParamRow path={w.sat} label="{w.label} Sat" labelWidth={72} />
        {/each}
      </div>
    </div>

    <div class="my-1 h-[1px] bg-white/5"></div>

    <div class="flex flex-col gap-[6px]">
      <p class="mb-[2px] text-[7px] font-bold tracking-wider text-white/40 uppercase">
        Texture &amp; Optics
      </p>
      <ParamRow path="effects.clarity" label="Clarity" labelWidth={72} />
      <ParamRow path="effects.grain_amount" label="Grain" labelWidth={72} />
      <ParamRow path="effects.grain_size" label="Grain Size" labelWidth={72} />
      <ParamRow path="effects.vignette_amount" label="Vignette" labelWidth={72} />
      <ParamRow path="effects.vignette_midpoint" label="Vignette Mid" labelWidth={72} />
    </div>
  </div>

  <img src={divider} alt="" class="mt-[10px] h-[10px] w-full" />
</CollapsibleSection>
