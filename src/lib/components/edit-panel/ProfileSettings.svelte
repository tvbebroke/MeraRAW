<script lang="ts">
  import { setCameraProfile } from "../../../ipc/commands";
  import { imageMeta } from "../../../stores/app";

  async function onChange(e: Event) {
    const file = (e.currentTarget as HTMLSelectElement).value;
    if (!file) return;
    try {
      const m = await setCameraProfile(file);
      imageMeta.set(m);
    } catch {
      /* ignore */
    }
  }

  const files = $derived($imageMeta?.availableProfileFiles ?? []);
  const names = $derived($imageMeta?.availableProfiles ?? []);
  const current = $derived($imageMeta?.cameraProfile ?? "");
  const currentFile = $derived.by(() => {
    const idx = names.findIndex((n) => n === current);
    return files[idx >= 0 ? idx : 0] ?? files[0] ?? "";
  });
</script>

{#if !$imageMeta}
  <p class="rail-empty">Open a RAW file to choose a camera profile.</p>
{:else if files.length === 0}
  <p class="rail-empty">
    No DCP profiles for {$imageMeta.cameraMake} {$imageMeta.cameraModel}.
  </p>
{:else}
  <select value={currentFile} onchange={onChange} class="rail-select">
    {#each files as file, i (file)}
      <option value={file}>{names[i] ?? file}</option>
    {/each}
  </select>
{/if}
