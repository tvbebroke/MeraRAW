<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import ToneCurve from "./ToneCurve.svelte";
</script>

<CollapsibleSection id="light" title="Light">
  <div class="flex flex-col gap-[6px]">
    <ParamRow path="exposure.stops" label="Exposure" labelWidth={48} />
    <ParamRow path="tone_curve.contrast" label="Contrast" labelWidth={48} />
    <ParamRow path="tone_curve.highlights" label="Highlights" labelWidth={48} />
    <ParamRow path="tone_curve.shadows" label="Shadows" labelWidth={48} />
    <ParamRow path="tone_curve.lights" label="Whites" labelWidth={48} />
    <ParamRow path="tone_curve.darks" label="Blacks" labelWidth={48} />
  </div>
  <div class="mt-[14px] flex justify-center w-full">
    <ToneCurve />
  </div>
</CollapsibleSection>
