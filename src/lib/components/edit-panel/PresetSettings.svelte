<script lang="ts">
  import { onMount } from "svelte";
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import {
    applyPreset,
    listPresetCatalog,
    savePresetNamed,
    type PresetCatalogEntry,
  } from "../../../ipc/commands";
  import { reconcile, doc } from "../../../stores/doc";

  /** Develop modules worth bundling into a look preset (skip crop/geometry). */
  const SAVE_MODULES = [
    "exposure",
    "white_balance",
    "calibration",
    "detail",
    "color_grade",
    "hsl",
    "tone_curve",
    "lut",
    "effects",
  ];

  let presets = $state<PresetCatalogEntry[]>([]);
  let selected = $state("");
  let filter = $state("");
  let saveName = $state("");
  let saveStatus = $state("");

  async function refresh() {
    try {
      presets = await listPresetCatalog();
    } catch {
      presets = [];
    }
  }

  onMount(() => {
    void refresh();
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

  async function save() {
    const name = saveName.trim();
    if (!name) {
      saveStatus = "Enter a name";
      return;
    }
    if (!$doc) {
      saveStatus = "Open an image first";
      return;
    }
    try {
      await savePresetNamed(name, SAVE_MODULES);
      saveStatus = `Saved “${name}”`;
      saveName = "";
      await refresh();
    } catch (e) {
      saveStatus = `Save failed: ${e}`;
    }
  }
</script>

<CollapsibleSection id="presetList" title="Presets Library">
  <div class="flex flex-col gap-[8px]">
    <div class="flex gap-[6px]">
      <input
        class="h-[24px] min-w-0 flex-1 rounded-[8px] border border-white/5 bg-white/[0.03] px-2 text-[9px] text-white/80 outline-none"
        placeholder="Preset name…"
        bind:value={saveName}
        onkeydown={(e) => {
          if (e.key === "Enter") void save();
        }}
      />
      <button
        type="button"
        class="h-[24px] shrink-0 rounded-[8px] border border-white/5 bg-white/[0.06] px-2 text-[9px] font-medium text-white/80 hover:bg-white/10"
        onclick={() => void save()}
      >
        Save
      </button>
    </div>
    {#if saveStatus}
      <p class="px-1 text-[8px] text-white/40">{saveStatus}</p>
    {/if}

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
