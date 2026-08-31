<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import ParamRow from "./ParamRow.svelte";
  import ToggleSwitch from "../primitives/ToggleSwitch.svelte";
  import { applyOp } from "../../../ipc/commands";
  import { doc, reconcile } from "../../../stores/doc";
  import { selectedMask, selectedRetouch, viewportTool, brushRadius } from "../../../stores/app";
  import {
    brushFlow,
    brushHardness,
    markMaskPending,
    maskDisplayName,
    maskErrors,
    maskOverlayVisible,
    maskPendingIds,
    maskRefineMode,
    maskToolGroup,
    objectPickActive,
    deselectMask,
    syncMaskOverlay,
    syncViewportToolForMask,
    type MaskRefineMode,
  } from "../../../stores/mask";
  import { showMaskPanel } from "../../editor/focus";

  let { embedded = false }: { embedded?: boolean } = $props();

  const masks = $derived($doc?.masks ?? []);
  const active = $derived(
    $selectedMask ? (masks.find((m) => m.id === $selectedMask) ?? null) : null,
  );
  const activeIndex = $derived(
    active ? masks.findIndex((m) => m.id === active.id) : -1,
  );

  type MaskKind =
    | "brush"
    | "linear"
    | "radial"
    | "subject"
    | "sky"
    | "background"
    | "object"
    | "parametric";

  const KIND_ICON: Record<string, string> = {
    subject: "◎",
    sky: "☁",
    background: "◫",
    object: "◉",
    brush: "◔",
    linear: "▥",
    radial: "◯",
    parametric: "◐",
  };

  function maskEnabled(m: (typeof masks)[number]): boolean {
    return m.enabled !== false;
  }

  function defaultSource(kind: MaskKind): Record<string, unknown> {
    switch (kind) {
      case "brush":
        return { type: "brush", strokes: [] };
      case "linear":
        return { type: "linear", start: [0.5, 0.2], end: [0.5, 0.8] };
      case "radial":
        return { type: "radial", center: [0.5, 0.5], radii: [0.28, 0.28], rotation: 0 };
      case "parametric":
        return {
          type: "parametric",
          luma_lo: 0.35,
          luma_hi: 0.85,
          chroma_lo: 0,
          chroma_hi: 1,
          hue_lo: 0,
          hue_hi: 360,
          softness: 0.08,
        };
      case "sky":
        return { type: "segmented", model: "sky_v1", hint: null };
      case "object":
        return {
          type: "segmented",
          model: "object_v1",
          hint: { point: [0.5, 0.5] },
        };
      default:
        return { type: "segmented", model: "subject_v1", hint: null };
    }
  }

  async function addMask(kind: MaskKind) {
    const source = defaultSource(kind);
    try {
      const delta = await applyOp({ op: "add_mask", kind, source });
      reconcile(delta);
      if (delta.newMaskId) {
        selectedRetouch.set(null);
        selectedMask.set(delta.newMaskId);
        maskToolGroup.set(null);
        const newMask = delta.doc.masks?.find((m) => m.id === delta.newMaskId);
        syncViewportToolForMask(newMask ?? null);
        if (source.type === "segmented") markMaskPending(delta.newMaskId);
        maskOverlayVisible.set(true);
        showMaskPanel();
        syncMaskOverlay();
      }
    } catch {
      /* ignore */
    }
  }

  async function addComponent(kind: MaskKind, mode: MaskRefineMode = $maskRefineMode) {
    if (!active) {
      await addMask(kind);
      return;
    }
    const source = defaultSource(kind);
    try {
      reconcile(
        await applyOp({
          op: "add_mask_component",
          id: active.id,
          mode,
          source,
        }),
      );
      syncViewportToolForMask(active);
      if (source.type === "segmented") markMaskPending(active.id);
      showMaskPanel();
      syncMaskOverlay();
    } catch {
      /* ignore */
    }
  }

  async function removeMask(id: string) {
    try {
      reconcile(await applyOp({ op: "remove_mask", id }));
      if ($selectedMask === id) {
        selectedMask.set(null);
        syncMaskOverlay();
      }
    } catch {
      /* ignore */
    }
  }

  async function duplicateMask(id: string) {
    try {
      const delta = await applyOp({ op: "duplicate_mask", id });
      reconcile(delta);
      if (delta.newMaskId) {
        selectedRetouch.set(null);
        selectedMask.set(delta.newMaskId);
        const dup = delta.doc.masks?.find((m) => m.id === delta.newMaskId);
        syncViewportToolForMask(dup ?? null);
        if (dup?.source?.type === "segmented") markMaskPending(delta.newMaskId);
        maskOverlayVisible.set(true);
        showMaskPanel();
        syncMaskOverlay();
      }
    } catch {
      /* ignore */
    }
  }

  async function toggleMaskEnabled(m: (typeof masks)[number]) {
    try {
      reconcile(
        await applyOp({
          op: "refine_mask",
          id: m.id,
          enabled: !maskEnabled(m),
        }),
      );
      syncMaskOverlay();
    } catch {
      /* ignore */
    }
  }

  function startObjectPick() {
    objectPickActive.set(true);
    showMaskPanel();
  }

  function select(id: string) {
    objectPickActive.set(false);
    if ($selectedMask === id) {
      deselectMask();
      return;
    }
    selectedRetouch.set(null);
    selectedMask.set(id);
    const m = masks.find((x) => x.id === id);
    syncViewportToolForMask(m ?? null);
    maskOverlayVisible.set(true);
    showMaskPanel();
    syncMaskOverlay();
  }

  async function refine(
    partial: { opacity?: number; feather?: number; invert?: boolean; blend?: string },
    live = false,
  ) {
    if (!$selectedMask) return;
    try {
      reconcile(await applyOp({ op: "refine_mask", id: $selectedMask, ...partial }, live));
    } catch {
      /* ignore */
    }
  }

  async function setParametric(key: string, value: number, live = false) {
    if (!active) return;
    const src = parametricSource(active);
    const source = { ...src, type: "parametric", [key]: value };
    try {
      reconcile(await applyOp({ op: "set_mask_source", id: active.id, source }, live));
    } catch {
      /* ignore */
    }
  }

  function parametricSource(m: (typeof masks)[number]): Record<string, unknown> {
    if (m.source?.type === "parametric") return m.source as Record<string, unknown>;
    if (m.source?.type === "composite") {
      const comps = m.source.components as { source?: Record<string, unknown> }[] | undefined;
      const p = comps?.find((c) => c.source?.type === "parametric");
      if (p?.source) return p.source;
    }
    return defaultSource("parametric");
  }

  function isParametric(m: (typeof masks)[number]): boolean {
    return (
      m.kind === "parametric" ||
      m.source?.type === "parametric" ||
      Boolean(
        m.source?.type === "composite" &&
          (m.source.components as { source?: { type?: string } }[] | undefined)?.some(
            (c) => c.source?.type === "parametric",
          ),
      )
    );
  }

  function num(source: Record<string, unknown>, key: string, fallback: number) {
    const v = source[key];
    return typeof v === "number" ? v : fallback;
  }

  function toggleOverlay() {
    maskOverlayVisible.set(!maskOverlayVisible.get());
    syncMaskOverlay();
  }

  function setRefineMode(mode: MaskRefineMode) {
    maskRefineMode.set(mode);
    if (active && active.kind !== "brush") {
      viewportTool.set("brush");
    }
  }
