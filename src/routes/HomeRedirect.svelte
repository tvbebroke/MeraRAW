<script lang="ts">
  import { onMount } from "svelte";
  import { push } from "svelte-spa-router";
  import { imageMeta, imageOpen } from "../stores/app";
  import { editorRoute, libraryRoute, workspace } from "../stores/workspace";
  import { isVideoPath } from "../lib/media";

  onMount(() => {
    const ws = workspace.get();
    const open = imageOpen.get();
    const vid = imageMeta.get()?.kind === "video" || isVideoPath(imageMeta.get()?.path);
    if (open && ((ws === "video" && vid) || (ws === "photo" && !vid))) {
      push(editorRoute(ws));
    } else {
      push(libraryRoute(ws));
    }
  });
</script>
