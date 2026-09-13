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
  import { presetSnapshotUrl } from "../../../stores/app";
  import { gradeClipboard } from "../../grade";
  import PresetThumb from "./PresetThumb.svelte";

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
    "input",
    "highlights",
  ];

  let presets = $state<PresetCatalogEntry[]>([]);
  let selected = $derived(
    String(($doc?.meta as { preset_id?: string } | undefined)?.preset_id ?? ""),
  );
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
      return p.label.toLowerCase().includes(q) || p.tags.some((t) => t.toLowerCase().includes(q));
    }),
  );

  async function apply(id: string) {
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

  async function saveCopied() {
    const name = saveName.trim();
    if (!name) {
      saveStatus = "Enter a name";
      return;
    }
    const clip = $gradeClipboard;
    if (!clip) {
      saveStatus = "Copy a grade first";
      return;
    }
    try {
      await savePresetNamed(name, [], clip.modules);
      saveStatus = `Saved copied grade “${name}”`;
      saveName = "";
      await refresh();
    } catch (e) {
      saveStatus = `Save failed: ${e}`;
    }
  }
</script>

<CollapsibleSection id="presets" title="Presets">
  <div class="save-row">
    <input
      class="rail-input"
      placeholder="Preset name…"
      bind:value={saveName}
      onkeydown={(e) => {
        if (e.key === "Enter") void save();
      }}
    />
    <button type="button" class="rail-btn" onclick={() => void save()}>Save</button>
    {#if $gradeClipboard}
      <button type="button" class="rail-btn" onclick={() => void saveCopied()}>Save copy</button>
    {/if}
  </div>
  {#if saveStatus}
    <p class="rail-empty">{saveStatus}</p>
  {/if}

  <input class="rail-input" placeholder="Filter presets…" bind:value={filter} />
  <p class="rail-empty">One preset at a time. Applying another replaces the last.</p>

  {#if visible.length === 0}
    <p class="rail-empty">No presets found.</p>
  {:else}
    {#each visible as preset (preset.id)}
      <button
        type="button"
        class="item"
        class:on={selected === preset.id}
        onclick={() => void apply(preset.id)}
      >
        <PresetThumb id={preset.id} modules={preset.modules} photoUrl={$presetSnapshotUrl} />
        <span class="meta">
          <span class="name">{preset.label}</span>
          {#if preset.tags[0]}
            <span class="tag">{preset.tags[0]}</span>
          {/if}
        </span>
      </button>
    {/each}
  {/if}
</CollapsibleSection>

<style>
  .save-row {
    display: flex;
    gap: var(--space-2);
    margin-bottom: var(--space-2);
  }
  .save-row .rail-btn { flex: none; padding: 0 var(--space-2); }
  .rail-input { margin-bottom: var(--space-2); }
  .item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    min-height: 58px;
    padding: 4px var(--space-2);
    margin-bottom: var(--space-1);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-hover);
    color: var(--color-fg);
    font-size: var(--text-ui);
    text-align: left;
    cursor: pointer;
  }
  .item.on {
    border-color: var(--color-accent);
    background: var(--color-accent-soft);
  }
  .item:hover { background: var(--color-active); }
  .meta {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tag { color: var(--color-subtle); font-size: 11px; }
</style>
