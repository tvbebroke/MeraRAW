<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import ToggleSwitch from "../primitives/ToggleSwitch.svelte";
  import { applyOp, setMaskOverlay } from "../../../ipc/commands";
  import { doc, reconcile } from "../../../stores/doc";
  import { selectedMask, selectedRetouch, viewportTool, brushRadius } from "../../../stores/app";

  const masks = $derived($doc?.masks ?? []);
  const active = $derived(
    $selectedMask ? (masks.find((m) => m.id === $selectedMask) ?? null) : null,
  );

  type MaskKind = "brush" | "linear" | "radial" | "subject" | "sky" | "background";

  async function addMask(kind: MaskKind) {
    const source =
      kind === "brush"
        ? { type: "brush", strokes: [] }
        : kind === "linear"
          ? { type: "linear", x0: 0.5, y0: 0.2, x1: 0.5, y1: 0.8 }
          : kind === "radial"
            ? { type: "radial", cx: 0.5, cy: 0.5, rx: 0.3, ry: 0.3 }
            : kind === "sky"
              ? { type: "segmented", model: "sky_v1", hint: null }
              : { type: "segmented", model: "subject_v1", hint: null };
    try {
      const delta = await applyOp({ op: "add_mask", kind, source });
      reconcile(delta);
      if (delta.newMaskId) {
        selectedRetouch.set(null);
        selectedMask.set(delta.newMaskId);
        viewportTool.set(kind === "brush" ? "brush" : "pan");
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
    selectedRetouch.set(null);
    selectedMask.set(id);
    const m = masks.find((x) => x.id === id);
    viewportTool.set(m?.kind === "brush" ? "brush" : "pan");
    void setMaskOverlay(id);
  }

  async function refine(
    partial: { opacity?: number; feather?: number; invert?: boolean },
    live = false,
  ) {
    if (!$selectedMask) return;
    try {
      reconcile(await applyOp({ op: "refine_mask", id: $selectedMask, ...partial }, live));
    } catch {
      /* ignore */
    }
  }
</script>

<CollapsibleSection id="mask" title="Mask">
  <p class="group-label">Create</p>
  <div class="grid2">
    <button type="button" class="rail-btn" onclick={() => void addMask("brush")}>Brush</button>
    <button type="button" class="rail-btn" onclick={() => void addMask("linear")}>Linear</button>
    <button type="button" class="rail-btn" onclick={() => void addMask("radial")}>Radial</button>
    <button type="button" class="rail-btn" onclick={() => void addMask("subject")}>Subject</button>
    <button type="button" class="rail-btn" onclick={() => void addMask("sky")}>Sky</button>
    <button type="button" class="rail-btn" onclick={() => void addMask("background")}>Background</button>
  </div>

  <ParamRow
    label="Brush"
    min={0.01}
    max={0.25}
    step={0.005}
    value={$brushRadius}
    resetValue={0.05}
    oninput={(v) => brushRadius.set(v)}
    onchange={(v) => brushRadius.set(v)}
  />

  {#if active}
    <p class="group-label">Refine selected</p>
    <ParamRow
      label="Opacity"
      min={0}
      max={100}
      step={1}
      value={active.opacity}
      resetValue={100}
      oninput={(v) => void refine({ opacity: v }, true)}
      onchange={(v) => void refine({ opacity: v }, false)}
    />
    <ParamRow
      label="Feather"
      min={0}
      max={100}
      step={1}
      value={active.feather}
      resetValue={0}
      oninput={(v) => void refine({ feather: v }, true)}
      onchange={(v) => void refine({ feather: v }, false)}
    />
    <div class="control-row">
      <span>Invert mask</span>
      <ToggleSwitch
        checked={active.invert}
        label="Invert mask"
        onchange={(v) => void refine({ invert: v })}
      />
    </div>
  {/if}

  <p class="group-label">Active masks</p>
  {#if masks.length === 0}
    <p class="rail-empty">No masks yet.</p>
  {:else}
    {#each masks as m (m.id)}
      <div class="item" class:on={$selectedMask === m.id}>
        <button type="button" class="pick" onclick={() => select(m.id)}>
          {m.kind ?? "Mask"} · {m.id.slice(0, 6)}
        </button>
        <button type="button" class="del" onclick={() => void removeMask(m.id)}>Delete</button>
      </div>
    {/each}
  {/if}
</CollapsibleSection>

<style>
  .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-2); }
  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    min-height: 28px;
    padding: 0 var(--space-2);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-hover);
    margin-bottom: var(--space-1);
  }
  .item.on {
    border-color: var(--color-accent);
    background: var(--color-accent-soft);
  }
  .pick {
    border: 0;
    background: none;
    color: var(--color-fg);
    font-size: var(--text-ui);
    cursor: pointer;
    text-align: left;
    flex: 1;
  }
  .del {
    border: 0;
    background: none;
    color: var(--color-subtle);
    font-size: var(--text-ui);
    cursor: pointer;
  }
  .del:hover { color: var(--color-fg); }
</style>
