<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    variant = "liquid",
    class: cls = "",
    children,
    ...rest
  }: {
    variant?: "solid" | "glass" | "viewport" | "liquid" | "quiet";
    class?: string;
    children?: Snippet;
    [key: string]: any;
  } = $props();

  // One surface, one radius. Elevation comes from the 1px border,
  // not from blur or shadow. The `liquid` variant keeps its name for
  // call-site compatibility but now renders the flat shared
  // `.glass-backdrop` surface (see app.css), which only becomes
  // frosted under the opt-in `.glass-look` root class.
  const variants = {
    solid: "bg-panel border border-border rounded-[10px]",
    glass: "bg-panel border border-border rounded-[10px]",
    viewport: "bg-canvas",
    liquid: "relative isolate",
    quiet: "relative isolate bg-sidebar",
  };
</script>

<div class="relative {variants[variant]} {cls}" {...rest}>
  {#if variant === "liquid"}
    <div class="glass-backdrop pointer-events-none"></div>
  {/if}
  {@render children?.()}
</div>
