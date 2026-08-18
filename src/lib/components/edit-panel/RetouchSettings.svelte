<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import ToggleSwitch from "../primitives/ToggleSwitch.svelte";
  import { applyOp } from "../../../ipc/commands";
  import { onRetouchDone, onRetouchError } from "../../../ipc/events";
  import { doc, reconcile } from "../../../stores/doc";
  import { selectedRetouch, selectedMask, viewportTool, brushRadius } from "../../../stores/app";
  import { onMount } from "svelte";

  const spots = $derived($doc?.retouch ?? []);
  const active = $derived(
    $selectedRetouch ? (spots.find((s) => s.id === $selectedRetouch) ?? null) : null,
  );

  let status = $state("");

  onMount(() => {
    let u1: (() => void) | undefined;
    let u2: (() => void) | undefined;
    void onRetouchDone(() => {
      status = "Healed";
    }).then((u) => {
      u1 = u;
    });
    void onRetouchError((msg) => {
      status = msg;
    }).then((u) => {
      u2 = u;
    });
    return () => {
      u1?.();
      u2?.();
    };
  });

  async function addSpot() {
    try {
      selectedMask.set(null);
      const delta = await applyOp({
        op: "add_retouch_spot",
        source: { type: "brush", strokes: [] },
      });
      reconcile(delta);
      if (delta.newRetouchId) {
        selectedRetouch.set(delta.newRetouchId);
        viewportTool.set("brush");
        status = "Paint over the object, then release";
      }
    } catch (e) {
      status = String(e);
    }
  }

  async function removeSpot(id: string) {
    try {
      status = "Rebuilding…";
      reconcile(await applyOp({ op: "remove_retouch_spot", id }));
      if ($selectedRetouch === id) selectedRetouch.set(null);
    } catch (e) {
      status = String(e);
    }
  }

  function select(id: string) {
    selectedMask.set(null);
    selectedRetouch.set(id);
    viewportTool.set("brush");
  }

  async function refine(partial: { feather?: number; enabled?: boolean }, live = false) {
    if (!$selectedRetouch) return;
    try {
      if (!live) status = "Rebuilding…";
      reconcile(
        await applyOp({ op: "refine_retouch_spot", id: $selectedRetouch, ...partial }, live),
      );
    } catch (e) {
      status = String(e);
    }
  }
</script>

<CollapsibleSection id="retouch" title="Retouch">
  <p class="rail-empty">Paint a spot, release to heal. Undo restores the master.</p>
  <button type="button" class="rail-btn full" onclick={() => void addSpot()}>New Heal Spot</button>

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
    <p class="group-label">Refine spot</p>
    <ParamRow
      label="Feather"
      min={0}
      max={100}
      step={1}
      value={active.feather}
      resetValue={25}
      oninput={(v) => void refine({ feather: v }, true)}
      onchange={(v) => void refine({ feather: v }, false)}
    />
    <div class="control-row">
      <span>Enabled</span>
      <ToggleSwitch
        checked={active.enabled}
        label="Spot enabled"
        onchange={(v) => void refine({ enabled: v })}
      />
    </div>
  {/if}

  <p class="group-label">Heal spots</p>
  {#if spots.length === 0}
    <p class="rail-empty">No heal spots yet.</p>
  {:else}
    {#each spots as s (s.id)}
      <div class="item" class:on={$selectedRetouch === s.id}>
        <button type="button" class="pick" onclick={() => select(s.id)}>
          Spot · {s.id.slice(0, 6)}{s.enabled ? "" : " (off)"}
        </button>
        <button type="button" class="del" onclick={() => void removeSpot(s.id)}>Delete</button>
      </div>
    {/each}
  {/if}

  {#if status}
    <p class="rail-empty">{status}</p>
  {/if}
</CollapsibleSection>

<style>
  .full { width: 100%; margin: var(--space-2) 0; }
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
