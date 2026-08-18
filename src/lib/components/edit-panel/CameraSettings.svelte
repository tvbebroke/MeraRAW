<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import Disclosure from "./Disclosure.svelte";
  import ProfileSettings from "./ProfileSettings.svelte";
  import DemosaicSettings from "./DemosaicSettings.svelte";
  import LutSettings from "./LutSettings.svelte";
  import CalibrationSettings from "./CalibrationSettings.svelte";
</script>

<CollapsibleSection id="camera" title="Camera">
  <ProfileSettings />
  <DemosaicSettings />
  <p class="group-label">LUT</p>
  <LutSettings />
  <Disclosure label="Calibration">
    <CalibrationSettings />
  </Disclosure>
</CollapsibleSection>
