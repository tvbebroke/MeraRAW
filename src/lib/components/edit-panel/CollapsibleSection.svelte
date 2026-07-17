<script lang="ts">
  import type { Snippet } from "svelte";
  import { slide } from "svelte/transition";
  import {
    openSections,
    toggleSection,
    type SectionId,
  } from "../../../stores/editor";
  import chevron from "../../icons/chevron-circle.svg";
  import chevronOpen from "../../icons/chevron-circle-open.svg";

  let {
    id,
    title,
    children,
  }: { id: SectionId; title: string; children?: Snippet } = $props();

  const open = $derived($openSections[id]);
</script>

<section class="w-full shrink-0 rounded-[22px] bg-[#171717] border border-white/[0.04] shadow-inner transition-all duration-300">
  <button
    class="grid h-[28px] w-full cursor-pointer grid-cols-[26px_1fr_26px] items-center px-[10px] focus:outline-none"
    onclick={() => toggleSection(id)}
    aria-expanded={open}
  >
    <span></span>
    <span class="text-center text-[10px] font-medium text-white/90">{title}</span>
    <img
      src={open ? chevronOpen : chevron}
      alt=""
      class="h-[18px] w-[26px] justify-self-end opacity-60 hover:opacity-100 transition-opacity"
    />
  </button>
  {#if open}
    <div class="px-[14px] pt-[6px] pb-[14px]" transition:slide={{ duration: 150 }}>
      {@render children?.()}
    </div>
  {/if}
</section>
