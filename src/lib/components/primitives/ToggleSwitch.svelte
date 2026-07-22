<script lang="ts">
  let {
    checked = false,
    label,
    title,
    onchange,
  }: {
    checked?: boolean;
    label: string;
    title?: string;
    onchange?: (v: boolean) => void;
  } = $props();
</script>

<button
  role="switch"
  aria-checked={checked}
  aria-label={label}
  title={title ?? label}
  class="toggle-switch"
  class:checked
  onclick={() => onchange?.(!checked)}
>
  <span
    class="toggle-handle"
    class:checked
  ></span>
</button>

<style>
  .toggle-switch {
    appearance: none;
    -webkit-appearance: none;
    outline: none;
    border: 1px solid rgba(255, 255, 255, 0.08);
    padding: 0;
    margin: 0;
    position: relative;
    height: 20px;
    width: 36px;
    flex-shrink: 0;
    border-radius: 9999px;
    background: rgba(255, 255, 255, 0.06);
    cursor: pointer;
    transition: background 200ms ease, border-color 200ms ease;
  }

  .toggle-switch:hover {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.12);
  }

  .toggle-switch.checked {
    background: #ffffff;
    border-color: #ffffff;
  }

  .toggle-handle {
    position: absolute;
    top: 2px;
    left: 2px;
    height: 14px;
    width: 14px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.8);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
    transition: transform 200ms cubic-bezier(0.25, 1, 0.5, 1), background-color 200ms ease;
    transform: translateX(0);
  }

  .toggle-handle.checked {
    transform: translateX(18px);
    background: #111113;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.1);
  }
</style>
