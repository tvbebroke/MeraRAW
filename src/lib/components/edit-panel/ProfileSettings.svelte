<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
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

<CollapsibleSection id="profile" title="Profile">
  {#if !$imageMeta}
    <p class="text-[8px] text-white/40">Open a RAW file to choose a camera profile.</p>
  {:else if files.length === 0}
    <p class="text-[8px] text-white/40">
      No DCP profiles for {$imageMeta.cameraMake} {$imageMeta.cameraModel}.
    </p>
  {:else}
    <div class="relative w-full">
      <select
        value={currentFile}
        onchange={onChange}
        class="profile-select h-[28px] w-full text-[9px] font-medium text-white/90 focus:outline-none"
      >
        {#each files as file, i (file)}
          <option value={file}>{names[i] ?? file}</option>
        {/each}
      </select>
    </div>
  {/if}
</CollapsibleSection>

<style>
  .profile-select {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(33, 33, 35, 0.65)
      url("data:image/svg+xml,%3Csvg width='8' height='5' viewBox='0 0 8 5' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1L4 4L7 1' stroke='rgba%28255,255,255,0.6%29' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")
      no-repeat right 10px center;
    background-size: 8px 5px;
    border-radius: 8px;
    padding: 0 24px 0 10px;
    outline: none;
    cursor: pointer;
  }
  .profile-select option {
    background: #222224;
    color: #fff;
    font-size: 9px;
  }
</style>
