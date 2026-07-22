<script lang="ts">
  // NOTE: lens-profile corrections are not wired in the engine yet (no
  // src-tauri optics module / registry params exist). This UI is adopted
  // wholesale from the upstream reference (meraraw-ui-svelte) as a visual
  // mockup — all state below is local component state only and nothing here
  // dispatches to the engine. Replace with ParamRow/setParam wiring once the
  // optics module ships.
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ToggleSwitch from "../primitives/ToggleSwitch.svelte";

  let removeCA = $state(false);
  let enableProfile = $state(false);

  let selectedMake = $state("Sony");
  let selectedModel = $state("FE 24-70mm F2.8 GM");
  let selectedProfile = $state("Adobe Standard Profile");

  const makes = ["Sony", "Canon", "Nikon", "Apple", "Sigma", "Tamron"];
  
  const models: Record<string, string[]> = {
    Sony: ["FE 24-70mm F2.8 GM", "FE 50mm F1.2 GM", "FE 85mm F1.4 GM"],
    Canon: ["RF 24-70mm F2.8L IS USM", "EF 50mm f/1.8 STM", "EF 24-105mm f/4L IS USM"],
    Nikon: ["NIKKOR Z 24-70mm f/2.8 S", "NIKKOR Z 50mm f/1.8 S"],
    Apple: ["iPhone 15 Pro Main Camera", "iPad Pro Back Camera"],
    Sigma: ["24-70mm F2.8 DG DN Art", "50mm F1.4 DG DN Art"],
    Tamron: ["28-75mm F2.8 Di III VXD G2"]
  };

  // Adjust model when make changes
  $effect(() => {
    const list = models[selectedMake] || [];
    if (!list.includes(selectedModel)) {
      selectedModel = list[0] || "";
    }
  });

  let fileInput = $state<HTMLInputElement>();
  let customProfiles = $state<string[]>([]);

  function triggerAddProfile() {
    fileInput?.click();
  }

  function handleFileAdded(e: Event) {
    const files = (e.target as HTMLInputElement).files;
    if (files && files.length > 0) {
      const name = files[0].name.replace(/\.[^/.]+$/, ""); // strip extension
      customProfiles = [...customProfiles, name];
      selectedProfile = name;
      alert(`Lens profile "${name}" successfully loaded.`);
    }
  }
</script>

<CollapsibleSection id="optics" title="Optics">
  <div class="flex flex-col gap-[12px]">
    <!-- Chromatic Aberration Toggle -->
    <div class="flex items-center justify-between py-[4px]">
      <span class="text-[8px] text-white/80 font-medium">Remove Chromatic Aberration</span>
      <div class="scale-75 origin-right">
        <ToggleSwitch checked={removeCA} label="Remove Chromatic Aberration" onchange={(v) => removeCA = v} />
      </div>
    </div>

    <!-- Profile Corrections Toggle -->
    <div class="flex items-center justify-between py-[4px]">
      <span class="text-[8px] text-white/80 font-medium">Use Profile Corrections</span>
      <div class="scale-75 origin-right">
        <ToggleSwitch checked={enableProfile} label="Use Profile Corrections" onchange={(v) => enableProfile = v} />
      </div>
    </div>

    {#if enableProfile}
      <!-- Lens Profile Selection card -->
      <div
        class="mt-[4px] rounded-[8px] border border-[rgba(103,103,103,0.05)] bg-[rgba(103,103,103,0.47)] p-[8px] backdrop-blur-[2px] flex flex-col gap-[8px]"
      >
        <div class="text-[7px] font-bold text-white/40 tracking-wider">Lens profile</div>
        
        <!-- Make Dropdown -->
        <div class="grid grid-cols-[48px_1fr] items-center gap-x-[8px]">
          <span class="text-[8px] text-white/70">Make</span>
          <div class="relative w-full">
            <select
              bind:value={selectedMake}
              class="optics-select w-full h-[24px] text-[8px] text-white/90 focus:outline-none"
            >
              {#each makes as m}
                <option value={m}>{m}</option>
              {/each}
            </select>
          </div>
        </div>

        <!-- Model Dropdown -->
        <div class="grid grid-cols-[48px_1fr] items-center gap-x-[8px]">
          <span class="text-[8px] text-white/70">Model</span>
          <div class="relative w-full">
            <select
              bind:value={selectedModel}
              class="optics-select w-full h-[24px] text-[8px] text-white/90 focus:outline-none"
            >
              {#each (models[selectedMake] || []) as m}
                <option value={m}>{m}</option>
              {/each}
            </select>
          </div>
        </div>

        <!-- Profile Dropdown -->
        <div class="grid grid-cols-[48px_1fr] items-center gap-x-[8px]">
          <span class="text-[8px] text-white/70">Profile</span>
          <div class="relative w-full">
            <select
              bind:value={selectedProfile}
              class="optics-select w-full h-[24px] text-[8px] text-white/90 focus:outline-none"
            >
              <option value="Adobe Standard Profile">Adobe Standard Profile</option>
              {#each customProfiles as p}
                <option value={p}>{p}</option>
              {/each}
            </select>
          </div>
        </div>

        <!-- Add Custom Profile Mechanism -->
        <input
          bind:this={fileInput}
          type="file"
          accept=".lcp,.xml"
          class="hidden"
          onchange={handleFileAdded}
        />
        <button
          onclick={triggerAddProfile}
          class="mt-[4px] h-[22px] w-full rounded-[6px] border border-white/5 bg-white/[0.03] text-[8px] font-semibold text-white/80 hover:bg-white/[0.08] active:scale-98 transition-all"
        >
          Add Custom Lens Profile...
        </button>
      </div>
    {/if}
  </div>
</CollapsibleSection>

<style>
  .optics-select {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(33, 33, 35, 0.6) url("data:image/svg+xml,%3Csvg width='8' height='5' viewBox='0 0 8 5' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1L4 4L7 1' stroke='rgba%28255,255,255,0.6%29' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E") no-repeat right 8px center;
    background-size: 8px 5px;
    border-radius: 6px;
    padding: 0 20px 0 8px;
    outline: none;
    cursor: pointer;
  }

  .optics-select option {
    background: #222224;
    color: #fff;
    font-size: 8px;
  }
</style>
