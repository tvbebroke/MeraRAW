<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { isBugReportOpen } from "../../../stores/ui";
  import ToggleSwitch from "../primitives/ToggleSwitch.svelte";

  // Local placeholder state — no submission pipeline yet
  let title = $state("");
  let description = $state("");
  let severity = $state("minor");
  let attachDiagnostics = $state(true);

  function close() {
    isBugReportOpen.set(false);
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  transition:fade={{ duration: 200 }}
  class="backdrop absolute inset-0 z-[100] flex items-center justify-center bg-black/60 px-4 py-6"
  onclick={close}
>
  <div
    transition:scale={{ duration: 250, start: 0.95 }}
    class="modal-card relative flex w-[420px] flex-col overflow-hidden rounded-[20px] border border-white/[0.06] bg-[#171717]/95 text-white backdrop-blur-md"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="flex-1 overflow-y-auto px-8 pt-7 pb-6">
      <h2 class="content-title">Report a bug</h2>
      <div class="settings-list">
        <div class="setting-row setting-row--stacked">
          <label for="bug-title" class="setting-label">Title</label>
          <input
            id="bug-title"
            type="text"
            bind:value={title}
            placeholder="Short summary of the issue"
            class="setting-input w-full"
          />
        </div>

        <div class="setting-row setting-row--stacked">
          <label for="bug-description" class="setting-label">Description</label>
          <textarea
            id="bug-description"
            rows="4"
            bind:value={description}
            placeholder="What happened? What did you expect instead?"
            class="setting-input w-full resize-none"
          ></textarea>
        </div>

        <div class="setting-row">
          <span class="setting-label">Severity</span>
          <select id="bug-severity" bind:value={severity} class="setting-select">
            <option value="minor">Minor</option>
            <option value="moderate">Moderate</option>
            <option value="severe">Severe</option>
            <option value="crash">Crash</option>
          </select>
        </div>

        <div class="setting-row">
          <span class="setting-label">Attach system info</span>
          <ToggleSwitch checked={attachDiagnostics} label="Attach Diagnostics" onchange={(v) => attachDiagnostics = v} />
        </div>
      </div>
    </div>

    <!-- ponytail: no submission pipeline yet, this is a UI placeholder -->
    <footer class="flex shrink-0 items-center justify-end gap-3 border-t border-white/[0.04] px-8 py-4">
      <button class="footer-btn footer-btn--ghost" onclick={close}>Cancel</button>
      <button class="footer-btn footer-btn--primary" onclick={close}>Submit report</button>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    background: rgba(0, 0, 0, 0.35);
    -webkit-backdrop-filter: blur(16px) saturate(120%);
    backdrop-filter: blur(16px) saturate(120%);
  }

  .modal-card {
    background: rgba(23, 23, 23, 0.96);
    box-shadow: 
      0 0 0 0.5px rgba(255, 255, 255, 0.06),
      0 8px 24px rgba(0, 0, 0, 0.3),
      0 24px 48px rgba(0, 0, 0, 0.25);
  }

  /* ── Content Title ──────────────────────────────────── */
  .content-title {
    font-size: 15px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
    letter-spacing: -0.01em;
    margin-bottom: 8px;
  }

  /* ── Settings List ──────────────────────────────────── */
  .settings-list {
    display: flex;
    flex-direction: column;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    transition: background-color 150ms ease;
  }

  .setting-row:last-child {
    border-bottom: none;
  }

  .setting-row--stacked {
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
  }

  .setting-label {
    font-size: 13px;
    font-weight: 400;
    color: rgba(255, 255, 255, 0.75);
    letter-spacing: -0.005em;
  }

  /* ── Form Controls ──────────────────────────────────── */
  .setting-input {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(255, 255, 255, 0.03);
    border-radius: 8px;
    padding: 7px 12px;
    font-size: 12px;
    font-family: inherit;
    color: rgba(255, 255, 255, 0.6);
    outline: none;
    transition: border-color 150ms ease, background-color 150ms ease;
  }

  .setting-input:focus {
    border-color: rgba(255, 255, 255, 0.15);
    background: rgba(255, 255, 255, 0.04);
  }

  .setting-select {
    appearance: none;
    -webkit-appearance: none;
    border: none;
    background: transparent url("data:image/svg+xml,%3Csvg width='8' height='5' viewBox='0 0 8 5' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1L4 4L7 1' stroke='rgba%28255,255,255,0.3%29' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E") no-repeat right 0 center;
    background-size: 8px 5px;
    padding: 4px 20px 4px 0;
    font-size: 13px;
    font-family: inherit;
    font-weight: 400;
    color: rgba(255, 255, 255, 0.5);
    outline: none;
    cursor: pointer;
    transition: color 150ms ease;
    text-align-last: right;
  }

  .setting-select:hover {
    color: rgba(255, 255, 255, 0.7);
  }

  .setting-select option {
    background: #1e1e20;
    color: #fff;
    text-align: left;
  }

  /* ── Footer Buttons ───────────────────────────────────── */
  .footer-btn {
    appearance: none;
    -webkit-appearance: none;
    border: none;
    border-radius: 10px;
    padding: 8px 20px;
    font-size: 12px;
    font-family: inherit;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 150ms ease, transform 150ms ease;
  }

  .footer-btn--ghost {
    background: transparent;
    color: rgba(255, 255, 255, 0.5);
  }

  .footer-btn--ghost:hover {
    color: rgba(255, 255, 255, 0.8);
  }

  .footer-btn--primary {
    background: var(--color-accent, #ffffff);
    color: #000;
  }

  .footer-btn--primary:hover {
    background: #e4e4e7;
    transform: translateY(-0.5px);
  }

  .footer-btn--primary:active {
    transform: scale(0.98) translateY(0);
  }
</style>
