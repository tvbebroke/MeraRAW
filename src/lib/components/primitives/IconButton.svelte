<script lang="ts">
  let {
    icon,
    label,
    title,
    iconClass = "size-[20px]",
    wide = false,
    onclick,
  }: {
    icon: string;
    label: string;
    title?: string;
    iconClass?: string;
    wide?: boolean;
    onclick?: () => void;
  } = $props();
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<button
  {onclick}
  data-tauri-drag-region="false"
  aria-label={label}
  title={title ?? label}
  class="icon-btn {wide ? 'icon-btn--wide' : ''}"
>
  <img src={icon} alt="" class="{iconClass} pointer-events-none" />
</button>

<style>
  /* Flat square-ish icon button: transparent at rest, hover wash on
     interaction. No blur, no inset highlights, no drop shadow. */
  .icon-btn {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid transparent;
    padding: 0;
    margin: 0;
    font: inherit;
    color: inherit;
    cursor: pointer;

    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;

    width: 28px;
    height: 28px;
    border-radius: 6px;

    background: transparent;
    transition: background-color 0.15s var(--ease-std), border-color 0.15s var(--ease-std);
  }

  .icon-btn--wide {
    width: 48px;
  }

  .icon-btn:hover {
    background: var(--color-hover);
  }

  .icon-btn:active {
    background: var(--color-active);
  }
</style>
