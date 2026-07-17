<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import { pickLut, setLut } from "../../../ipc/commands";
  import { doc } from "../../../stores/doc";

  const lutFile = $derived(
    ($doc?.meta as { lut_file?: string } | undefined)?.lut_file ?? null,
  );
  const hasLut = $derived(!!lutFile);

  async function importLut() {
    const path = await pickLut();
    if (!path) return;
    await setLut(path).catch(() => {});
  }

  async function clearLut() {
    await setLut(null).catch(() => {});
  }
</script>

<CollapsibleSection id="lut" title="LUT Conversion">
  <div class="flex flex-col gap-[12px]">
    <div class="text-[8px] text-white/55 truncate" title={lutFile ?? undefined}>
      {lutFile ? lutFile.split("/").pop() : "No LUT loaded"}
    </div>

    {#if hasLut}
      <ParamRow path="lut.opacity" label="Intensity" />
    {/if}

    <button
      onclick={() => void importLut()}
      class="h-[22px] w-full rounded-[6px] border border-white/5 bg-white/[0.03] text-[8px] font-semibold text-white/80 transition-all hover:bg-white/[0.08]"
    >
      Import LUT (.cube)…
    </button>
    {#if hasLut}
      <button
        onclick={() => void clearLut()}
        class="h-[22px] w-full rounded-[6px] border border-white/5 bg-white/[0.02] text-[8px] font-semibold text-white/60 transition-all hover:bg-white/[0.08]"
      >
        Clear LUT
      </button>
    {/if}
  </div>
</CollapsibleSection>
