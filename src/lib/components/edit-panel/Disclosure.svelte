<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    label,
    open = $bindable(false),
    children,
  }: { label: string; open?: boolean; children?: Snippet } = $props();
</script>

<button type="button" class="disclosure" onclick={() => (open = !open)}>
  <span class="chev" class:open aria-hidden="true">›</span>
  {label}
</button>
{#if open}
  <div class="disclosure-body">
    {@render children?.()}
  </div>
{/if}
