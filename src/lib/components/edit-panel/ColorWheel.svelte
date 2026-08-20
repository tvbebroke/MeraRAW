<script lang="ts">
  let {
    hue,
    sat,
    lum,
    onHueSat,
    onLum,
    onRelease,
    disabled = false,
  }: {
    hue: number;
    sat: number;
    lum: number;
    onHueSat: (hue: number, sat: number) => void;
    onLum: (lum: number) => void;
    onRelease?: () => void;
    disabled?: boolean;
  } = $props();

  let svgEl = $state<SVGSVGElement | null>(null);

  function polarFromEvent(e: PointerEvent) {
    const el = svgEl;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const cx = r.left + r.width / 2;
    const cy = r.top + r.height / 2;
    const dx = e.clientX - cx;
    const dy = e.clientY - cy;
    const rad = Math.atan2(dy, dx);
    let h = (rad * 180) / Math.PI;
    if (h < 0) h += 360;
    const maxR = r.width / 2 - 4;
    const dist = Math.min(Math.hypot(dx, dy) / maxR, 1);
    onHueSat(h, dist * 100);
  }

  function onDown(e: PointerEvent) {
    if (disabled) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    polarFromEvent(e);
  }
  function onMove(e: PointerEvent) {
    if (disabled || !(e.currentTarget as HTMLElement).hasPointerCapture(e.pointerId)) return;
    polarFromEvent(e);
  }

  const ang = $derived((hue * Math.PI) / 180);
  const rr = $derived(0.42 * Math.min(1, Math.abs(sat) / 100));
  const kx = $derived(0.5 + Math.cos(ang) * rr);
  const ky = $derived(0.5 + Math.sin(ang) * rr);
</script>

<div class="wheel-row">
  <svg
    bind:this={svgEl}
    class="wheel"
    class:off={disabled}
    viewBox="0 0 100 100"
    role="slider"
    aria-label="Color wheel (double-click to reset)"
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={Math.round(sat)}
    tabindex="0"
    onpointerdown={onDown}
    onpointermove={onMove}
    onpointerup={() => onRelease?.()}
    onpointercancel={() => onRelease?.()}
    ondblclick={() => {
      if (disabled) return;
      onHueSat(0, 0);
      onLum(0);
      onRelease?.();
    }}
  >
    <title>Double-click to reset</title>
    <defs>
      <radialGradient id="satfade" cx="50%" cy="50%" r="50%">
        <stop offset="0%" stop-color="#808080" />
        <stop offset="100%" stop-color="#808080" stop-opacity="0" />
      </radialGradient>
    </defs>
    <foreignObject x="2" y="2" width="96" height="96">
      <div
        xmlns="http://www.w3.org/1999/xhtml"
        class="conic"
      ></div>
    </foreignObject>
    <circle cx="50" cy="50" r="48" fill="url(#satfade)" />
    <circle cx={kx * 100} cy={ky * 100} r="4" fill="var(--color-fg)" stroke="var(--color-bg)" stroke-width="1.5" />
  </svg>
  <input
    class="lum"
    type="range"
    min="-100"
    max="100"
    step="1"
    value={lum}
    {disabled}
    oninput={(e) => onLum(Number((e.currentTarget as HTMLInputElement).value))}
    onchange={() => onRelease?.()}
    aria-label="Luminance"
  />
</div>

<style>
  .wheel-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 6px 0 8px;
  }
  .wheel {
    width: 88px;
    height: 88px;
    flex: none;
    border-radius: 50%;
    cursor: crosshair;
    touch-action: none;
  }
  .wheel.off { opacity: 0.4; pointer-events: none; }
  .conic {
    width: 96px;
    height: 96px;
    border-radius: 50%;
    background: conic-gradient(
      from 0deg,
      #ff2a2a,
      #ffff2a,
      #2aff2a,
      #2affff,
      #2a7fff,
      #9f2aff,
      #ff2aff,
      #ff2a2a
    );
  }
  .lum {
    writing-mode: vertical-lr;
    direction: rtl;
    width: 14px;
    height: 88px;
    accent-color: var(--color-fg);
  }
</style>
