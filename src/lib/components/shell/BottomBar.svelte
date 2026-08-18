<script lang="ts">
  import { statusMessage, imageMeta, lastOpenedPath } from "../../../stores/app";
  import { activePhoto, folder, photos } from "../../../stores/browse";
  import { isBugReportOpen } from "../../../stores/ui";

  const photoIndex = $derived(
    $activePhoto ? $photos.findIndex((p) => p.path === $activePhoto!.path) + 1 : 0,
  );

  function basename(path: string | null | undefined): string {
    if (!path) return "";
    const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
    return parts.at(-1) ?? path;
  }

  const fileName = $derived(
    $activePhoto?.filename || basename($imageMeta?.path) || basename($lastOpenedPath),
  );

  const fileFormat = $derived($imageMeta?.format?.trim() ?? "");

  const fileTitle = $derived($imageMeta?.path ?? $lastOpenedPath ?? fileName);

  const exifBits = $derived.by(() => {
    const m = $imageMeta;
    if (!m) return [] as { text: string; mono: boolean }[];
    const bits: { text: string; mono: boolean }[] = [];
    const cam = [m.cameraMake, m.cameraModel].filter(Boolean).join(" ").trim();
    if (cam) bits.push({ text: cam, mono: false });
    if (m.focalMm != null) bits.push({ text: `${Math.round(m.focalMm)}mm`, mono: true });
    if (m.aperture != null) bits.push({ text: `ƒ/${m.aperture}`, mono: true });
    if (m.shutter) bits.push({ text: m.shutter, mono: true });
    if (m.iso != null) bits.push({ text: `ISO ${m.iso}`, mono: true });
    if (m.gpsLat != null && m.gpsLon != null) bits.push({ text: "GPS", mono: false });
    if (m.kind === "rendered" && m.inputColorSpace) {
      bits.push({ text: m.inputColorSpace, mono: false });
    }
    return bits;
  });

  const hasIdentity = $derived(Boolean(fileName || fileFormat || exifBits.length));
</script>

<footer
  class="col-span-full grid h-[22px] grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-4 px-3 text-[10px] leading-none text-subtle"
>
  <span class="min-w-0 truncate" title={$folder ?? ""}>{$folder ?? ""}</span>

  <div class="flex min-w-0 max-w-[min(52rem,70vw)] items-center justify-center">
    {#if hasIdentity}
      {#if fileName}
        <span class="selectable truncate text-secondary" title={fileTitle}>{fileName}</span>
      {/if}
      {#if fileFormat}
        <span class="num ml-1.5 shrink-0 tracking-wide text-subtle">{fileFormat}</span>
      {/if}
      {#if (fileName || fileFormat) && exifBits.length}
        <span class="mx-2 shrink-0 opacity-35" aria-hidden="true">·</span>
      {/if}
      {#each exifBits as bit, i}
        {#if i > 0}
          <span class="mx-1.5 shrink-0 opacity-35" aria-hidden="true">·</span>
        {/if}
        <span class="selectable shrink-0 {bit.mono ? 'num' : ''}">{bit.text}</span>
      {/each}
    {:else}
      <span class="truncate">{$statusMessage}</span>
    {/if}
  </div>

  <div class="flex min-w-0 items-center justify-end gap-3">
    {#if $photos.length}
      <span>
        <span class="num">{photoIndex ? `${photoIndex} / ` : ""}{$photos.length}</span>
        photos
      </span>
    {/if}
    <button
      class="cursor-pointer text-subtle transition-colors hover:text-fg"
      onclick={() => isBugReportOpen.set(true)}
    >
      Report Bug
    </button>
  </div>
</footer>
