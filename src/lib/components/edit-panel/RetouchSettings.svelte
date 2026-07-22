<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import Slider from "./Slider.svelte";
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

  async function refine(
    partial: { feather?: number; enabled?: boolean },
    live = false,
  ) {
    if (!$selectedRetouch) return;
    try {
      if (!live) status = "Rebuilding…";
      reconcile(
        await applyOp(
          { op: "refine_retouch_spot", id: $selectedRetouch, ...partial },
          live,
        ),
      );
    } catch (e) {
      status = String(e);
    }
  }
</script>

<CollapsibleSection id="retouchRemove" title="Object Removal">
  <div class="flex flex-col gap-[8px]">
    <p class="px-1 text-[9px] text-white/45">
      Paint a spot, release to heal (content-aware fill). Undo restores the clean master.
    </p>

    <button
      onclick={() => void addSpot()}
      class="h-[36px] rounded-[12px] border border-white/5 bg-white/[0.02] text-[10px] font-medium text-white/80 transition-all hover:bg-white/5"
    >
      New Heal Spot
    </button>

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

    {#if active}
      <div class="my-2 h-[1px] bg-white/5"></div>
      <div class="mb-1 px-1 text-[10px] font-bold text-white/50">Refine Spot</div>
      <div class="flex flex-col gap-[10px] px-1">
        <div class="grid h-[15px] grid-cols-[64px_1fr] items-center gap-x-[10px]">
          <span class="text-[9px] text-white/70">Feather</span>
          <Slider
            label="Feather"
            min={0}
            max={100}
            step={1}
            value={active.feather}
            resetValue={25}
            oninput={(v) => void refine({ feather: v }, true)}
            onchange={(v) => void refine({ feather: v }, false)}
          />
        </div>
        <label class="flex items-center gap-2 px-0 text-[9px] text-white/70">
          <input
            type="checkbox"
            checked={active.enabled}
            onchange={(e) =>
              void refine({
                enabled: (e.currentTarget as HTMLInputElement).checked,
              })}
          />
          Enabled
        </label>
      </div>
    {/if}

    <div class="my-2 h-[1px] bg-white/5"></div>
    <div class="mb-1 px-1 text-[10px] font-bold text-white/50">HEAL SPOTS</div>
    {#if spots.length === 0}
      <p class="px-1 text-[9px] text-white/35">No heal spots yet.</p>
    {:else}
      {#each spots as s (s.id)}
        <div
          class="flex items-center justify-between rounded-[12px] border p-2 {$selectedRetouch === s.id
            ? 'border-accent/40 bg-accent/10'
            : 'border-white/5 bg-white/[0.03]'}"
        >
          <button class="flex items-center gap-2 text-left" onclick={() => select(s.id)}>
            <div
              class="flex size-[18px] items-center justify-center rounded-full border border-accent/40 bg-accent/20 text-[9px] font-semibold text-accent"
            >
              H
            </div>
            <span class="text-[10px] text-white/80">
              Spot · {s.id.slice(0, 6)}{s.enabled ? "" : " (off)"}
            </span>
          </button>
          <button
            class="text-[9px] text-white/40 hover:text-white/80"
            onclick={() => void removeSpot(s.id)}
          >
            Delete
          </button>
        </div>
      {/each}
    {/if}

    {#if status}
      <p class="px-1 text-[8px] text-white/40">{status}</p>
    {/if}
  </div>
</CollapsibleSection>
