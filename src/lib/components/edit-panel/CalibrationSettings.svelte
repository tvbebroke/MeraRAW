<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
</script>

<CollapsibleSection id="calibration" title="Calibration">
  <div class="flex flex-col gap-[6px]">
    <ParamRow path="calibration.red_hue" label="Red Hue" labelWidth={76} />
    <ParamRow path="calibration.red_sat" label="Red Sat" labelWidth={76} />
    <ParamRow path="calibration.green_hue" label="Green Hue" labelWidth={76} />
    <ParamRow path="calibration.green_sat" label="Green Sat" labelWidth={76} />
    <ParamRow path="calibration.blue_hue" label="Blue Hue" labelWidth={76} />
    <ParamRow path="calibration.blue_sat" label="Blue Sat" labelWidth={76} />
    <ParamRow path="calibration.shadow_tint" label="Shadow Tint" labelWidth={76} />
  </div>
</CollapsibleSection>
