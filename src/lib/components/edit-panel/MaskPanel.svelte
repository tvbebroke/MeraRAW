<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { doc } from "../../../stores/doc";
  import { selectedMask } from "../../../stores/app";
  import { clearMaskPending, clearMaskError, maskDisplayName, maskErrors, maskPendingIds, setMaskError } from "../../../stores/mask";
  import { onMaskReady, onMaskError } from "../../../ipc/events";
  import MaskSettings from "./MaskSettings.svelte";
  import LightSettings from "./LightSettings.svelte";
  import ColorSettings from "./ColorSettings.svelte";
  import ToneCurve from "./ToneCurve.svelte";
  import DetailSettings from "./DetailSettings.svelte";
  import GradingSettings from "./GradingSettings.svelte";

  const masks = $derived($doc?.masks ?? []);
  const active = $derived(
    $selectedMask ? (masks.find((m) => m.id === $selectedMask) ?? null) : null,
  );
  const activeIndex = $derived(
    active ? masks.findIndex((m) => m.id === active.id) : -1,
  );
  const activePending = $derived(active ? $maskPendingIds.has(active.id) : false);
  const activeError = $derived(active ? ($maskErrors.get(active.id) ?? null) : null);

  onMount(() => {
    let unlistenReady: (() => void) | undefined;
    let unlistenErr: (() => void) | undefined;
    void onMaskReady((id) => {
      clearMaskPending(id);
      clearMaskError(id);
    }).then((fn) => {
      unlistenReady = fn;
    });
    void onMaskError(({ id, message }) => {
      clearMaskPending(id);
      setMaskError(id, message);
    }).then((fn) => {
      unlistenErr = fn;
    });
    return () => {
      unlistenReady?.();
      unlistenErr?.();
    };
  });
</script>

<div class="mask-panel custom-scrollbar">
  <MaskSettings embedded />

  {#if active}
    <div class="adjust-header" in:fade={{ duration: 120 }}>
      <p class="adjust-title">
        Adjust · {maskDisplayName(active.kind, activeIndex >= 0 ? activeIndex : 0)}
      </p>
      {#if activePending}
        <p class="adjust-pending">Detecting {active.kind === "sky" ? "sky" : "selection"}…</p>
      {:else if activeError}
        <p class="adjust-error">{activeError}</p>
      {/if}
      <p class="adjust-hint">Sliders below affect only the masked area.</p>
    </div>
    <LightSettings />
    <ColorSettings />
    <ToneCurve />
    <DetailSettings />
    <GradingSettings />
  {:else if masks.length > 0}
    <p class="rail-empty pick-hint">
      All masks are active. Click a mask above or the image to show its overlay.
    </p>
  {:else}
    <p class="rail-empty pick-hint">Create a mask with the tools above.</p>
  {/if}
</div>

<style>
  .mask-panel {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: var(--space-4);
  }
  .adjust-header {
    padding: var(--space-2) var(--space-3) 0;
    border-top: 1px solid var(--color-border);
    margin-top: var(--space-2);
  }
  .adjust-title {
    margin: 0;
    font-size: var(--text-ui);
    font-weight: 500;
    color: var(--color-fg);
  }
  .adjust-hint {
    margin: var(--space-1) 0 var(--space-2);
    font-size: 11px;
    color: var(--color-subtle);
  }
  .adjust-error {
    margin: 0 0 var(--space-1);
    font-size: 11px;
    color: #f87171;
  }
  .adjust-pending {
    margin: 0 0 var(--space-1);
    font-size: 11px;
    color: var(--color-accent, #7eb8ff);
    animation: pulse 1.4s ease-in-out infinite;
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 0.55;
    }
    50% {
      opacity: 1;
    }
  }
  .pick-hint {
    padding: var(--space-3);
    text-align: center;
  }
</style>
