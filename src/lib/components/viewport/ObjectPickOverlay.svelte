<script lang="ts">
  import { proposeObjectMasks, type ObjectProposal } from "../../../ipc/commands";
  import { objectPickActive } from "../../../stores/mask";

  interface Props {
    imgBox: { left: number; top: number; width: number; height: number } | null;
    imageNormToLocal: (nx: number, ny: number) => { x: number; y: number } | null;
    onPick: (centroid: [number, number]) => void;
  }

  let { imgBox, imageNormToLocal, onPick }: Props = $props();

  let proposals = $state<ObjectProposal[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let hoverId = $state<number | null>(null);
  let gen = 0;

  async function load() {
    const g = ++gen;
    loading = true;
    error = null;
    proposals = [];
    try {
      const next = await proposeObjectMasks();
      if (g !== gen) return;
      proposals = next ?? [];
      if (proposals.length === 0) {
        error = "No outlines found — click the subject to mask.";
      }
    } catch (e) {
      if (g !== gen) return;
      error = e instanceof Error ? e.message : "Detection failed";
      proposals = [];
    } finally {
      if (g === gen) loading = false;
    }
  }

  $effect(() => {
    if ($objectPickActive) {
      void load();
    } else {
      gen++;
      proposals = [];
      hoverId = null;
      loading = false;
      error = null;
    }
  });

  function toSvgPath(p: ObjectProposal): string {
    if (!imgBox) return "";
    const pts: string[] = [];
    for (const [nx, ny] of p.path) {
      const loc = imageNormToLocal(nx, ny);
      if (!loc) continue;
      pts.push(`${loc.x - imgBox.left},${loc.y - imgBox.top}`);
    }
    if (pts.length < 3) return "";
    return `M ${pts.join(" L ")} Z`;
  }

  function pickProposal(p: ObjectProposal, e: MouseEvent) {
    e.stopPropagation();
    onPick(p.centroid);
  }
</script>

{#if $objectPickActive && imgBox}
  <div
    class="pick-layer"
    style="left:{imgBox.left}px; top:{imgBox.top}px; width:{imgBox.width}px; height:{imgBox.height}px;"
  >
    <svg class="pick-svg" width={imgBox.width} height={imgBox.height}>
      {#each proposals as p (p.id)}
        {@const d = toSvgPath(p)}
        {#if d}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <path
            class="outline"
            class:hover={hoverId === p.id}
            d={d}
            onmouseenter={() => (hoverId = p.id)}
            onmouseleave={() => {
              if (hoverId === p.id) hoverId = null;
            }}
            onclick={(e) => pickProposal(p, e)}
          />
        {/if}
      {/each}
    </svg>
    <div class="pick-hint">
      {#if loading}
        Finding objects…
      {:else if error}
        {error}
      {:else}
        Click a dotted outline to mask that object
      {/if}
    </div>
  </div>
{/if}

<style>
  .pick-layer {
    position: absolute;
    z-index: 12;
    pointer-events: none;
  }
  .pick-svg {
    position: absolute;
    inset: 0;
    overflow: visible;
  }
  .outline {
    fill: rgba(255, 72, 72, 0.1);
    stroke: #ff4d4d;
    stroke-width: 1.75;
    stroke-dasharray: 5 4;
    stroke-linejoin: round;
    stroke-linecap: round;
    vector-effect: non-scaling-stroke;
    animation: dash 0.85s linear infinite;
    pointer-events: fill;
    cursor: pointer;
  }
  .outline.hover {
    fill: rgba(255, 72, 72, 0.26);
    stroke: #ff9a9a;
    stroke-width: 2.35;
  }
  @keyframes dash {
    to {
      stroke-dashoffset: -18;
    }
  }
  .pick-hint {
    position: absolute;
    left: 50%;
    bottom: 10px;
    transform: translateX(-50%);
    padding: 5px 10px;
    border-radius: 6px;
    background: rgba(12, 12, 14, 0.72);
    color: #f2f2f4;
    font-size: 11px;
    white-space: nowrap;
    pointer-events: none;
  }
</style>
