// Lightroom-style UI primitives: collapsible Panel, Slider (label left,
// value right click-to-type, dbl-click reset, drag-scrub), inline icons.
// All edits flow through the registry → ops → doc-mirror reconcile loop.
import { useCallback, useEffect, useRef, useState } from "react";
import { setParam } from "../../ipc/commands";
import type { ImageMeta, ParamSpec } from "../../ipc/types";
import { docParam, useDocStore } from "../../state/docStore";
import { useUiStore } from "../../state/uiStore";

// ---------- inline SVG icons (our own, no Adobe assets) ----------
type IconProps = { size?: number };
const svg = (path: React.ReactNode, vb = 24) => (p: IconProps) => (
  <svg
    width={p.size ?? 16}
    height={p.size ?? 16}
    viewBox={`0 0 ${vb} ${vb}`}
    fill="none"
    stroke="currentColor"
    strokeWidth="1.8"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    {path}
  </svg>
);
export const Icon = {
  Chevron: svg(<polyline points="9 6 15 12 9 18" />),
  Crop: svg(
    <>
      <path d="M6 2v14a2 2 0 0 0 2 2h14" />
      <path d="M2 6h14a2 2 0 0 1 2 2v14" />
    </>,
  ),
  Mask: svg(
    <>
      <circle cx="9" cy="12" r="7" />
      <path d="M14 5a7 7 0 0 1 0 14" />
    </>,
  ),
  Eyedropper: svg(
    <>
      <path d="m12 22 1-1" />
      <path d="M5 17l8.5-8.5" />
      <path d="M16 6l2-2a2.8 2.8 0 0 0-4-4l-2 2" />
      <path d="M4 16l-1 4 4-1 9-9-3-3z" />
    </>,
  ),
  Brush: svg(
    <>
      <path d="M3 21c3 0 5-2 5-5" />
      <path d="M14 4l6 6-8 8H8v-4z" />
    </>,
  ),
  Sparkles: svg(
    <>
      <path d="M12 3v4M12 17v4M3 12h4M17 12h4" />
      <path d="m6 6 2 2M16 16l2 2M18 6l-2 2M8 16l-2 2" />
    </>,
  ),
  Wand: svg(
    <>
      <path d="M15 4V2M15 10V8M11 6H9M21 6h-2" />
      <path d="m3 21 12-12 2 2L5 23z" />
    </>,
  ),
  Compare: svg(
    <>
      <rect x="3" y="5" width="18" height="14" rx="1" />
      <path d="M12 5v14" />
      <path d="M7 9l-2 3 2 3M17 9l2 3-2 3" />
    </>,
  ),
  Loupe: svg(
    <>
      <circle cx="11" cy="11" r="7" />
      <path d="m21 21-4.3-4.3" />
    </>,
  ),
  Fit: svg(
    <>
      <path d="M3 8V3h5M21 8V3h-5M3 16v5h5M21 16v5h-5" />
    </>,
  ),
  Reset: svg(
    <>
      <path d="M3 12a9 9 0 1 0 3-6.7L3 8" />
      <path d="M3 3v5h5" />
    </>,
  ),
  Export: svg(
    <>
      <path d="M12 3v12" />
      <path d="m8 7 4-4 4 4" />
      <path d="M5 15v4a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-4" />
    </>,
  ),
  EditSliders: svg(
    <>
      <line x1="4" y1="7" x2="20" y2="7" />
      <circle cx="9" cy="7" r="2" fill="currentColor" stroke="none" />
      <line x1="4" y1="12" x2="20" y2="12" />
      <circle cx="15" cy="12" r="2" fill="currentColor" stroke="none" />
      <line x1="4" y1="17" x2="20" y2="17" />
      <circle cx="11" cy="17" r="2" fill="currentColor" stroke="none" />
    </>,
  ),
  Presets: svg(
    <>
      <circle cx="9" cy="12" r="6" />
      <circle cx="15" cy="12" r="6" />
    </>,
  ),
  Remove: svg(
    <>
      <path d="M12 3v6" />
      <path d="M8 9h8l-1 11H9L8 9z" />
      <path d="M9 3h6" />
    </>,
  ),
};

// ---------- collapsible LR panel ----------
export function Panel({
  title,
  defaultOpen = true,
  right,
  className,
  children,
}: {
  title: string;
  defaultOpen?: boolean;
  right?: React.ReactNode;
  className?: string;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <section className={`lr-panel ${className ?? ""}`}>
      <header className="lr-panel-head" onClick={() => setOpen((o) => !o)}>
        <span className={`lr-disclosure ${open ? "open" : ""}`}>
          <Icon.Chevron size={12} />
        </span>
        <span className="lr-panel-title">{title}</span>
        <span className="lr-panel-right" onClick={(e) => e.stopPropagation()}>
          {right}
        </span>
      </header>
      {open && <div className="lr-panel-body">{children}</div>}
    </section>
  );
}

// ---------- registry param resolution (mask-aware) ----------
export interface ParamHandle {
  value: number;
  default: number;
  min: number;
  max: number;
  step: number;
  setLive: (v: number) => void; // throttled live preview
  commit: (v: number) => void; // final committed value
  reset: () => void;
}