</script>

{#snippet maskBody()}
  <div class="mask-toolbar">
    <button
      type="button"
      class="overlay-btn"
      class:on={$maskOverlayVisible}
      onclick={toggleOverlay}
      title="Show mask overlay"
    >
      Overlay
    </button>
  </div>

  {#if active}
    <div class="refine-bar">
      <span class="group-label">Refine mask</span>
      <div class="seg seg-3">
        <button
          type="button"
          class="seg-btn"
          class:on={$maskRefineMode === "add"}
          onclick={() => setRefineMode("add")}
        >Add</button>
        <button
          type="button"
          class="seg-btn"
          class:on={$maskRefineMode === "subtract"}
          onclick={() => setRefineMode("subtract")}
        >Subtract</button>
        <button
          type="button"
          class="seg-btn"
          class:on={$maskRefineMode === "intersect"}
          onclick={() => setRefineMode("intersect")}
        >Intersect</button>
      </div>
    </div>
  {/if}

  {#if $objectPickActive}
    <p class="pick-banner">Click the subject on the image to create an object mask.</p>
  {/if}

  <p class="group-label">Add new mask</p>

  <div class="tool-section">
    <button
      type="button"
      class="section-head"
      onclick={() => maskToolGroup.set($maskToolGroup === "ai" ? null : "ai")}
    >
      <span>AI Select</span>
      <span class="chev" class:open={$maskToolGroup === "ai"}>›</span>
    </button>
    {#if $maskToolGroup === "ai"}
      <div class="tool-grid ai">
        <button type="button" class="tool-btn" onclick={() => void addMask("subject")}>
          <span class="ico">◎</span> Subject
        </button>
        <button type="button" class="tool-btn" onclick={() => void addMask("sky")}>
          <span class="ico">☁</span> Sky
        </button>
        <button type="button" class="tool-btn" onclick={() => void addMask("background")}>
          <span class="ico">◫</span> Background
        </button>
        <button type="button" class="tool-btn" onclick={startObjectPick}>
          <span class="ico">◉</span> Object · click image
        </button>
      </div>
    {/if}
  </div>

  <div class="tool-section">
    <button
      type="button"
      class="section-head"
      onclick={() => maskToolGroup.set($maskToolGroup === "manual" ? null : "manual")}
    >
      <span>Manual</span>
      <span class="chev" class:open={$maskToolGroup === "manual"}>›</span>
    </button>
    {#if $maskToolGroup === "manual"}
      <div class="tool-grid">
        <button
          type="button"
          class="tool-btn"
          onclick={() => void (active ? addComponent("brush") : addMask("brush"))}
        >
          <span class="ico">🖌</span> Brush
        </button>
        <button
          type="button"
          class="tool-btn"
          onclick={() => void (active ? addComponent("linear") : addMask("linear"))}
        >
          <span class="ico">▥</span> Linear
        </button>
        <button
          type="button"
          class="tool-btn"
          onclick={() => void (active ? addComponent("radial") : addMask("radial"))}
        >
          <span class="ico">◯</span> Radial
        </button>
      </div>
    {/if}
  </div>

  <div class="tool-section">
    <button
      type="button"
      class="section-head"
      onclick={() => maskToolGroup.set($maskToolGroup === "range" ? null : "range")}
    >
      <span>Range</span>
      <span class="chev" class:open={$maskToolGroup === "range"}>›</span>
    </button>
    {#if $maskToolGroup === "range"}
      <div class="tool-grid">
        <button
          type="button"
          class="tool-btn wide"
          onclick={() => void (active ? addComponent("parametric") : addMask("parametric"))}
        >
          <span class="ico">◐</span> Luminance / Color Range
        </button>
      </div>
    {/if}
  </div>

  {#if active}
    <ParamRow
      label="Brush size"
      min={0.01}
      max={0.25}
      step={0.005}
      value={$brushRadius}
      resetValue={0.05}
      oninput={(v) => brushRadius.set(v)}
      onchange={(v) => brushRadius.set(v)}
    />
    <ParamRow
      label="Hardness"
      min={0}
      max={1}
      step={0.05}
      value={$brushHardness}
      resetValue={0.6}
      oninput={(v) => brushHardness.set(v)}
      onchange={(v) => brushHardness.set(v)}
    />
    <ParamRow
      label="Flow"
      min={0.05}
      max={1}
      step={0.05}
      value={$brushFlow}
      resetValue={1}
      oninput={(v) => brushFlow.set(v)}
      onchange={(v) => brushFlow.set(v)}
    />

    <p class="group-label">Selected mask</p>
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
      <span>Invert</span>
      <ToggleSwitch
        checked={active.invert}
        label="Invert mask"
        onchange={(v) => void refine({ invert: v })}
      />
    </div>

    {#if isParametric(active)}
      {@const ps = parametricSource(active)}
      <p class="group-label">Luminance range</p>
      <ParamRow
        label="Luma min"
        min={0}
        max={1}
        step={0.01}
        value={num(ps, "luma_lo", 0)}
        resetValue={0}
        oninput={(v) => void setParametric("luma_lo", v, true)}
        onchange={(v) => void setParametric("luma_lo", v, false)}
      />
      <ParamRow
        label="Luma max"
        min={0}
        max={1}
        step={0.01}
        value={num(ps, "luma_hi", 1)}
        resetValue={1}
        oninput={(v) => void setParametric("luma_hi", v, true)}
        onchange={(v) => void setParametric("luma_hi", v, false)}
      />
      <ParamRow
        label="Softness"
        min={0}
        max={0.3}
        step={0.005}
        value={num(ps, "softness", 0.05)}
        resetValue={0.05}
        oninput={(v) => void setParametric("softness", v, true)}
        onchange={(v) => void setParametric("softness", v, false)}
      />
      <p class="group-label">Color range</p>
      <ParamRow
        label="Chroma min"
        min={0}
        max={1}
        step={0.01}
        value={num(ps, "chroma_lo", 0)}
        resetValue={0}
        oninput={(v) => void setParametric("chroma_lo", v, true)}
        onchange={(v) => void setParametric("chroma_lo", v, false)}
      />
      <ParamRow
        label="Chroma max"
        min={0}
        max={1}
        step={0.01}
        value={num(ps, "chroma_hi", 1)}
        resetValue={1}
        oninput={(v) => void setParametric("chroma_hi", v, true)}
        onchange={(v) => void setParametric("chroma_hi", v, false)}
      />
      <ParamRow
        label="Hue min"
        min={0}
        max={360}
        step={1}
        value={num(ps, "hue_lo", 0)}
        resetValue={0}
        oninput={(v) => void setParametric("hue_lo", v, true)}
        onchange={(v) => void setParametric("hue_lo", v, false)}
      />
      <ParamRow
        label="Hue max"
        min={0}
        max={360}
        step={1}
        value={num(ps, "hue_hi", 360)}
        resetValue={360}
        oninput={(v) => void setParametric("hue_hi", v, true)}
        onchange={(v) => void setParametric("hue_hi", v, false)}
      />
    {/if}
  {/if}

  <p class="group-label">Masks ({masks.length})</p>
  {#if masks.length === 0}
    <p class="rail-empty">Select a tool above to create a mask.</p>
  {:else}
    {#each masks as m, i (m.id)}
      {@const pending = $maskPendingIds.has(m.id)}
      {@const err = $maskErrors.get(m.id)}
      {@const on = $selectedMask === m.id}
      <div class="mask-row" class:on class:disabled={!maskEnabled(m)}>
        <button
          type="button"
          class="mask-enable"
          aria-label={maskEnabled(m) ? "Disable mask" : "Enable mask"}
          onclick={() => void toggleMaskEnabled(m)}
        >
          <span class="enable-dot" class:off={!maskEnabled(m)}></span>
        </button>
        <button type="button" class="mask-pick" onclick={() => select(m.id)}>
          <span class="mask-name">
            <span class="kind-ico">{KIND_ICON[m.kind] ?? "◆"}</span>
            {maskDisplayName(m.kind, i)}
          </span>
          <span class="mask-meta">
            {#if pending}
              <span class="meta-pending">Detecting…</span>
            {:else if err}
              <span class="meta-error">Failed</span>
            {:else if !maskEnabled(m)}
              <span class="meta-off">Disabled</span>
            {:else}
              {Math.round(m.opacity)}% · {m.feather > 0 ? `feather ${Math.round(m.feather)}` : "sharp"}
            {/if}
          </span>
        </button>
        <button
          type="button"
          class="mask-act"
          title="Duplicate"
          aria-label="Duplicate mask"
          onclick={() => void duplicateMask(m.id)}
        >⧉</button>
        <button type="button" class="mask-del" aria-label="Delete mask" onclick={() => void removeMask(m.id)}>×</button>
      </div>
    {/each}
  {/if}
{/snippet}

{#if embedded}
  <div class="mask-embedded">
    {@render maskBody()}
  </div>
{:else}
  <CollapsibleSection id="mask" title="Masking">
    {@render maskBody()}
  </CollapsibleSection>
{/if}

<style>
  .mask-embedded {
    padding-top: var(--space-1);
  }
  .mask-toolbar {
    display: flex;
    justify-content: flex-end;
    padding: 0 var(--space-1) var(--space-2);
  }
  .overlay-btn {
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-hover);
    color: var(--color-subtle);
    font-size: var(--text-ui);
    padding: 4px 10px;
    cursor: pointer;
    transition: border-color 0.15s, color 0.15s, background 0.15s;
  }
  .overlay-btn.on {
    border-color: var(--color-accent);
    color: var(--color-fg);
    background: var(--color-accent-soft);
  }
  .refine-bar {
    margin-bottom: var(--space-2);
  }
  .seg {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-1);
  }
  .seg.seg-3 {
    grid-template-columns: 1fr 1fr 1fr;
  }
  .pick-banner {
    margin: 0 0 var(--space-2);
    padding: var(--space-2);
    border-radius: 8px;
    border: 1px solid var(--color-accent);
    background: var(--color-accent-soft);
    color: var(--color-fg);
    font-size: 11px;
    text-align: center;
  }
  .seg-btn {
    height: 28px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-hover);
    color: var(--color-subtle);
    font-size: var(--text-ui);
    cursor: pointer;
    transition: all 0.15s var(--ease-std);
  }
  .seg-btn.on {
    border-color: var(--color-accent);
    color: var(--color-fg);
    background: var(--color-accent-soft);
  }
  .tool-section {
    margin-bottom: var(--space-2);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    overflow: hidden;
    background: var(--color-hover);
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    min-height: 32px;
    padding: 0 var(--space-2);
    border: 0;
    background: transparent;
    color: var(--color-fg);
    font-size: var(--text-ui);
    cursor: pointer;
  }
  .section-head:hover {
    background: var(--color-active);
  }
  .chev {
    color: var(--color-subtle);
    transform: rotate(0deg);
    transition: transform 0.15s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .tool-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-1);
    padding: var(--space-2);
    border-top: 1px solid var(--color-border);
  }
  .tool-grid.ai {
    grid-template-columns: 1fr;
  }
  .tool-btn {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 34px;
    padding: 0 var(--space-2);
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-sidebar);
    color: var(--color-fg);
    font-size: var(--text-ui);
    cursor: pointer;
    transition: border-color 0.15s, background 0.15s;
  }
  .tool-btn:hover {
    border-color: var(--color-accent);
    background: var(--color-accent-soft);
  }
  .tool-btn.wide {
    grid-column: 1 / -1;
  }
  .ico {
    opacity: 0.85;
    width: 1.1em;
    text-align: center;
  }
  .mask-row {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    min-height: 34px;
    padding: 2px;
    border: 1px solid var(--color-border);
    border-radius: 8px;
    background: var(--color-hover);
    margin-bottom: var(--space-1);
    transition: border-color 0.15s, background 0.15s;
  }
  .mask-row.on {
    border-color: var(--color-accent);
    background: var(--color-accent-soft);
  }
  .mask-row.disabled {
    opacity: 0.55;
  }
  .mask-enable {
    border: 0;
    background: none;
    cursor: pointer;
    padding: 0 var(--space-1);
    display: flex;
    align-items: center;
  }
  .enable-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--color-accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-accent) 35%, transparent);
  }
  .enable-dot.off {
    background: var(--color-subtle);
    box-shadow: none;
  }
  .mask-pick {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    border: 0;
    background: none;
    color: var(--color-fg);
    font-size: var(--text-ui);
    cursor: pointer;
    padding: var(--space-1) var(--space-1);
    text-align: left;
    min-width: 0;
  }
  .mask-name {
    font-weight: 500;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .kind-ico {
    opacity: 0.75;
    font-size: 12px;
  }
  .mask-meta {
    color: var(--color-subtle);
    font-size: 10px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta-pending {
    color: var(--color-accent, #7eb8ff);
  }
  .meta-error {
    color: #f87171;
  }
  .meta-off {
    color: var(--color-subtle);
  }
  .mask-act {
    border: 0;
    background: none;
    color: var(--color-subtle);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    padding: 0 var(--space-1);
  }
  .mask-act:hover {
    color: var(--color-fg);
  }
  .mask-del {
    border: 0;
    background: none;
    color: var(--color-subtle);
    font-size: 18px;
    line-height: 1;
    cursor: pointer;
    padding: 0 var(--space-2);
  }
  .mask-del:hover {
    color: var(--color-fg);
  }
</style>
