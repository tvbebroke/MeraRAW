<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import {
    needsConsentPrompt,
    setTelemetryConsent,
  } from "../../../analytics/telemetry";

  function choose(granted: boolean) {
    setTelemetryConsent(granted);
  }

  // Escape = decline. Dismissing without an explicit "yes" must never be
  // read as consent.
  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") choose(false);
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

{#if $needsConsentPrompt}
  <!-- No backdrop click-to-dismiss: this is a deliberate choice, not a modal
       to swat away. -->
  <div
    transition:fade={{ duration: 200 }}
    class="absolute inset-0 z-[200] flex items-center justify-center bg-black/70 px-4 py-6"
  >
    <div
      transition:scale={{ duration: 250, start: 0.95 }}
      class="w-[460px] overflow-hidden rounded-[10px] border border-border-strong bg-panel p-7 text-fg backdrop-blur-md"
      role="dialog"
      aria-modal="true"
      aria-labelledby="telemetry-title"
    >
      <h2 id="telemetry-title" class="text-[15px] font-semibold text-fg">
        Help improve MeraRAW?
      </h2>

      <p class="mt-3 text-[12px] leading-relaxed text-secondary">
        MeraRAW can send anonymous usage data — which tools and export formats
        get used, how long renders take, and which camera models hit decode
        errors. It helps us find crashes and prioritise what to build.
      </p>

      <div class="mt-4 rounded-[8px] border border-border bg-black/20 p-3">
        <p class="text-[11px] font-semibold text-secondary">Never collected</p>
        <p class="mt-1.5 text-[11px] leading-relaxed text-subtle">
          Your photos, filenames, folder paths, presets, or anything you type.
          No account is created and no personal profile is built.
        </p>
      </div>

      <p class="mt-3 text-[11px] text-subtle">
        You can change this any time in Settings → General.
      </p>

      <div class="mt-6 flex justify-end gap-2">
        <button class="consent-btn" onclick={() => choose(false)}>
          No thanks
        </button>
        <button class="consent-btn consent-btn--primary" onclick={() => choose(true)}>
          Share usage data
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .consent-btn {
    border-radius: 8px;
    border: 1px solid var(--color-border-strong);
    background: var(--color-hover);
    padding: 8px 16px;
    font-size: 12px;
    font-weight: 500;
    color: var(--color-secondary);
    cursor: pointer;
    transition: background 150ms ease, color 150ms ease;
  }

  .consent-btn:hover {
    background: var(--color-active);
    color: var(--color-fg);
  }

  .consent-btn--primary {
    border-color: var(--color-border-strong);
    background: var(--color-fg);
    color: #111113;
  }

  .consent-btn--primary:hover {
    background: var(--color-fg);
    color: #111113;
  }
</style>