/** Resolve a registry param to a live, mask-scoped, op-dispatching handle. */
export function useParam(spec: ParamSpec, meta?: ImageMeta | null): ParamHandle {
  const doc = useDocStore((s) => s.doc);
  const reconcile = useDocStore((s) => s.reconcile);
  const selectedMask = useUiStore((s) => s.selectedMask);
  const dot = spec.path.indexOf(".");
  const module = spec.path.slice(0, dot);
  const param = spec.path.slice(dot + 1);
  const targetPath = selectedMask
    ? `mask.${selectedMask}.${spec.path}`
    : spec.path;

  const asShot =
    !selectedMask && module === "white_balance" && param === "temp"
      ? meta?.estimatedCct ?? undefined
      : undefined;
  const specDefault = typeof spec.default === "number" ? spec.default : 0;
  const fallback = asShot ?? specDefault;

  const maskVal = selectedMask
    ? (doc?.masks?.find((m) => m.id === selectedMask)?.modules?.[module]?.[
        param
      ] as number | undefined)
    : undefined;
  const docVal = selectedMask ? maskVal : docParam(doc, module, param);

  const [drag, setDrag] = useState<number | null>(null);
  const value = drag ?? docVal ?? fallback;
  /** Bumps on each outbound setParam; stale responses are ignored. */
  const opSeq = useRef(0);
  /** Latest value waiting to be sent while a request is in flight. */
  const pendingLive = useRef<number | null>(null);
  const inFlight = useRef(false);

  const sendParam = useCallback(
    (v: number, clearDragOnSuccess: boolean) => {
      const seq = ++opSeq.current;
      // live during the drag (coalesced to one undo step); commit on release
      return setParam(targetPath, v, !clearDragOnSuccess)
        .then((delta) => {
          if (seq !== opSeq.current) return;
          reconcile(delta);
          if (clearDragOnSuccess) setDrag(null);
        })
        .catch(() => {
          if (clearDragOnSuccess) setDrag(null);
        });
    },
    [targetPath, reconcile],
  );

  // In-flight coalescing: send the newest value immediately; while one is
  // outstanding, keep only the latest and fire it the moment the previous
  // resolves. No fixed throttle delay, never drops the final value, and the
  // send rate self-limits to whatever the engine can keep up with.
  const pumpLive = useCallback(() => {
    if (inFlight.current || pendingLive.current === null) return;
    const v = pendingLive.current;
    pendingLive.current = null;
    inFlight.current = true;
    void sendParam(v, false).finally(() => {
      inFlight.current = false;
      pumpLive();
    });
  }, [sendParam]);

  const setLive = useCallback(
    (v: number) => {
      setDrag(v);
      pendingLive.current = v;
      pumpLive();
    },
    [pumpLive],
  );
  const commit = useCallback(
    (v: number) => {
      pendingLive.current = null; // supersede any queued live value
      void sendParam(v, true);
    },
    [sendParam],
  );
  const reset = useCallback(() => commit(fallback), [commit, fallback]);

  return {
    value,
    default: fallback,
    min: spec.min,
    max: spec.max,
    step: spec.ui.step,
    setLive,
    commit,
    reset,
  };
}

// ---------- LR slider ----------
function fmt(v: number): string {
  if (Math.abs(v) >= 1000) return v.toFixed(0);
  if (Math.abs(v) >= 10) return v.toFixed(0);
  return Number.isInteger(v) ? v.toFixed(0) : v.toFixed(2);
}

export function Slider({
  label,
  h,
}: {
  label: string;
  h: ParamHandle;
}) {
  const [editing, setEditing] = useState<string | null>(null);

  return (
    <div className="lr-slider">
      <span
        className="lr-slider-label"
        onDoubleClick={h.reset}
        title="double-click to reset"
      >
        {label}
      </span>
      <input
        className="lr-slider-track"
        type="range"
        min={h.min}
        max={h.max}
        step={h.step}
        value={h.value}
        onChange={(e) => h.setLive(parseFloat(e.target.value))}
        onPointerUp={(e) => h.commit(parseFloat((e.target as HTMLInputElement).value))}
        onDoubleClick={h.reset}
      />
      {editing === null ? (
        <span
          className="lr-slider-value"
          onClick={() => setEditing(fmt(h.value))}
          title="click to type"
        >
          {fmt(h.value)}
        </span>
      ) : (
        <input
          className="lr-slider-input"
          autoFocus
          value={editing}
          onChange={(e) => setEditing(e.target.value)}
          onBlur={() => {
            const v = parseFloat(editing);
            if (!Number.isNaN(v)) h.commit(Math.min(h.max, Math.max(h.min, v)));
            setEditing(null);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
            if (e.key === "Escape") setEditing(null);
          }}
        />
      )}
    </div>
  );
}

/** Registry-driven slider by exact param path. Returns null if absent. */
export function ParamSlider({
  specs,
  path,
  label,
  meta,
}: {
  specs: ParamSpec[];
  path: string;
  label?: string;
  meta?: ImageMeta | null;
}) {
  const spec = specs.find((s) => s.path === path);
  // hooks must run unconditionally → resolve with a fallback spec
  const safe = spec ?? {
    path,
    ty: "f32" as const,
    min: 0,
    max: 1,
    default: 0,
    ui: { label: label ?? path, step: 0.01, scale: "linear", group: "" },
  };
  const h = useParam(safe, meta);
  if (!spec) return null;
  return <Slider label={label ?? spec.ui.label} h={h} />;
}

// re-export for convenience
export { useDocStore };
export type { ParamSpec };

/** Small hook: load the registry once. */
export function useRegistry(): ParamSpec[] {
  const [specs, setSpecs] = useState<ParamSpec[]>([]);
  useEffect(() => {
    import("../../ipc/commands").then(({ getRegistry }) =>
      getRegistry().then(setSpecs).catch(() => {}),
    );
  }, []);
  return specs;
}
