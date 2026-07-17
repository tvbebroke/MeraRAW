<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import { applyOp, setMaskOverlay } from "../../../ipc/commands";
  import { doc, reconcile } from "../../../stores/doc";
  import { selectedMask, viewportTool, brushRadius } from "../../../stores/app";

  const masks = $derived($doc?.masks ?? []);

  async function addMask(kind: "brush" | "linear" | "radial") {
    const source =
      kind === "brush"
        ? { type: "brush", strokes: [] }
        : kind === "linear"
          ? { type: "linear", x0: 0.5, y0: 0.2, x1: 0.5, y1: 0.8 }
          : { type: "radial", cx: 0.5, cy: 0.5, rx: 0.3, ry: 0.3 };
    try {
      const delta = await applyOp({ op: "add_mask", kind, source });
      reconcile(delta);
      if (delta.newMaskId) {
        selectedMask.set(delta.newMaskId);
        viewportTool.set("brush");
        void setMaskOverlay(delta.newMaskId);
      }
    } catch {
      /* ignore */
    }
  }

  async function removeMask(id: string) {
    try {
      reconcile(await applyOp({ op: "remove_mask", id }));
      if ($selectedMask === id) {
        selectedMask.set(null);
        void setMaskOverlay(null);
      }
    } catch {
      /* ignore */
    }
  }

  function select(id: string) {
    selectedMask.set(id);
    viewportTool.set("brush");
    void setMaskOverlay(id);
  }
</script>

<CollapsibleSection id="maskList" title="Masking Tools">
  <div class="flex flex-col gap-[8px]">
    <div class="mb-1 px-1 text-[10px] font-bold text-white/50">CREATE NEW MASK</div>
    <div class="grid grid-cols-2 gap-[8px]">
      <button
        onclick={() => void addMask("brush")}
        class="flex h-[36px] flex-col items-center justify-center rounded-[12px] border border-white/5 bg-white/[0.02] text-[10px] text-white/80 transition-all hover:bg-white/5"
      >
        <span class="font-medium">Brush Mask</span>
      </button>
      <button
        onclick={() => void addMask("linear")}
        class="flex h-[36px] flex-col items-center justify-center rounded-[12px] border border-white/5 bg-white/[0.02] text-[10px] text-white/80 transition-all hover:bg-white/5"
      >
        <span class="font-medium">Linear Gradient</span>
      </button>
      <button
        onclick={() => void addMask("radial")}
        class="flex h-[36px] flex-col items-center justify-center rounded-[12px] border border-white/5 bg-white/[0.02] text-[10px] text-white/80 transition-all hover:bg-white/5"
      >
        <span class="font-medium">Radial Gradient</span>
      </button>
    </div>

    <div class="grid h-[15px] grid-cols-[64px_1fr] items-center gap-x-[10px] px-1">
      <span class="text-[9px] text-white/70">Brush</span>
      <input
        type="range"
        min="0.01"
        max="0.25"
        step="0.005"
        value={$brushRadius}
        oninput={(e) =>
          brushRadius.set(parseFloat((e.currentTarget as HTMLInputElement).value))}
        class="w-full"
      />
    </div>

    <div class="my-2 h-[1px] bg-white/5"></div>

    <div class="mb-1 px-1 text-[10px] font-bold text-white/50">ACTIVE MASKS</div>
    {#if masks.length === 0}
      <p class="px-1 text-[9px] text-white/35">No masks yet.</p>
    {:else}
      {#each masks as m (m.id)}
        <div
          class="flex items-center justify-between rounded-[12px] border p-2 {$selectedMask === m.id
            ? 'border-accent/40 bg-accent/10'
            : 'border-white/5 bg-white/[0.03]'}"
        >
          <button
            class="flex items-center gap-2 text-left"
            onclick={() => select(m.id)}
          >
            <div
              class="flex size-[18px] items-center justify-center rounded-full border border-accent/40 bg-accent/20 text-[9px] font-semibold text-accent"
            >
              {(m.kind ?? "m")[0]?.toUpperCase() ?? "M"}
            </div>
            <span class="text-[10px] text-white/80">{m.kind ?? "Mask"} · {m.id.slice(0, 6)}</span>
          </button>
          <button
            class="text-[9px] text-white/40 hover:text-white/80"
            onclick={() => void removeMask(m.id)}
          >
            Delete
          </button>
        </div>
      {/each}
    {/if}
  </div>
</CollapsibleSection>
