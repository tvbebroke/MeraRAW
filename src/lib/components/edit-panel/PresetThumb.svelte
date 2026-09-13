<script lang="ts">
  import {
    cachedPresetSnapshot,
    renderPostcardSnapshot,
    type PresetModules,
  } from "../../presets/snapshot";

  let {
    id,
    modules,
    photoUrl,
  }: {
    id: string;
    modules?: PresetModules;
    photoUrl: string | null;
  } = $props();

  let host: HTMLSpanElement | undefined = $state();
  let src = $state("");
  let visible = $state(false);

  // Instant postcard so the row never sits blank while the observer / photo loads.
  $effect(() => {
    const postcard = renderPostcardSnapshot(modules);
    if (postcard) src = postcard;
  });

  $effect(() => {
    const el = host;
    if (!el) return;
    let cancelled = false;
    const mark = () => {
      if (!cancelled) visible = true;
    };
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) mark();
      },
      { rootMargin: "160px" },
    );
    io.observe(el);
    // WKWebView / slide-open can miss the first IO callback.
    const t = window.setTimeout(mark, 80);
    return () => {
      cancelled = true;
      window.clearTimeout(t);
      io.disconnect();
    };
  });

  $effect(() => {
    if (!visible) return;
    const presetId = id;
    const url = photoUrl;
    const mods = modules;
    let cancelled = false;
    void cachedPresetSnapshot(presetId, mods, url)
      .then((next) => {
        if (!cancelled && next) src = next;
      })
      .catch(() => {
        /* postcard already shown */
      });
    return () => {
      cancelled = true;
    };
  });
</script>

<span bind:this={host} class="snap-wrap" aria-hidden="true">
  {#if src}
    <img class="snap" {src} alt="" width="80" height="50" />
  {:else}
    <span class="snap"></span>
  {/if}
</span>

<style>
  .snap-wrap {
    flex: none;
    width: 80px;
    height: 50px;
  }
  .snap {
    display: block;
    width: 80px;
    height: 50px;
    border-radius: 4px;
    object-fit: cover;
    background: var(--color-active);
  }
</style>
