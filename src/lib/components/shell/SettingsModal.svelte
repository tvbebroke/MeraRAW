<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { isSettingsOpen, classicLook, triggerThemeTransition } from "../../../stores/ui";
  import ToggleSwitch from "../primitives/ToggleSwitch.svelte";
  import { folder, importAndBrowse } from "../../../stores/browse";
  import { pickFolder } from "../../fs";
  import {
    telemetryConsent,
    setTelemetryConsent,
  } from "../../../analytics/telemetry";
  import { telemetryConfigured } from "../../../config";
  import { histMode, setHistMode, type HistMode } from "../../../stores/app";

  // Modal active tab state
  type Tab = "general" | "editor" | "performance" | "export";
  let activeTab = $state<Tab>("general");

  // Local settings state
  let theme = $state("dark");
  let language = $state("en");
  let autoThumbnails = $state(true);
  
  let rawDecoder = $state("libraw");
  let colorSpace = $state("srgb");

  const histogramType = $derived($histMode);
  function onHistogramType(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value as HistMode;
    if (v === "rgb" || v === "luma" || v === "parade") setHistMode(v);
  }
  
  let gpuAcceleration = $state(true);
  let cacheSize = $state(10); // GB
  
  let exportFormat = $state("jpeg");
  let jpegQuality = $state(90);
  let exportLocation = $state("source");

  function close() {
    isSettingsOpen.set(false);
  }

  // Handle escape key
  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      close();
    }
  }

  async function handleBrowseFolder() {
    const dir = await pickFolder();
    if (dir) {
      await importAndBrowse(dir);
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
    class="modal-card flex h-[560px] w-[780px] overflow-hidden rounded-[24px] border border-white/[0.08] bg-[#171717]/95 text-white backdrop-blur-md"
    onclick={(e) => e.stopPropagation()}
  >
    <!-- Left Navigation Sidebar -->
    <aside class="flex w-[180px] shrink-0 flex-col border-r border-white/[0.05] bg-[#111113]/50 p-4">
      <h2 class="mb-4 px-3 text-[11px] font-bold uppercase tracking-wider text-white/30">Settings</h2>
      <nav class="flex flex-1 flex-col gap-[4px]">
        <button
          class="tab-btn {activeTab === 'general' ? 'tab-btn--active' : ''}"
          onclick={() => activeTab = "general"}
        >
          <!-- General Gear Icon -->
          <svg class="tab-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="8" cy="8" r="2.5" />
            <path d="M8 1v1.5M8 13.5V15M1 8h1.5M13.5 8H15M3.05 3.05l1.06 1.06M10.89 10.89l1.06 1.06M3.05 12.95l1.06-1.06M10.89 5.11l1.06-1.06" />
          </svg>
          General
        </button>

        <button
          class="tab-btn {activeTab === 'editor' ? 'tab-btn--active' : ''}"
          onclick={() => activeTab = "editor"}
        >
          <!-- Editor Sliders Icon -->
          <svg class="tab-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M3 1v5M3 10v5M8 1v9M8 14v1M13 1v2M13 6v9M1 6h4M6 10h4M11 3h4" />
          </svg>
          Editor
        </button>

        <button
          class="tab-btn {activeTab === 'performance' ? 'tab-btn--active' : ''}"
          onclick={() => activeTab = "performance"}
        >
          <!-- Performance CPU Icon -->
          <svg class="tab-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
            <rect x="4" y="4" width="8" height="8" rx="1.5" />
            <path d="M6 1v3M10 1v3M6 12v3M10 12v3M1 6h3M1 10h3M12 6h3M12 10h3" />
          </svg>
          Performance
        </button>

        <button
          class="tab-btn {activeTab === 'export' ? 'tab-btn--active' : ''}"
          onclick={() => activeTab = "export"}
        >
          <!-- Export Icon -->
          <svg class="tab-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M8 12V2.5M5.5 5L8 2.5l2.5 2.5M2 9v4.5A1.5 1.5 0 003.5 15h9a1.5 1.5 0 001.5-1.5V9" />
          </svg>
          Export
        </button>
      </nav>

      <!-- App Info at bottom -->
      <div class="mt-auto px-3 text-[10px] text-white/20">
        <p>MeraRAW v0.1.0 (Beta)</p>
      </div>
    </aside>

    <!-- Right Content Pane -->
    <main class="flex flex-1 flex-col overflow-hidden">
      <!-- Modal Header -->
      <header class="flex shrink-0 items-center justify-between border-b border-white/[0.05] px-6 py-4">
        <h3 class="text-sm font-semibold capitalize text-white/90">
          {activeTab} Settings
        </h3>
        <button
          class="close-btn"
          onclick={close}
          aria-label="Close settings"
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M1 1L11 11M11 1L1 11" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
          </svg>
        </button>
      </header>

      <!-- Settings List -->
      <div class="flex-1 overflow-y-auto px-6 py-5">
        {#if activeTab === "general"}
          <div class="space-y-5">
            <!-- Default Import Folder -->
            <div class="setting-group">
              <label for="default-folder" class="setting-label">Default Import Directory</label>
              <div class="flex gap-2">
                <input
                  id="default-folder"
                  type="text"
                  placeholder="No default directory selected"
                  class="setting-input flex-1 truncate"
                  value={$folder || ""}
                  readonly
                />
                <button class="action-btn" onclick={handleBrowseFolder}>
                  Browse…
                </button>
              </div>
              <p class="setting-desc">The folder that automatically loads when opening the library.</p>
            </div>

            <!-- Theme Settings -->
            <div class="setting-group">
              <label for="theme-select" class="setting-label">Interface Theme</label>
              <select id="theme-select" bind:value={theme} class="setting-select">
                <option value="dark">Dark Mode (Default)</option>
                <option value="light">Light Mode</option>
                <option value="system">Follow System</option>
              </select>
              <p class="setting-desc">Choose a visual aesthetic for MeraRAW.</p>
            </div>

            <!-- Language Settings -->
            <div class="setting-group">
              <label for="lang-select" class="setting-label">Language</label>
              <select id="lang-select" bind:value={language} class="setting-select">
                <option value="en">English (US)</option>
                <option value="de">Deutsch</option>
                <option value="fr">Français</option>
                <option value="es">Español</option>
              </select>
            </div>

            <!-- Auto thumbnails -->
            <div class="flex items-center justify-between py-2 border-t border-white/[0.03]">
              <div class="space-y-0.5">
                <span class="setting-label">Auto-generate Thumbnails</span>
                <p class="setting-desc">Generate low-resolution previews in background</p>
              </div>
              <ToggleSwitch checked={autoThumbnails} label="Auto-generate Thumbnails" onchange={(v) => autoThumbnails = v} />
            </div>

            <!-- Classic Look -->
            <div class="flex items-center justify-between py-2 border-t border-white/[0.03]">
              <div class="space-y-0.5">
                <span class="setting-label">Classic Look</span>
                <p class="setting-desc">Use sharp corners, a dense Lightroom-style grid layout, and flat panels</p>
              </div>
              <ToggleSwitch checked={$classicLook} label="Classic Look" onchange={triggerThemeTransition} />
            </div>

            <!-- Anonymous usage data -->
            {#if telemetryConfigured()}
              <div class="flex items-center justify-between py-2 border-t border-white/[0.03]">
                <div class="space-y-0.5">
                  <span class="setting-label">Share Anonymous Usage Data</span>
                  <p class="setting-desc">
                    Tool and export usage, render timings, and decode errors. Never
                    your photos, filenames, or folder paths.
                  </p>
                </div>
                <ToggleSwitch
                  checked={$telemetryConsent === "granted"}
                  label="Share Anonymous Usage Data"
                  onchange={setTelemetryConsent}
                />
              </div>
            {/if}
          </div>
        {:else if activeTab === "editor"}
          <div class="space-y-5">
            <!-- Raw Decoder -->
            <div class="setting-group">
              <label for="decoder-select" class="setting-label">Default RAW Decoder</label>
              <select id="decoder-select" bind:value={rawDecoder} class="setting-select">
                <option value="libraw">LibRaw Engine (Recommended)</option>
                <option value="dng">Adobe DNG Converter</option>
                <option value="native">Camera Native API</option>
              </select>
              <p class="setting-desc">The engine used to parse raw sensor metadata (NEF, CR3, ARW).</p>
            </div>

            <!-- Histogram Type -->
            <div class="setting-group">
              <label for="histogram-select" class="setting-label">Default Histogram</label>
              <select
                id="histogram-select"
                value={histogramType}
                onchange={onHistogramType}
                class="setting-select"
              >
                <option value="rgb">RGB Overlay</option>
                <option value="luma">Luminance Channel</option>
                <option value="parade">RGB Parade</option>
              </select>
              <p class="setting-desc">Preferred style for real-time tone distribution feedback.</p>
            </div>

            <!-- Working Color Space -->
            <div class="setting-group">
              <label for="colorspace-select" class="setting-label">Working Color Space</label>
              <select id="colorspace-select" bind:value={colorSpace} class="setting-select">
                <option value="srgb">sRGB IEC61966-2.1 (Web Standard)</option>
                <option value="adobe">Adobe RGB 1998 (Print)</option>
                <option value="p3">Display P3 (Apple Wide Color)</option>
                <option value="prophoto">ProPhoto RGB (Wide Gamut)</option>
              </select>
            </div>
          </div>
        {:else if activeTab === "performance"}
          <div class="space-y-5">
            <!-- GPU Accel -->
            <div class="flex items-center justify-between py-2">
              <div class="space-y-0.5">
                <span class="setting-label">GPU Hardware Acceleration</span>
                <p class="setting-desc">Use hardware rendering pipelines for image processing</p>
              </div>
              <ToggleSwitch checked={gpuAcceleration} label="GPU Acceleration" onchange={(v) => gpuAcceleration = v} />
            </div>

            <!-- Cache Size -->
            <div class="setting-group border-t border-white/[0.03] pt-4">
              <div class="flex justify-between">
                <label for="cache-slider" class="setting-label">Texture Cache Limit</label>
                <span class="text-[11px] font-semibold text-accent">{cacheSize} GB</span>
              </div>
              <input
                id="cache-slider"
                type="range"
                min="2"
                max="50"
                step="2"
                bind:value={cacheSize}
                class="setting-slider"
              />
              <p class="setting-desc">Maximum disk and RAM cache allocated for fast image scrubbing.</p>
            </div>
          </div>
        {:else if activeTab === "export"}
          <div class="space-y-5">
            <!-- Format -->
            <div class="setting-group">
              <label for="format-select" class="setting-label">Default Format</label>
              <select id="format-select" bind:value={exportFormat} class="setting-select">
                <option value="jpeg">JPEG (8-bit compressed)</option>
                <option value="tiff">TIFF (16-bit uncompressed)</option>
                <option value="png">PNG (8-bit lossless)</option>
                <option value="dng">DNG (RAW digital negative)</option>
              </select>
            </div>

            <!-- Quality -->
            {#if exportFormat === "jpeg"}
              <div class="setting-group">
                <div class="flex justify-between">
                  <label for="quality-slider" class="setting-label">JPEG Quality</label>
                  <span class="text-[11px] font-semibold text-accent">{jpegQuality}%</span>
                </div>
                <input
                  id="quality-slider"
                  type="range"
                  min="50"
                  max="100"
                  bind:value={jpegQuality}
                  class="setting-slider"
                />
              </div>
            {/if}

            <!-- Location -->
            <div class="setting-group">
              <label for="location-select" class="setting-label">Export Location</label>
              <select id="location-select" bind:value={exportLocation} class="setting-select">
                <option value="source">Same as source folder</option>
                <option value="ask">Ask every time</option>
              </select>
            </div>
          </div>
        {/if}
      </div>

      <!-- Footer Buttons -->
      <footer class="flex shrink-0 items-center justify-end gap-3 border-t border-white/[0.05] bg-[#111113]/20 px-6 py-4">
        <button class="footer-btn footer-btn--primary" onclick={close}>
          Done
        </button>
      </footer>
    </main>
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

  /* ── Tab Navigation ───────────────────────────────────── */
  .tab-btn {
    appearance: none;
    -webkit-appearance: none;
    border: none;
    background: transparent;
    font: inherit;
    cursor: pointer;

    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 12px;
    border-radius: 10px;
    font-size: 12px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.5);
    text-align: left;
    transition: background 150ms ease, color 150ms ease;
  }

  .tab-btn:hover {
    background: rgba(255, 255, 255, 0.04);
    color: rgba(255, 255, 255, 0.8);
  }

  .tab-btn--active {
    background: rgba(255, 255, 255, 0.07);
    color: #fff;
  }

  .tab-icon {
    width: 14px;
    height: 14px;
    opacity: 0.7;
    transition: opacity 150ms ease;
  }

  .tab-btn--active .tab-icon {
    opacity: 1;
    color: var(--color-accent, #ffffff);
  }

  /* ── Close Button ────────────────────────────────────── */
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

  /* ── Form Controls & Settings Layout ──────────────────── */
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
    background: rgba(255, 255, 255, 0.03) url("data:image/svg+xml,%3Csvg width='8' height='5' viewBox='0 0 8 5' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1L4 4L7 1' stroke='rgba%28255,255,255,0.4%29' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E") no-repeat right 12px center;
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

  /* ── Action/Footer Buttons ────────────────────────────── */
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

  .action-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
  }

  .action-btn:active {
    transform: scale(0.97);
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

  .footer-btn--primary:hover {
    background: #e4e4e7;
    transform: translateY(-0.5px);
  }

  .footer-btn--primary:active {
    transform: scale(0.97) translateY(0);
  }
</style>
