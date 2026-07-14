// Preview backdrop picker — used in Settings and as a compact topbar control.
import { useEffect, useRef, useState } from "react";
import { useUiStore } from "../state/uiStore";
import { VIEWPORT_BG_OPTIONS, type ViewportBg } from "../theme/viewportBackground";
import { Icon } from "./lr/widgets";
import { PsychedelicControls } from "./PsychedelicControls";

function SwatchGrid({
  value,
  onChange,
}: {
  value: ViewportBg;
  onChange: (id: ViewportBg) => void;
}) {
  return (
    <div className="vp-bg-grid" role="listbox" aria-label="Preview background">
      {VIEWPORT_BG_OPTIONS.map((opt) => (
        <button
          key={opt.id}
          type="button"
          role="option"
          aria-selected={value === opt.id}
          className={`vp-bg-option ${value === opt.id ? "active" : ""}`}
          title={`${opt.label} — ${opt.hint}`}
          onClick={() => onChange(opt.id)}
        >
          <span
            className="vp-bg-swatch"
            style={{ background: opt.swatch }}
            aria-hidden="true"
          />
          <span className="vp-bg-label">{opt.label}</span>
        </button>
      ))}
    </div>
  );
}

/** Full control block for the Settings dialog. */
export function ViewportBgSettings() {
  const viewportBg = useUiStore((s) => s.viewportBg);
  const setViewportBg = useUiStore((s) => s.setViewportBg);
  return (
    <section className="settings-section">
      <h3 className="settings-h">App background</h3>
      <p className="muted sm">
        Chrome behind panels. Transparent / liquid glass show your desktop;
        Psychedelic plays a looping video with audio. Photo and controls stay
        solid.
      </p>
      <SwatchGrid value={viewportBg} onChange={setViewportBg} />
      {viewportBg === "psychedelic" && (
        <div className="psy-controls-block">
          <div className="vp-bg-popover-title">Background video</div>
          <PsychedelicControls />
        </div>
      )}
    </section>
  );
}

/** Compact topbar button with popover. */
export function ViewportBgMenuButton() {
  const viewportBg = useUiStore((s) => s.viewportBg);
  const setViewportBg = useUiStore((s) => s.setViewportBg);
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open]);

  return (
    <div className="vp-bg-menu" ref={rootRef}>
      <button
        type="button"
        className={`topbar-icon-btn ${open ? "active" : ""}`}
        title="App background"
        aria-expanded={open}
        aria-haspopup="listbox"
        onClick={() => setOpen((o) => !o)}
      >
        <Icon.Backdrop size={16} />
      </button>
      {open && (
        <div className="vp-bg-popover">
          <div className="vp-bg-popover-title">App background</div>
          <SwatchGrid
            value={viewportBg}
            onChange={(id) => {
              setViewportBg(id);
              if (id !== "psychedelic") setOpen(false);
            }}
          />
          {viewportBg === "psychedelic" && (
            <div className="psy-controls-block">
              <div className="vp-bg-popover-title">Background video</div>
              <PsychedelicControls />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
