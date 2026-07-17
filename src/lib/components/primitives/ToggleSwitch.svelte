<script lang="ts">
  let {
    checked = false,
    label,
    onchange,
  }: {
    checked?: boolean;
    label: string;
    onchange?: (v: boolean) => void;
  } = $props();
</script>

<button
  role="switch"
  aria-checked={checked}
  aria-label={label}
  title={label}
  class="toggle-switch"
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
    border: 1px solid rgba(255, 255, 255, 0.06);
    padding: 0;
    margin: 0;
    position: relative;
    height: 36px;
    width: 68px;
    flex-shrink: 0;
    border-radius: 9999px;
    background: rgba(33, 33, 35, 0.65);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      inset 0 -1px 0 rgba(0, 0, 0, 0.2),
      0 2px 6px rgba(0, 0, 0, 0.25);
    cursor: pointer;
    transition: background 150ms ease;
  }

  .toggle-switch:hover {
    background: rgba(47, 47, 49, 0.85);
  }

  .toggle-switch:active {
    background: rgba(65, 65, 68, 0.9);
  }

  .toggle-handle {
    position: absolute;
    top: 6px;
    left: 6px;
    height: 22px;
    width: 22px;
    border-radius: 50%;
    background: #d9d9d9;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
    transition: transform 250ms cubic-bezier(0.25, 1, 0.5, 1);
    transform: translateX(0);
  }

  .toggle-handle.checked {
    transform: translateX(32px);
  }
</style>

