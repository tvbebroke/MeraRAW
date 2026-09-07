<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import Disclosure from "./Disclosure.svelte";
</script>

<CollapsibleSection id="light" title="Light">
  <ParamRow path="exposure.stops" label="Exposure" />
  <ParamRow path="highlights.amount" label="Highlight reconstruct" />
  <ParamRow path="tone_curve.contrast" label="Contrast" />
  <ParamRow path="tone_curve.highlights" label="Highlights" />
  <ParamRow path="tone_curve.shadows" label="Shadows" />
  <ParamRow path="tone_curve.sigmoid" label="Sigmoid" />
  <ParamRow path="effects.dehaze" label="Dehaze" />
  <Disclosure label="Advanced">
    <ParamRow path="highlights.clip" label="Highlight clip" />
    <ParamRow path="highlights.enabled" label="Highlights on" />
    <ParamRow path="tone_curve.lights" label="Whites" />
    <ParamRow path="tone_curve.darks" label="Blacks" />
    <ParamRow path="effects.clarity" label="Clarity" />
    <ParamRow path="effects.vignette_amount" label="Vignette" />
    <ParamRow path="effects.vignette_midpoint" label="Vignette mid" />
    <ParamRow path="effects.enabled" label="Effects on" />
  </Disclosure>
</CollapsibleSection>
