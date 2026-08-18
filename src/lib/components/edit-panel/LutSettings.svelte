<script lang="ts">
  import ParamRow from "./ParamRow.svelte";
  import { pickLut, setLut } from "../../../ipc/commands";
  import { doc } from "../../../stores/doc";

  const lutFile = $derived(
    ($doc?.meta as { lut_file?: string } | undefined)?.lut_file ?? null,
  );
  const hasLut = $derived(!!lutFile);
  const lutName = $derived(lutFile ? lutFile.split("/").pop() : "No LUT loaded");

  async function importLut() {
    const path = await pickLut();
    if (!path) return;
    await setLut(path).catch(() => {});
  }

  async function clearLut() {
    await setLut(null).catch(() => {});
  }
</script>

<p class="rail-empty" title={lutFile ?? undefined}>{lutName}</p>
{#if hasLut}
  <ParamRow path="lut.opacity" label="Intensity" />
{/if}
<button type="button" class="rail-btn full" onclick={() => void importLut()}>
  Import LUT (.cube)…
</button>
{#if hasLut}
  <button type="button" class="rail-btn full" onclick={() => void clearLut()}>
    Clear LUT
  </button>
{/if}

<style>
  .full { width: 100%; margin-top: var(--space-2); }
</style>
