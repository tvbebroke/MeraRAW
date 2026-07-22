<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { isExportOpen } from "../../../stores/ui";
  import ToggleSwitch from "../primitives/ToggleSwitch.svelte";
  import { folder, photos } from "../../../stores/browse";
  import { decodeState } from "../../../stores/app";
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
  let busy = $state(false);
  let status = $state<string | null>(null);
  let progress = $state<{ phase: string; pct: number } | null>(null);
  let savedPath = $state<string | null>(null);

  const decodeReady = $derived($decodeState === "ready");
  const lossy = $derived(format === "jpeg" || format === "heic");
  const folderCount = $derived($photos.length);
  const batchMode = $derived(scope === "folder");

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
    };
  }

  async function handleExport() {
    busy = true;
    savedPath = null;
    progress = null;

    if (batchMode) {
      const paths = $photos.map((p) => p.path).filter(Boolean);
      if (paths.length === 0) {
        status = "No photos in the current folder to export.";
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
        status = `Exporting ${n} photos…`;
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

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  transition:fade={{ duration: 200 }}
  class="backdrop absolute inset-0 z-[100] flex items-center justify-center bg-black/60 px-4 py-6"
  onclick={close}
>
  <div
    transition:scale={{ duration: 250, start: 0.95 }}
    class="modal-card flex h-[480px] w-[580px] flex-col overflow-hidden rounded-[24px] border border-white/[0.08] bg-[#171717]/95 text-white shadow-2xl backdrop-blur-md"
    onclick={(e) => e.stopPropagation()}
  >
    <header class="flex shrink-0 items-center justify-between border-b border-white/[0.05] px-6 py-4">
      <h3 class="text-sm font-semibold text-white/90">Export Photo</h3>
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
              Current photo
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
              ? "Exports every photo in the current library folder with its own sidecar edits."
              : "Exports the photo currently open in the editor."}
          </p>
        </div>

        <div class="setting-group">
          <label for="export-folder" class="setting-label">Destination Directory</label>
          <div class="flex gap-2">
            <input
              id="export-folder"
              type="text"
              placeholder="No export path selected"
              class="setting-input flex-1 truncate"
              bind:value={exportPath}
              disabled={busy}
            />
            <button class="action-btn" onclick={handleBrowseFolder} disabled={busy}>
              Browse…
            </button>
          </div>
          <p class="setting-desc">Choose where the exported images will be saved.</p>
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div class="setting-group">
            <label for="format-select" class="setting-label">File Format</label>
            <select id="format-select" bind:value={format} class="setting-select" disabled={busy}>
              <option value="jpeg">JPEG (8-bit compressed)</option>
              <option value="tiff16">TIFF (16-bit uncompressed)</option>
              <option value="png">PNG (8-bit lossless)</option>
              <option value="heic">HEIC</option>
            </select>
          </div>

          <div class="setting-group">
            <label for="colorspace-select" class="setting-label">Color Space</label>
            <select id="colorspace-select" bind:value={target} class="setting-select" disabled={busy}>
              <option value="srgb">sRGB IEC61966-2.1</option>
              <option value="adobe-rgb">Adobe RGB 1998</option>
              <option value="display-p3">Display P3</option>
              <option value="prophoto">ProPhoto RGB</option>
            </select>
          </div>
        </div>

        {#if lossy}
          <div class="setting-group border-t border-white/[0.03] pt-4">
            <div class="flex justify-between">
              <label for="quality-slider" class="setting-label">JPEG Quality</label>
              <span class="text-[11px] font-semibold text-accent">{quality}%</span>
            </div>
            <input
              id="quality-slider"
              type="range"
              min="50"
              max="100"
              bind:value={quality}
              class="setting-slider"
              disabled={busy}
            />
          </div>
        {/if}

        <div class="border-t border-white/[0.03] pt-4">
          <div class="flex items-center justify-between pb-3">
            <div class="space-y-0.5">
              <span class="setting-label">Resize Image Dimensions</span>
              <p class="setting-desc">Scale the exported image to custom max edge</p>
            </div>
            <ToggleSwitch
              checked={resizeImage}
              label="Resize Image"
              onchange={(v) => (resizeImage = v)}
            />
          </div>

          {#if resizeImage}
            <div class="setting-group mt-2">
              <label for="resize-max" class="setting-label">Max edge (px)</label>
              <input
                id="resize-max"
                type="number"
                bind:value={maxDim}
                class="setting-input"
                min="100"
                max="10000"
                disabled={busy}
              />
            </div>
          {/if}
        </div>

        <div class="setting-group border-t border-white/[0.03] pt-4 flex flex-col gap-3">
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

        {#if busy && progress && progress.phase !== "done"}
          <div class="h-[4px] overflow-hidden rounded-full bg-white/[0.08]">
            <div
              class="h-full bg-accent transition-all duration-200"
              style="width: {progress.pct}%"
            ></div>
          </div>
        {/if}

        {#if !batchMode && !decodeReady && !busy}
          <p class="text-[10px] text-red-400/90">
            Image still decoding — export unlocks when status says ready.
          </p>
        {/if}
        {#if batchMode && folderCount === 0 && !busy}
          <p class="text-[10px] text-red-400/90">
            No photos in the current folder.
          </p>
        {/if}

        {#if status}
          <p class="text-[10px] {status.startsWith('Saved') ? 'text-green-400/90' : status.startsWith('Export failed') ? 'text-red-400/90' : 'text-white/45'}">
            {status}
          </p>
        {/if}
      </div>
    </div>

    <footer class="flex shrink-0 items-center justify-end gap-3 border-t border-white/[0.05] bg-[#111113]/20 px-6 py-4">
      {#if savedPath}
        <button
          type="button"
          class="action-btn"
          onclick={() => revealInFinder(savedPath!)}
        >
          Reveal in Finder
        </button>
      {/if}
      {#if busy && batchMode}
        <button class="action-btn" onclick={() => void handleCancelBatch()}>
          Cancel batch
        </button>
      {:else}
        <button class="action-btn" onclick={close} disabled={busy}>Cancel</button>
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
    background: rgba(0, 0, 0, 0.45);
    -webkit-backdrop-filter: blur(12px) saturate(120%);
    backdrop-filter: blur(12px) saturate(120%);
  }

  .modal-card {
    background: rgba(23, 23, 23, 0.95);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      0 4px 6px rgba(0, 0, 0, 0.2),
      0 12px 20px rgba(0, 0, 0, 0.25),
      0 20px 40px rgba(0, 0, 0, 0.3),
      0 40px 80px rgba(0, 0, 0, 0.45);
  }

  .close-btn {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
    padding: 0;
    margin: 0;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    color: rgba(255, 255, 255, 0.5);
    transition: background 150ms ease, color 150ms ease, transform 150ms ease;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
  }

  .close-btn:active {
    transform: scale(0.93);
  }

  .setting-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .setting-label {
    font-size: 11px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.75);
    letter-spacing: -0.01em;
  }

  .setting-desc {
    font-size: 10px;
    color: rgba(255, 255, 255, 0.35);
    margin: 0;
  }

  .setting-input {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.07);
    background: rgba(255, 255, 255, 0.03);
    border-radius: 8px;
    padding: 8px 12px;
    font-size: 11px;
    font-family: inherit;
    color: rgba(255, 255, 255, 0.85);
    outline: none;
  }

  .setting-select {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.07);
    background: rgba(255, 255, 255, 0.03)
      url("data:image/svg+xml,%3Csvg width='8' height='5' viewBox='0 0 8 5' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1L4 4L7 1' stroke='rgba%28255,255,255,0.4%29' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")
      no-repeat right 12px center;
    background-size: 8px 5px;
    border-radius: 8px;
    padding: 8px 30px 8px 12px;
    font-size: 11px;
    font-family: inherit;
    color: rgba(255, 255, 255, 0.85);
    outline: none;
    cursor: pointer;
  }

  .setting-select option {
    background: #1e1e20;
    color: #fff;
  }

  .setting-slider {
    appearance: none;
    -webkit-appearance: none;
    width: 100%;
    height: 4px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.08);
    outline: none;
    margin: 8px 0;
  }

  .setting-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--color-accent, #ffffff);
    cursor: pointer;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
    transition: transform 150ms ease;
  }

  .setting-slider::-webkit-slider-thumb:hover {
    transform: scale(1.2);
  }

  .action-btn {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    border-radius: 8px;
    padding: 6px 12px;
    font-size: 11px;
    font-family: inherit;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.85);
    cursor: pointer;
    transition: background 150ms ease, color 150ms ease, transform 150ms ease;
  }

  .action-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
  }

  .action-btn:active:not(:disabled) {
    transform: scale(0.97);
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
    transition: background 150ms ease, transform 150ms ease;
  }

  .footer-btn--primary {
    background: var(--color-accent, #ffffff);
    color: #000;
  }

  .footer-btn--primary:hover:not(:disabled) {
    background: #e4e4e7;
    transform: translateY(-0.5px);
  }

  .footer-btn--primary:active:not(:disabled) {
    transform: scale(0.97) translateY(0);
  }

  .footer-btn--primary:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
