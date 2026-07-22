<script lang="ts">
  import { statusMessage, imageMeta } from "../../../stores/app";
  import { activePhoto, folder, photos } from "../../../stores/browse";
  import { isBugReportOpen } from "../../../stores/ui";

  const photoIndex = $derived(
    $activePhoto ? $photos.findIndex((p) => p.path === $activePhoto!.path) + 1 : 0,
  );

  const exifLine = $derived.by(() => {
    const m = $imageMeta;
    if (!m) return "";
    const bits: string[] = [];
    const cam = [m.cameraMake, m.cameraModel].filter(Boolean).join(" ").trim();
    if (cam) bits.push(cam);
    if (m.focalMm != null) bits.push(`${Math.round(m.focalMm)}mm`);
    if (m.aperture != null) bits.push(`ƒ/${m.aperture}`);
    if (m.shutter) bits.push(m.shutter);
    if (m.iso != null) bits.push(`ISO ${m.iso}`);
    if (m.gpsLat != null && m.gpsLon != null) bits.push("GPS");
    if (m.kind === "rendered" && m.inputColorSpace) bits.push(m.inputColorSpace);
    return bits.join(" · ");
  });
</script>

<footer
  class="col-span-full flex h-[20px] items-center justify-between gap-3 px-2 text-[9px] text-white/35"
>
  <span class="truncate">{$folder ?? ""}</span>
  <span class="shrink-0 truncate max-w-[50%] text-center">
    {exifLine || $statusMessage}
  </span>
  <span class="flex shrink-0 items-center gap-3">
    {#if $photos.length}
      <span>{photoIndex ? `${photoIndex} / ` : ""}{$photos.length} photos</span>
    {/if}
    <button
      class="cursor-pointer rounded-[4px] border border-white/10 px-[6px] py-[1px] text-white/50 transition-colors hover:bg-white/[0.08] hover:text-white/80"
      onclick={() => isBugReportOpen.set(true)}
    >
      Report Bug
    </button>
  </span>
</footer>
