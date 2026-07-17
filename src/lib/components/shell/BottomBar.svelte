<script lang="ts">
  import { statusMessage } from "../../../stores/app";
  import { activePhoto, folder, photos } from "../../../stores/browse";

  const photoIndex = $derived(
    $activePhoto ? $photos.findIndex((p) => p.path === $activePhoto!.path) + 1 : 0,
  );
</script>

<footer
  class="col-span-full flex h-[20px] items-center justify-between gap-3 px-2 text-[9px] text-white/35"
>
  <span class="truncate">{$folder ?? ""}</span>
  <span class="shrink-0 truncate max-w-[40%] text-center">{$statusMessage}</span>
  <span class="shrink-0">
    {#if $photos.length}
      {photoIndex ? `${photoIndex} / ` : ""}{$photos.length} photos
    {/if}
  </span>
</footer>
