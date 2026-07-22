<!-- Registry-driven slider row: label + Slider bound to a param path.
     Renders nothing when the path isn't in the registry. -->
<script lang="ts">
  import Slider from "./Slider.svelte";
  import { registry, effectiveValue, defaultValue, setParamLive, commitParam } from "../../engine/params";
  import { doc } from "../../../stores/doc";
  import { imageMeta, selectedMask } from "../../../stores/app";

  let {
    path,
    label,
    labelWidth = 76,
    disabled = false,
  }: {
    path: string;
    label?: string;
    labelWidth?: number;
    disabled?: boolean;
  } = $props();

  const spec = $derived($registry.find((s) => s.path === path));
  const value = $derived(
    spec ? effectiveValue($doc, spec, $imageMeta, $selectedMask) : 0,
  );
  const reset = $derived(spec ? defaultValue(spec, $imageMeta, $selectedMask) : 0);
</script>

{#if spec}
  <div
    class="grid h-[16px] items-center gap-x-[10px]"
    style="grid-template-columns: {labelWidth}px 1fr;"
  >
    <span
      class="truncate text-[11px] font-medium text-white/90"
      title={label ?? spec.ui.label}
    >
      {label ?? spec.ui.label}
    </span>
    <Slider
      label={label ?? spec.ui.label}
      min={spec.min}
      max={spec.max}
      step={spec.ui.step}
      {value}
      resetValue={reset}
      {disabled}
      oninput={(v) => setParamLive(path, v)}
      onchange={(v) => void commitParam(path, v)}
    />
  </div>
{/if}
