// Thin toolbar under the viewport: zoom (fit / 1:1 / ± / %), display look,
// before/after, loupe. Drives the viewport via the uiStore view-command channel.
import { useState } from "react";
import { useUiStore } from "../../state/uiStore";
import { setDisplayLook } from "../../ipc/commands";
import { Icon } from "./widgets";

const LOOKS: [string, number, string][] = [
  ["Neutral", 0, "Flat scene-referred view"],
  ["Camera", 1, "Punchy JPEG-like view"],
  ["Filmic", 2, "AgX — filmic highlight rolloff"],
  ["Original", 4, "Demosaiced sensor data — no profile, edits, or tone mapping"],
];

export function ViewportToolbar() {
  const zoomLabel = useUiStore((s) => s.zoomLabel);
  const sendViewCmd = useUiStore((s) => s.sendViewCmd);
  const before = useUiStore((s) => s.beforeAfter);
  const setBefore = useUiStore((s) => s.setBeforeAfter);
  const [look, setLook] = useState(1); // matches engine default (Camera)

  function pickLook(l: number) {
    setLook(l);
    void setDisplayLook(l).catch(() => {});
  }

  return (
    <div className="vp-toolbar">
      <button title="Fit (⌘0)" onClick={() => sendViewCmd("fit")}>
        <Icon.Fit size={14} />
      </button>
      <button title="1:1" onClick={() => sendViewCmd("oneToOne")}>
        1:1
      </button>
      <button title="Zoom out" onClick={() => sendViewCmd("zoomOut")}>
        −
      </button>
      <span className="vp-zoom">{zoomLabel}</span>
      <button title="Zoom in" onClick={() => sendViewCmd("zoomIn")}>
        +
      </button>
      <div className="vp-spacer" />
      <div className="vp-look" title="Display look">
        {LOOKS.map(([label, l, tip]) => (
          <button
            key={l}
            className={look === l ? "active" : ""}
            title={tip}
            onClick={() => pickLook(l)}
          >
            {label}
          </button>
        ))}
      </div>
      <button
        className={before ? "active" : ""}
        title="Before / After (\)"
        onClick={() => setBefore(!before)}
      >
        <Icon.Compare size={14} /> {before ? "Before" : "After"}
      </button>
      <button title="Loupe" onClick={() => sendViewCmd("oneToOne")}>
        <Icon.Loupe size={14} />
      </button>
    </div>
  );
}
