<script lang="ts">
  import { onMount } from "svelte";
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import {
    applyPreset,
    listPresetCatalog,
    type PresetCatalogEntry,
  } from "../../../ipc/commands";
  import { reconcile } from "../../../stores/doc";

  let presets = $state<PresetCatalogEntry[]>([]);
  let selected = $state("");
  let filter = $state("");

  onMount(() => {
    void listPresetCatalog()
      .then((p) => (presets = p))
      .catch(() => (presets = []));
  });

  const visible = $derived(
    presets.filter((p) => {
      if (!filter) return true;
      const q = filter.toLowerCase();
      return (
        p.label.toLowerCase().includes(q) ||
        p.tags.some((t) => t.toLowerCase().includes(q))
      );
    }),
  );

  async function apply(id: string) {
    selected = id;
    try {
      reconcile(await applyPreset(id));
    } catch {
      /* ignore */
    }
  }
</script>

<CollapsibleSection id="presetList" title="Presets Library">
  <div class="flex flex-col gap-[8px]">
    <input
      class="h-[24px] rounded-[8px] border border-white/5 bg-white/[0.03] px-2 text-[9px] text-white/80 outline-none"
      placeholder="Filter presets…"
      bind:value={filter}
    />
    {#if visible.length === 0}
      <p class="text-[8px] text-white/35">No presets found.</p>
    {:else}
      {#each visible as preset (preset.id)}
        <button
          onclick={() => void apply(preset.id)}
          class="flex items-center justify-between rounded-[12px] border p-[10px] text-left transition-all {selected ===
          preset.id
            ? 'border-accent bg-accent/10 text-white'
            : 'border-white/5 bg-white/[0.02] text-white/80 hover:bg-white/5'}"
        >
          <span class="text-[10px] font-medium">{preset.label}</span>
          <span class="text-[8px] text-white/40"
            >{preset.tags[0] ?? ""}</span
          >
        </button>
      {/each}
    {/if}
  </div>
</CollapsibleSection>
