<!--
  Versions / history popover.

  A port of Pencil's `versions` control. The whole backend for this already
  existed — get_history, snapshot, list_snapshots and restore_snapshot are all
  registered Tauri commands with frontend bindings — but nothing in the UI ever
  called them, so a full named-version system shipped dark. This surfaces it.

  Two sections, per the menu spec: sentence-case bold headers, rows with a
  right-aligned mono hint, no uppercase-tracked caps (those are panel eyebrows).
-->
<script lang="ts">
  import {
    getHistory,
    listSnapshots,
    restoreSnapshot,
    snapshot,
    undo,
  } from "../../../ipc/commands";
  import { reconcile } from "../../../stores/doc";
  import { imageOpen } from "../../../stores/app";

  let open = $state(false);
  let history = $state<string[]>([]);
  let versions = $state<string[]>([]);
  let naming = $state(false);
  let draftName = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function refresh() {
    error = null;
    try {
      const [h, v] = await Promise.all([getHistory(), listSnapshots()]);
      history = h;
      versions = v;
    } catch (e) {
      error = String(e);
    }
  }

  async function toggle() {
    open = !open;
    if (open) await refresh();
  }

  /** Step back N edits. The engine exposes single-step undo, so walk it. */
  async function revertTo(index: number) {
    const steps = history.length - index;
    if (steps <= 0) return;
    busy = true;
    error = null;
    try {
      for (let i = 0; i < steps; i++) reconcile(await undo());
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function saveVersion() {
    const name = draftName.trim();
    if (!name) return;
    busy = true;
    error = null;
    try {
      await snapshot(name);
      draftName = "";
      naming = false;
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function restore(name: string) {
    busy = true;
    error = null;
    try {
      reconcile(await restoreSnapshot(name));
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && open) {
      e.stopPropagation();
      open = false;
      naming = false;
    }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if $imageOpen}
  <div class="ver-wrap">
    <button class="ver-trigger" onclick={toggle} aria-expanded={open} title="Versions & history">
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M3 3v5h5" />
        <path d="M3.05 13A9 9 0 1 0 6 5.3L3 8" />
        <path d="M12 7v5l4 2" />
      </svg>
      <span>versions</span>
    </button>

    {#if open}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div class="ver-scrim" onclick={() => (open = false)}></div>
      <div class="ver-popover" role="dialog" aria-label="Versions and history">
        <div class="ver-section">Saved versions</div>
        {#if versions.length === 0}
          <div class="ver-empty">No saved versions yet.</div>
        {:else}
          {#each versions as name (name)}
            <button class="ver-row" disabled={busy} onclick={() => restore(name)}>
              <span class="ver-label">{name}</span>
              <span class="ver-hint">restore</span>
            </button>
          {/each}
        {/if}

        {#if naming}
          <div class="ver-name-row">
            <input
              placeholder="Version name"
              bind:value={draftName}
              onkeydown={(e) => { if (e.key === "Enter") void saveVersion(); }}
            />
            <button class="mini-button" disabled={busy || !draftName.trim()} onclick={() => void saveVersion()}>Save</button>
            <button class="mini-button" onclick={() => { naming = false; draftName = ""; }}>Cancel</button>
          </div>
        {:else}
          <button class="ver-row ver-row--action" disabled={busy} onclick={() => (naming = true)}>
            <span class="ver-label">Save current as version…</span>
          </button>
        {/if}

        <div class="ver-sep"></div>

        <div class="ver-section">Recent edits</div>
        {#if history.length === 0}
          <div class="ver-empty">Nothing to undo yet.</div>
        {:else}
          {#each history.slice().reverse() as label, i (history.length - i)}
            <button class="ver-row" disabled={busy} onclick={() => revertTo(history.length - i - 1)}>
              <span class="ver-label">{label}</span>
              <span class="ver-hint num">−{i + 1}</span>
            </button>
          {/each}
        {/if}

        {#if error}
          <div class="error-text ver-error">{error}</div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .ver-wrap { position: relative; flex: none; }

  .ver-trigger {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 9px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    color: var(--color-secondary);
    font-size: 11.5px;
    cursor: pointer;
    transition: background-color 0.15s var(--ease-std), color 0.15s var(--ease-std);
  }
  .ver-trigger:hover { background: var(--color-hover); color: var(--color-fg); }

  .ver-scrim { position: fixed; inset: 0; z-index: 39; }

  .ver-popover {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 40;
    width: 300px;
    max-height: 420px;
    overflow-y: auto;
    padding: 6px;
    background: var(--color-panel);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-popover);
  }

  /* Sentence case + semibold — menu headers, not panel eyebrows. */
  .ver-section {
    padding: 6px 10px 4px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--color-fg);
  }

  .ver-empty { padding: 4px 10px 8px; font-size: 11px; color: var(--color-subtle); }

  .ver-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
    padding: 6px 10px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--color-fg);
    font: inherit;
    font-size: 12.5px;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.12s var(--ease-std);
  }
  .ver-row:hover:not(:disabled) { background: var(--color-hover); }
  .ver-row:disabled { opacity: 0.5; cursor: default; }
  .ver-row--action { color: var(--color-secondary); }

  .ver-label { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ver-hint { flex: none; font-size: 10.5px; color: var(--color-subtle); }

  .ver-sep { height: 1px; margin: 5px 6px; background: var(--color-border); }

  .ver-name-row { display: flex; align-items: center; gap: 5px; padding: 4px 8px 6px; }
  .ver-name-row input {
    flex: 1;
    min-width: 0;
    height: 24px;
    padding: 0 8px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-sunken);
    color: var(--color-fg);
    font: inherit;
    font-size: 11.5px;
    outline: none;
  }
  .ver-name-row input:focus { border-color: var(--color-accent); }

  .mini-button {
    padding: 2px 8px;
    border: 1px solid var(--color-border);
    border-radius: 5px;
    background: transparent;
    color: var(--color-secondary);
    font-size: 10.5px;
    cursor: pointer;
  }
  .mini-button:hover:not(:disabled) { background: var(--color-hover); color: var(--color-fg); }
  .mini-button:disabled { opacity: 0.45; cursor: default; }

  .ver-error { padding: 6px 10px; }
</style>
