<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { isExportOpen } from "../../../stores/ui";
  import ToggleSwitch from "../primitives/ToggleSwitch.svelte";
  import { folder, libraryItems } from "../../../stores/browse";
  import { decodeState, imageMeta, videoMark } from "../../../stores/app";
  import { workspace } from "../../../stores/workspace";
  import { pickFolder } from "../../fs";
  import {
    cancelExportBatch,
    exportBatch,
    exportImage,
    revealInFinder,
  } from "../../../ipc/commands";
  import {
    onExportBatchDone,
    onExportBatchProgress,
    onExportProgress,
  } from "../../../ipc/events";
  import {
    formatAppError,
    type ExportSettings,
    type MetadataPolicy,
  } from "../../../ipc/types";

  type Scope = "current" | "folder";

  let format = $state<ExportSettings["format"]>("jpeg");
  let quality = $state(90);
  let target = $state<ExportSettings["target"]>("srgb");
  let maxDim = $state(2560);
  let resizeImage = $state(false);
  let metadataPolicy = $state<MetadataPolicy>("preserve");
  let copyright = $state("");
  let exportPath = $state("");
  let scope = $state<Scope>("current");
  let videoClip = $state(false);
  let videoAudio = $state(true);
  let busy = $state(false);
  let status = $state<string | null>(null);
  let progress = $state<{ phase: string; pct: number } | null>(null);
  let savedPath = $state<string | null>(null);

  const decodeReady = $derived($decodeState === "ready");
  const lossy = $derived(format === "jpeg" || format === "heic");
  const folderCount = $derived($libraryItems.length);
  const batchMode = $derived(scope === "folder");
  const isVideo = $derived($imageMeta?.kind === "video");
  const noun = $derived($workspace === "video" ? "clips" : "photos");
  const currentNoun = $derived(isVideo ? "clip" : "photo");

  // Default export path to active folder if set
  $effect(() => {
    if ($folder && !exportPath) {
      exportPath = `${$folder}/exports`;
    }
  });

  $effect(() => {
    let cancelled = false;
    const unlistens: (() => void)[] = [];
    onExportProgress((p) => {
      if (batchMode) return;
      const pct = p.total > 0 ? Math.round((p.done / p.total) * 100) : 0;
      progress = { phase: p.phase, pct };
      if (p.phase === "render") status = `Rendering tiles ${p.done}/${p.total}…`;
      else if (p.phase === "encode") status = "Encoding…";
    }).then((u) => {
      if (cancelled) u();
      else unlistens.push(u);
    });
    onExportBatchProgress((p) => {
      const filePct = p.total > 0 ? p.done / p.total : 0;
      const pct = Math.round(((p.index + filePct) / Math.max(p.count, 1)) * 100);
      progress = { phase: p.phase, pct };
      const name = p.path.split("/").pop() ?? p.path;
      status = `Batch ${p.index + 1}/${p.count}: ${name} (${p.phase})`;
    }).then((u) => {
      if (cancelled) u();
      else unlistens.push(u);
    });
    onExportBatchDone((d) => {
      busy = false;
      progress = { phase: "done", pct: 100 };
      if (d.cancelled) {
        status = `Batch cancelled — ${d.ok.length} saved, ${d.failed.length} failed`;
      } else {
        status = `Batch done — ${d.ok.length} saved, ${d.failed.length} failed`;
      }
      if (d.ok[0]) savedPath = d.ok[0];
    }).then((u) => {
      if (cancelled) u();
      else unlistens.push(u);
    });
    return () => {
      cancelled = true;
      unlistens.forEach((u) => u());
    };
  });

  function close() {
    if (busy) return;
    isExportOpen.set(false);
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }

  async function handleBrowseFolder() {
    const dir = await pickFolder();
    if (dir) {
      exportPath = dir;
      status = null;
    }
  }

  function buildSettings(): ExportSettings {
    return {
      format,
      target,
      quality,
      maxDim: resizeImage ? maxDim || null : null,
      sharpen: 25,
      destDir: exportPath,
      metadataPolicy,
      stripMetadata: metadataPolicy === "stripAll",
      copyright: copyright.trim() || null,
      watermarkText: null,
      videoClip: isVideo && videoClip && !batchMode,
      videoIn: isVideo && videoClip ? $videoMark.inFrame : null,
      videoOut: isVideo && videoClip ? $videoMark.outFrame : null,
      videoAudio: isVideo && videoClip && videoAudio,
    };
  }

  async function handleExport() {
    busy = true;
    savedPath = null;
    progress = null;

    if (batchMode) {
      const paths = $libraryItems.map((p) => p.path).filter(Boolean);
      if (paths.length === 0) {
        status = `No ${noun} in the current folder to export.`;
        busy = false;
        return;
      }
      if (!exportPath) {
        status = "Choose a save folder…";
        busy = false;
        return;
      }
      status = `Queuing ${paths.length} exports…`;
      try {
        const n = await exportBatch(paths, buildSettings());
        status = `Exporting ${n} ${noun}…`;
      } catch (e) {
        status = `Export failed: ${formatAppError(e)}`;
        progress = null;
        busy = false;
      }
      return;
    }

    if (!decodeReady) {
      status = "Wait until the image is ready to export.";
      busy = false;
      return;
    }

    status = exportPath ? "Starting export…" : "Choose a save folder…";

    try {
      const out = await exportImage(buildSettings());
      savedPath = out;
      if (!exportPath) {
        const slash = out.lastIndexOf("/");
        if (slash > 0) exportPath = out.slice(0, slash);
      }
      status = `Saved: ${out.split("/").pop()}`;
      progress = { phase: "done", pct: 100 };
    } catch (e) {
      status = `Export failed: ${formatAppError(e)}`;
      progress = null;
    } finally {
      busy = false;
    }
  }

  async function handleCancelBatch() {
    try {
      await cancelExportBatch();
    } catch {
      /* ignore */
    }
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

<!-- Modal Backdrop -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  transition:fade={{ duration: 200 }}
  class="backdrop absolute inset-0 z-[100] flex items-center justify-center bg-black/60 px-4 py-6"
  onclick={close}
>
  <!-- Modal Content Card -->
  <div
    transition:scale={{ duration: 250, start: 0.95 }}
    class="modal-card relative flex max-h-[560px] w-[520px] flex-col overflow-hidden text-fg"
    onclick={(e) => e.stopPropagation()}
  >
    <header class="flex shrink-0 items-center justify-between border-b border-border px-6 py-4">
      <h3 class="text-sm font-semibold text-fg">{isVideo ? "Export Clip" : "Export Photo"}</h3>
      <button class="close-btn" onclick={close} aria-label="Close export dialog">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" xmlns="http://www.w3.org/2000/svg">
          <path d="M1 1L11 11M11 1L1 11" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
        </svg>
      </button>
    </header>

    <div class="flex-1 overflow-y-auto px-6 py-5">
      <div class="space-y-5">
        <div class="setting-group">
          <span class="setting-label">Scope</span>
          <div class="mt-1 flex gap-2">
            <button
              type="button"
              class="action-btn {scope === 'current' ? 'ring-1 ring-accent' : ''}"
              disabled={busy}
              onclick={() => (scope = "current")}
            >
              Current {currentNoun}
            </button>
            <button
              type="button"
              class="action-btn {scope === 'folder' ? 'ring-1 ring-accent' : ''}"
              disabled={busy}
              onclick={() => (scope = "folder")}
            >
              Folder ({folderCount})
            </button>
          </div>
          <p class="setting-desc">
            {batchMode
              ? `Exports every ${$workspace === "video" ? "clip" : "photo"} in the current library folder with its own sidecar edits.`
              : isVideo && videoClip
                ? "Every frame from In to Out goes through the same graph as preview, then FFmpeg muxes H.264 (no dropped frames). Audio is copied when present, otherwise the clip is silent. Delivery is Rec.709 SDR."
                : isVideo
                  ? "Exports the current video frame as a still through the same graph as preview. Audio is not muxed. HDR/log inputs are tone-mapped to Rec.709 SDR."
                  : "Exports the photo currently open in the editor."}
          </p>
          {#if isVideo && !batchMode}
            <div class="mt-2">
              <ToggleSwitch
                checked={videoClip}
                label={`Clip ${$videoMark.inFrame}–${$videoMark.outFrame} → H.264`}
                onchange={(v) => (videoClip = v)}
              />
            </div>
            {#if videoClip}
              <div class="mt-2">
                <ToggleSwitch
                  checked={videoAudio}
                  label="Copy source audio"
                  onchange={(v) => (videoAudio = v)}
                />
              </div>
            {/if}
          {/if}
        </div>

        <div class="setting-group">
          <label for="export-folder" class="setting-label">Destination Directory</label>
          <div class="flex gap-2">
            <input
              id="export-folder"
              type="text"
              placeholder="No path selected"
              class="setting-input w-[180px] truncate"
              bind:value={exportPath}
              disabled={busy}
            />
            <button class="action-btn" onclick={handleBrowseFolder} disabled={busy}>
              Browse…
            </button>
          </div>
        </div>

        <!-- File Format -->
        <div class="setting-row">
          <span class="setting-label">File format</span>
          <select id="format-select" bind:value={format} class="setting-select" disabled={busy}>
            <option value="jpeg">JPEG (8-bit compressed)</option>
            <option value="tiff16">TIFF (16-bit uncompressed)</option>
            <option value="png">PNG (8-bit lossless)</option>
            <option value="heic">HEIC</option>
          </select>
        </div>

        <!-- Color Space -->
        <div class="setting-row">
          <span class="setting-label">Color space</span>
          <select id="colorspace-select" bind:value={target} class="setting-select" disabled={busy}>
            <option value="srgb">sRGB IEC61966-2.1</option>
            <option value="adobe-rgb">Adobe RGB 1998</option>
            <option value="display-p3">Display P3</option>
            <option value="prophoto">ProPhoto RGB</option>
          </select>
        </div>

        <!-- JPEG Quality Slider (only for lossy formats) -->
        {#if lossy}
          <div class="setting-row">
            <span class="setting-label">JPEG quality</span>
            <div class="flex items-center gap-3 w-[200px]">
              <input
                id="quality-slider"
                type="range"
                min="50"
                max="100"
                bind:value={quality}
                class="setting-slider flex-1"
                disabled={busy}
              />
              <span class="setting-value num w-[38px] text-right">{quality}%</span>
            </div>
          </div>
        {/if}

        <!-- Resizing options -->
        <div class="setting-row">
          <span class="setting-label">Resize image</span>
          <ToggleSwitch checked={resizeImage} label="Resize Image" onchange={(v) => (resizeImage = v)} />
        </div>

        {#if resizeImage}
          <div class="setting-row setting-row--nested">
            <span class="setting-label setting-label--secondary">Max edge (px)</span>
            <div class="flex items-center gap-1.5">
              <input
                id="resize-max"
                type="number"
                bind:value={maxDim}
                class="setting-input w-[80px] text-center"
                min="100"
                max="10000"
                disabled={busy}
              />
            </div>
          </div>
        {/if}

        <div class="setting-group border-t border-border pt-4 flex flex-col gap-3">
          <label for="meta-policy" class="setting-label">Metadata</label>
          <select
            id="meta-policy"
            class="setting-input"
            bind:value={metadataPolicy}
            disabled={busy}
          >
            <option value="preserve">Preserve (incl. GPS)</option>
            <option value="stripGps">Preserve, strip GPS</option>
            <option value="stripAll">Strip all metadata</option>
          </select>
          <label for="copyright" class="setting-label">Copyright / Artist</label>
          <input
            id="copyright"
            type="text"
            class="setting-input"
            placeholder="© Your Name"
            bind:value={copyright}
            disabled={busy || metadataPolicy === "stripAll"}
          />
        </div>
      </div>

      {#if busy && progress && progress.phase !== "done"}
        <div class="progress-track">
          <div class="progress-fill" style="width: {progress.pct}%"></div>
        </div>
      {/if}

      {#if !batchMode && !decodeReady && !busy}
        <p class="status-line status-line--warn">
          Image still decoding — export unlocks when status says ready.
        </p>
      {/if}
      {#if batchMode && folderCount === 0 && !busy}
        <p class="status-line status-line--warn">
          No {noun} in the current folder.
        </p>
      {/if}

      {#if status}
        <p
          class="status-line"
          class:status-line--ok={status.startsWith("Saved") || status.startsWith("Batch done")}
          class:status-line--error={status.startsWith("Export failed")}
        >
          {status}
        </p>
      {/if}
    </div>

    <!-- Footer Actions -->
    <footer class="flex shrink-0 items-center justify-end gap-3 border-t border-border px-8 py-4">
      {#if savedPath}
        <button type="button" class="action-btn" onclick={() => revealInFinder(savedPath!)}>
          Reveal in Finder
        </button>
      {/if}
      {#if busy && batchMode}
        <button class="footer-btn footer-btn--ghost" onclick={() => void handleCancelBatch()}>
          Cancel batch
        </button>
      {:else}
        <button class="footer-btn footer-btn--ghost" onclick={close} disabled={busy}>
          Cancel
        </button>
      {/if}
      <button
        class="footer-btn footer-btn--primary"
        onclick={handleExport}
        disabled={busy || (!batchMode && !decodeReady) || (batchMode && folderCount === 0)}
      >
        {busy ? "Exporting…" : batchMode ? `Export ${folderCount}` : "Export"}
      </button>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    background: rgba(0, 0, 0, 0.35);
    -webkit-backdrop-filter: blur(16px) saturate(120%);
    backdrop-filter: blur(16px) saturate(120%);
  }

  /* Solid panel + hairline border + one shadow layer. */
  .modal-card {
    background: var(--color-panel);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-popover);
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 0;
    border-bottom: 1px solid var(--color-border);
    transition: background-color 150ms ease;
  }

  .setting-row:last-child {
    border-bottom: none;
  }

  .setting-row--nested {
    padding: 10px 0 14px 0;
    background: transparent;
  }

  .setting-label {
    font-size: 13px;
    font-weight: 400;
    color: var(--color-secondary);
    letter-spacing: -0.005em;
  }

  .setting-label--secondary {
    color: var(--color-subtle);
    font-size: 12px;
  }

  .setting-value {
    font-size: 12px;
    font-weight: 500;
    color: var(--color-subtle);
  }

  /* ── Form Controls ──────────────────────────────────── */
  .setting-input {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid var(--color-border);
    background: var(--color-hover);
    border-radius: 8px;
    padding: 7px 12px;
    font-size: 12px;
    font-family: inherit;
    color: var(--color-secondary);
    outline: none;
    transition: border-color 150ms ease, background-color 150ms ease;
  }

  .setting-input:focus {
    border-color: var(--color-border-strong);
    background: var(--color-hover);
  }

  .setting-input:disabled {
    opacity: 0.45;
    cursor: default;
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
    color: var(--color-subtle);
    outline: none;
    cursor: pointer;
    transition: color 150ms ease;
    text-align-last: right;
  }

  .setting-select:hover {
    color: var(--color-secondary);
  }

  .setting-select:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .setting-select option {
    background: var(--color-panel);
    color: var(--color-fg);
    text-align: left;
  }

  .setting-slider {
    appearance: none;
    -webkit-appearance: none;
    height: 2px;
    border-radius: 999px;
    background: var(--color-active);
    outline: none;
  }

  .setting-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--color-fg);
    cursor: pointer;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
    transition: transform 150ms ease;
  }

  .setting-slider::-webkit-slider-thumb:hover {
    transform: scale(1.15);
  }

  /* ── Progress + Status ────────────────────────────────── */
  /* Hairline progress, per the schema. Never a chunky bar. */
  .progress-track {
    margin-top: 4px;
    height: 2.5px;
    overflow: hidden;
    border-radius: 2px;
    background: var(--color-sunken);
  }

  .progress-fill {
    height: 100%;
    background: var(--color-accent);
    transition: width 200ms ease;
  }

  .status-line {
    margin: 10px 0 0;
    font-size: 11px;
    color: var(--color-subtle);
  }

  .status-line--warn {
    color: rgba(248, 113, 113, 0.9);
  }

  .status-line--ok {
    color: rgba(74, 222, 128, 0.9);
  }

  .status-line--error {
    color: rgba(248, 113, 113, 0.9);
  }

  /* ── Action/Footer Buttons ────────────────────────────── */
  .action-btn {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid var(--color-border-strong);
    background: var(--color-hover);
    border-radius: 8px;
    padding: 6px 14px;
    font-size: 12px;
    font-family: inherit;
    font-weight: 500;
    color: var(--color-secondary);
    cursor: pointer;
    transition: background-color 150ms ease, border-color 150ms ease;
  }

  .action-btn:hover:not(:disabled) {
    background-color: var(--color-active);
    border-color: var(--color-border-strong);
  }

  .action-btn:active:not(:disabled) {
    transform: scale(0.98);
  }

  .action-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

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
    color: var(--color-subtle);
  }

  .footer-btn--ghost:hover:not(:disabled) {
    color: var(--color-fg);
  }

  .footer-btn--ghost:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .footer-btn--primary {
    background: var(--color-accent);
    color: #000;
  }

  .footer-btn--primary:hover:not(:disabled) {
    background: var(--color-border-strong);
    transform: translateY(-0.5px);
  }

  .footer-btn--primary:active:not(:disabled) {
    transform: scale(0.98) translateY(0);
  }

  .footer-btn--primary:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
