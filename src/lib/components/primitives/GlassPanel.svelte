<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    variant = "liquid",
    class: cls = "",
    children,
    ...rest
  }: {
    variant?: "solid" | "glass" | "viewport" | "liquid";
    class?: string;
    children?: Snippet;
    [key: string]: any;
  } = $props();

  const variants = {
    solid: "bg-panel border border-white/[0.04] rounded-[22px]",
    glass:
      "bg-[rgba(20,20,22,0.6)] backdrop-blur-md border border-white/[0.04] rounded-[22px]",
    viewport: "bg-[#171717] border border-white/[0.04] rounded-[22px]",
    liquid: "relative rounded-[22px] transition-all duration-300 isolate"
  };
</script>

<div class="relative {variants[variant]} {cls}" {...rest}>
  {#if variant === 'liquid'}
    <!-- Clean, non-distorted glass backdrop optimized for WebKit and low-banding performance -->
    <div class="glass-backdrop absolute inset-0 rounded-[22px] pointer-events-none z-[-1]"></div>
  {/if}
  {@render children?.()}
</div>

<style>
  .glass-backdrop {
    position: absolute;
    inset: 0;
    z-index: -1;
    -webkit-backdrop-filter: blur(30px) saturate(140%);
    backdrop-filter: blur(30px) saturate(140%);
    
    /* Dark panel fill sitting inside the lighter charcoal window */
    background: var(--glass-bg, var(--color-panel, rgba(23, 23, 23, 0.78)));
      
    border: 1px solid var(--color-border-subtle, rgba(255, 255, 255, 0.05));
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.04),
      inset 0 -1px 0 rgba(0, 0, 0, 0.3),
      0 6px 20px rgba(0, 0, 0, 0.12);
  }
</style>
