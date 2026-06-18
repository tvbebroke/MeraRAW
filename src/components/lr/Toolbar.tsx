// Thin toolbar under the viewport: zoom (fit / 1:1 / ± / %), before/after,
// loupe. Drives the viewport via the uiStore view-command channel.
import { useUiStore } from "../../state/uiStore";
import { Icon } from "./widgets";

export function ViewportToolbar() {
  const zoomLabel = useUiStore((s) => s.zoomLabel);
  const sendViewCmd = useUiStore((s) => s.sendViewCmd);
  const before = useUiStore((s) => s.beforeAfter);
  const setBefore = useUiStore((s) => s.setBeforeAfter);

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
