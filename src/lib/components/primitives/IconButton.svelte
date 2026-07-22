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
  aria-label={label}
  title={title ?? label}
  class="icon-btn {wide ? 'icon-btn--wide' : ''}"
>
  <img src={icon} alt="" class={iconClass} />
</button>

<style>
  .icon-btn {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.06);
    padding: 0;
    margin: 0;
    font: inherit;
    color: inherit;
    cursor: pointer;

    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;

    width: 36px;
    height: 36px;
    border-radius: 9999px;

    background: rgba(33, 33, 35, 0.65);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      inset 0 -1px 0 rgba(0, 0, 0, 0.2),
      0 2px 6px rgba(0, 0, 0, 0.25);
    transition: background 150ms cubic-bezier(0.4, 0, 0.2, 1), border-color 150ms cubic-bezier(0.4, 0, 0.2, 1);
  }

  .icon-btn--wide {
    width: 59px;
  }

  .icon-btn:hover {
    background: rgba(47, 47, 49, 0.85);
  }

  .icon-btn:active {
    background: rgba(65, 65, 68, 0.9);
  }
</style>
