<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import Disclosure from "./Disclosure.svelte";
</script>

<CollapsibleSection id="light" title="Light">
  <ParamRow path="exposure.stops" label="Exposure" />
  <ParamRow path="tone_curve.contrast" label="Contrast" />
  <ParamRow path="tone_curve.highlights" label="Highlights" />
  <ParamRow path="tone_curve.shadows" label="Shadows" />
  <Disclosure label="Advanced">
    <ParamRow path="tone_curve.lights" label="Whites" />
    <ParamRow path="tone_curve.darks" label="Blacks" />
  </Disclosure>
</CollapsibleSection>
