<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { isSettingsOpen, classicLook, themePreference, setThemePreference, triggerThemeTransition, type ThemePreference } from "../../../stores/ui";
  import ToggleSwitch from "../primitives/ToggleSwitch.svelte";
  import { folder, importAndBrowse } from "../../../stores/browse";
  import { pickFolder } from "../../fs";
  import {
    telemetryConsent,
    setTelemetryConsent,
  } from "../../../analytics/telemetry";
  import { telemetryConfigured } from "../../../config";
  import { histMode, setHistMode, type HistMode } from "../../../stores/app";
  import { licenseStatus } from "../../../stores/session";
  import { licenseClearToken } from "../../../ipc/commands";
  import { formatAppError } from "../../../ipc/types";

  // Modal active tab state
  type Tab = "general" | "editor" | "performance" | "export";
  let activeTab = $state<Tab>("general");

  // Local settings state
  let language = $state("en");
  let autoThumbnails = $state(true);
  
  let rawDecoder = $state("libraw");
  let colorSpace = $state("srgb");

  const histogramType = $derived($histMode);
  function onHistogramType(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value as HistMode;
    if (v === "rgb" || v === "luma" || v === "parade" || v === "wave" || v === "scope") setHistMode(v);
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

  let signingOut = $state(false);
  async function signOut() {
    signingOut = true;
    try {
      await licenseClearToken();
      licenseStatus.set({
        licensed: false,
        userId: null,
        email: null,
        reason: "signed out",
      });
      close();
    } catch (e) {
      console.error(formatAppError(e));
    } finally {
      signingOut = false;
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
    class="modal-card relative flex h-[520px] w-[720px] overflow-hidden text-fg"
    onclick={(e) => e.stopPropagation()}
  >
    <!-- Left Navigation Sidebar -->
    <aside class="sidebar flex w-[180px] shrink-0 flex-col border-r border-border px-3 py-5">
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
      <div class="mt-auto px-3 text-[10px] text-subtle">
        <p>MeraRAW v0.1.0 (Beta)</p>
      </div>
    </aside>

    <!-- Right Content Pane -->
    <main class="flex flex-1 flex-col overflow-hidden relative">
      <!-- Close Button -->
      <button
        class="close-btn"
        onclick={close}
        aria-label="Close settings"
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" xmlns="http://www.w3.org/2000/svg">
          <path d="M1 1L11 11M11 1L1 11" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
      </button>

      <!-- Settings List -->
      <div class="flex-1 overflow-y-auto px-8 pt-7 pb-6">
        <h2 class="content-title capitalize">{activeTab}</h2>
        {#if activeTab === "general"}
          <div class="settings-list">
            <div class="setting-row">
              <span class="setting-label">Account</span>
              <div class="flex items-center gap-2">
                <span class="setting-desc truncate max-w-[220px]">
                  {$licenseStatus?.email || ($licenseStatus?.licensed ? "Signed in" : "Not signed in")}
                </span>
                {#if $licenseStatus?.licensed}
                  <button class="action-btn" disabled={signingOut} onclick={() => void signOut()}>
                    {signingOut ? "Signing out…" : "Sign out"}
                  </button>
                {/if}
              </div>
            </div>

            <!-- Default Import Folder -->
            <div class="setting-row">
              <span class="setting-label">Default import directory</span>
              <div class="flex items-center gap-2">
                <input
                  id="default-folder"
                  type="text"
                  placeholder="No directory selected"
                  class="setting-input w-[200px] truncate"
                  value={$folder || ""}
                  readonly
                />
                <button class="action-btn" onclick={handleBrowseFolder}>
                  Browse…
                </button>
              </div>
            </div>

            <!-- Theme Settings -->
            <div class="setting-row">
              <span class="setting-label">Interface theme</span>
              <select
                id="theme-select"
                value={$themePreference}
                onchange={(e) => setThemePreference((e.currentTarget as HTMLSelectElement).value as ThemePreference)}
                class="setting-select"
              >
                <option value="dark">Dark mode</option>
                <option value="light">Light mode</option>
                <option value="system">Follow system</option>
              </select>
            </div>
            <p class="setting-desc">
              Affects panels and menus only. The area around the photo stays dark
              so exposure and white balance still read true.
            </p>

            <!-- Language Settings -->
            <div class="setting-row">
              <span class="setting-label">Language</span>
              <select id="lang-select" bind:value={language} class="setting-select">
                <option value="en">English (US)</option>
                <option value="de">Deutsch</option>
                <option value="fr">Français</option>
                <option value="es">Español</option>
              </select>
            </div>

            <!-- Auto thumbnails -->
            <div class="setting-row">
              <span class="setting-label">Auto-generate thumbnails</span>
              <ToggleSwitch checked={autoThumbnails} label="Auto-generate Thumbnails" onchange={(v) => autoThumbnails = v} />
            </div>

            <!-- Classic Look -->
            <div class="setting-row">
              <span class="setting-label">Classic look</span>
              <ToggleSwitch checked={$classicLook} label="Classic Look" onchange={triggerThemeTransition} />
            </div>


            <!-- Anonymous usage data -->
            {#if telemetryConfigured()}
              <div class="flex items-center justify-between py-2 border-t border-border">
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
          <div class="settings-list">
            <!-- Raw Decoder -->
            <div class="setting-row">
              <span class="setting-label">Default RAW decoder</span>
              <select id="decoder-select" bind:value={rawDecoder} class="setting-select">
                <option value="libraw">LibRaw engine</option>
                <option value="dng">Adobe DNG converter</option>
                <option value="native">Camera native API</option>
              </select>
            </div>

            <!-- Histogram Type -->
            <div class="setting-row">
              <span class="setting-label">Default histogram</span>
              <select
                id="histogram-select"
                value={histogramType}
                onchange={onHistogramType}
                class="setting-select"
              >
                <option value="rgb">RGB overlay</option>
                <option value="luma">Luminance channel</option>
                <option value="parade">RGB parade</option>
                <option value="wave">Luma waveform</option>
                <option value="scope">Vectorscope</option>
              </select>
            </div>

            <!-- Working Color Space -->
            <div class="setting-row">
              <span class="setting-label">Working color space</span>
              <select id="colorspace-select" bind:value={colorSpace} class="setting-select">
                <option value="srgb">sRGB IEC61966-2.1</option>
                <option value="adobe">Adobe RGB 1998</option>
                <option value="p3">Display P3</option>
                <option value="prophoto">ProPhoto RGB</option>
              </select>
            </div>
          </div>
        {:else if activeTab === "performance"}
          <div class="settings-list">
            <!-- GPU Accel -->
            <div class="setting-row">
              <span class="setting-label">GPU hardware acceleration</span>
              <ToggleSwitch checked={gpuAcceleration} label="GPU Acceleration" onchange={(v) => gpuAcceleration = v} />
            </div>

            <!-- Cache Size -->
            <div class="setting-row">
              <span class="setting-label">Texture cache limit</span>
              <div class="flex items-center gap-3 w-[200px]">
                <input
                  id="cache-slider"
                  type="range"
                  min="2"
                  max="50"
                  step="2"
                  bind:value={cacheSize}
                  class="setting-slider flex-1"
                />
                <span class="setting-value num w-[38px] text-right">{cacheSize} GB</span>
              </div>
            </div>
          </div>
        {:else if activeTab === "export"}
          <div class="settings-list">
            <!-- Format -->
            <div class="setting-row">
              <span class="setting-label">Default format</span>
              <select id="format-select" bind:value={exportFormat} class="setting-select">
                <option value="jpeg">JPEG</option>
                <option value="tiff">TIFF</option>
                <option value="png">PNG</option>
                <option value="dng">DNG</option>
              </select>
            </div>

            <!-- Quality -->
            {#if exportFormat === "jpeg"}
              <div class="setting-row">
                <span class="setting-label">JPEG quality</span>
                <div class="flex items-center gap-3 w-[200px]">
                  <input
                    id="quality-slider"
                    type="range"
                    min="50"
                    max="100"
                    bind:value={jpegQuality}
                    class="setting-slider flex-1"
                  />
                  <span class="setting-value num w-[38px] text-right">{jpegQuality}%</span>
                </div>
              </div>
            {/if}

            <!-- Location -->
            <div class="setting-row">
              <span class="setting-label">Export location</span>
              <select id="location-select" bind:value={exportLocation} class="setting-select">
                <option value="source">Same as source</option>
                <option value="ask">Ask every time</option>
              </select>
            </div>
          </div>
        {/if}
      </div>

      <!-- Footer -->
      <footer class="flex shrink-0 items-center justify-end gap-3 border-t border-border px-8 py-4">
        <button class="footer-btn footer-btn--primary" onclick={close}>
          Apply
        </button>
      </footer>
    </main>
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

  /* ── Sidebar ────────────────────────────────────────── */
  .sidebar {
    background: transparent;
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
    padding: 10px 16px;
    border-radius: 10px;
    font-size: 13px;
    font-weight: 500;
    color: var(--color-subtle);
    text-align: left;
    transition: background 150ms ease, color 150ms ease;
  }

  .tab-btn:hover {
    background: var(--color-hover);
    color: var(--color-secondary);
  }

  .tab-btn--active {
    background: var(--color-active);
    color: var(--color-fg);
  }

  .tab-icon {
    width: 15px;
    height: 15px;
    opacity: 0.5;
    transition: opacity 150ms ease;
  }

  .tab-btn--active .tab-icon {
    opacity: 1;
    color: var(--color-accent, #ffffff);
  }

  /* ── Close Button ────────────────────────────────────── */
  .close-btn {
    position: absolute;
    top: 18px;
    right: 20px;
    z-index: 10;
    appearance: none;
    -webkit-appearance: none;
    border: none;
    background: transparent;
    padding: 6px;
    cursor: pointer;

    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    color: var(--color-subtle);
    transition: background 150ms ease, color 150ms ease;
  }

  .close-btn:hover {
    background: var(--color-hover);
    color: var(--color-fg);
  }

  /* ── Content Title ──────────────────────────────────── */
  .content-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--color-fg);
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
    border-bottom: 1px solid var(--color-border);
    transition: background-color 150ms ease;
  }

  .setting-row:last-child {
    border-bottom: none;
  }

  .setting-label {
    font-size: 13px;
    font-weight: 400;
    color: var(--color-secondary);
    letter-spacing: -0.005em;
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

  .action-btn:hover {
    background-color: var(--color-active);
    border-color: var(--color-border-strong);
  }

  .action-btn:active {
    transform: scale(0.98);
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

  .footer-btn--primary {
    background: var(--color-accent, #ffffff);
    color: #000;
  }

  .footer-btn--primary:hover {
    background: var(--color-border-strong);
    transform: translateY(-0.5px);
  }

  .footer-btn--primary:active {
    transform: scale(0.98) translateY(0);
  }
</style>
