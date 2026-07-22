<script lang="ts">
  import { slide } from "svelte/transition";
  import GlassPanel from "../primitives/GlassPanel.svelte";
  import { activePhoto, thumbUrl } from "../../../stores/browse";
  import { photoDetailsCollapsed } from "../../../stores/editor";
  import { getAssetDetail } from "../../../ipc/commands";
  import type { AssetDetail } from "../../../ipc/types";
  import chevron from "../../icons/chevron-circle.svg";
  import chevronOpen from "../../icons/chevron-circle-open.svg";

  let { class: cls = "" }: { class?: string } = $props();

  let metadataOpen = $state(true);
  let detail = $state<AssetDetail | null>(null);

  $effect(() => {
    const photo = $activePhoto;
    detail = null;
    if (!photo) return;
    let cancelled = false;
    getAssetDetail(photo.id)
      .then((d) => {
        if (!cancelled) detail = d;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  });

  function formatDate(iso: string | null): string {
    if (!iso) return "—";
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return iso;
    return d.toLocaleString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
    });
  }

  const rows = $derived.by(() => {
    const photo = $activePhoto;
    if (!photo) return [];
    const d = detail;
    return [
      { label: "Dimensions", value: `${photo.width} × ${photo.height}` },
      { label: "Date Captured", value: formatDate(photo.capturedAt) },
      { label: "Camera", value: d?.cameraMake && d?.cameraModel ? `${d.cameraMake} ${d.cameraModel}` : photo.cameraModel ?? "—" },
      { label: "Lens", value: d?.lens ?? "—" },
      { label: "Focal Length", value: d?.focalMm != null ? `${d.focalMm}mm` : "—" },
      { label: "Aperture", value: d?.aperture != null ? `f/${d.aperture}` : "—" },
      { label: "Shutter Speed", value: d?.shutter ?? "—" },
      { label: "ISO", value: d?.iso != null ? String(d.iso) : "—" },
    ];
  });
</script>

<GlassPanel class="flex h-full min-h-0 w-full flex-col overflow-hidden p-[10px] {cls}" style="--glass-bg: #171717;">
  <!-- Header with title and collapse control, matching FileBrowser -->
  <div class="mb-2 flex shrink-0 items-center justify-between px-[6px] pb-[6px]">
    <span class="text-[11px] font-semibold tracking-wide text-white/90">Details</span>
    <button
      onclick={() => photoDetailsCollapsed.set(true)}
      aria-label="Collapse Details"
      class="flex size-[26px] cursor-pointer items-center justify-center rounded-full border border-white/5 bg-white/[0.04] text-white/80 transition-all hover:bg-white/[0.12] hover:text-white active:scale-95"
    >
      <svg width="6" height="10" viewBox="0 0 6 10" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="pointer-events-none">
        <path d="M1.5 1.5L5 5L1.5 8.5" />
      </svg>
    </button>
  </div>

  <div class="min-h-0 flex-1 overflow-y-auto px-[4px]">
    <!-- Preview -->
    <div class="flex aspect-[3/2] shrink-0 items-center justify-center overflow-hidden border border-white/[0.04] bg-black/20 will-change-[width]">
      {#if $activePhoto?.hasThumb}
        <img src={thumbUrl($activePhoto.id, "p")} alt={$activePhoto.filename} class="h-full w-full object-contain" />
      {:else if $activePhoto}
        <span class="text-[10px] font-semibold tracking-widest text-white/20">RAW</span>
      {:else}
        <span class="text-[10px] text-white/20">No photo selected</span>
      {/if}
    </div>

    <!-- Metadata -->
    <section class="mt-[11px] w-full shrink-0 rounded-[22px] border border-white/[0.04] bg-[#2b2b2b]/60 shadow-inner">
      <button
        class="grid h-[28px] w-full cursor-pointer grid-cols-[26px_1fr_26px] items-center px-[10px] focus:outline-none"
        onclick={() => (metadataOpen = !metadataOpen)}
        aria-expanded={metadataOpen}
      >
        <span></span>
        <span class="text-center text-[10px] font-medium text-white/90">Metadata</span>
        <img
          src={metadataOpen ? chevronOpen : chevron}
          alt=""
          class="h-[18px] w-[26px] justify-self-end opacity-60 transition-opacity hover:opacity-100"
        />
      </button>
      {#if metadataOpen}
        <div class="px-[14px] pb-[14px] pt-[6px]" transition:slide={{ duration: 180 }}>
          {#if $activePhoto}
            <p class="mb-[10px] truncate text-[11px] font-medium text-white/90">{$activePhoto.filename}</p>
            <div class="flex flex-col gap-[8px]">
              {#each rows as row}
                <div class="flex items-baseline justify-between gap-[8px]">
                  <span class="text-[9px] text-white/35">{row.label}</span>
                  <span class="truncate text-[9px] text-white/70">{row.value}</span>
                </div>
              {/each}
            </div>
          {:else}
            <p class="text-[10px] text-white/25">Select a photo to view its details.</p>
          {/if}
        </div>
      {/if}
    </section>
  </div>
</GlassPanel>
