import type { ReactElement } from "react";
import type { ImageMeta } from "../../ipc/types";
import { useUiStore } from "../../state/uiStore";
import { MasksPanel } from "../MasksPanel";
import { CropPanel } from "./CropPanel";
import { DevelopRail } from "./DevelopRail";
import { PresetsPanel } from "./PresetsPanel";
import { Icon } from "./widgets";

export type RightToolTab = "presets" | "edit" | "crop" | "remove" | "masking";

type TabDef = {
  id: RightToolTab;
  label: string;
  icon: (p: { size?: number }) => ReactElement;
};

/** Modern (Figma) tool strip order. */
const TABS_MODERN: TabDef[] = [
  { id: "edit", label: "Edit", icon: Icon.EditSliders },
  { id: "crop", label: "Crop", icon: Icon.Crop },
  { id: "masking", label: "Mask", icon: Icon.Mask },
  { id: "remove", label: "AI", icon: Icon.Sparkles },
  { id: "presets", label: "Presets", icon: Icon.Presets },
];

/** Faithful (Lightroom Classic / v0.1.4) tool strip order. */
const TABS_FAITHFUL: TabDef[] = [
  { id: "presets", label: "Presets", icon: Icon.Presets },
  { id: "edit", label: "Edit", icon: Icon.EditSliders },
  { id: "crop", label: "Crop", icon: Icon.Crop },
  { id: "remove", label: "Remove", icon: Icon.Remove },
  { id: "masking", label: "Masking", icon: Icon.Mask },
];

function RemovePanel({ modern }: { modern: boolean }) {
  return (
    <div className="lr-rail-panel">
      <header className="lr-rail-panel-head">
        <h2>{modern ? "AI / Remove" : "Remove"}</h2>
      </header>
      <div className="lr-rail-panel-body">
        <p className="muted sm">
          Spot removal and healing brush — coming soon. Press <kbd>Q</kbd> to open this panel.
        </p>
      </div>
    </div>
  );
}

function MaskingPanel() {
  const tool = useUiStore((s) => s.tool);
  const setTool = useUiStore((s) => s.setTool);

  return (
    <div className="lr-rail-panel">
      <header className="lr-rail-panel-head">
        <h2>Masking</h2>
      </header>
      <div className="lr-rail-panel-body">
        <div className="masking-tools">
          <button
            type="button"
            className={tool === "brush" ? "active" : ""}
            title="Brush — paint the selected mask (K)"
            onClick={() => setTool(tool === "brush" ? "pan" : "brush")}
          >
            <Icon.Brush size={16} /> Brush
          </button>
        </div>
        <MasksPanel />
      </div>
    </div>
  );
}

export function RightRail({
  meta,
  onMetaChange,
}: {
  meta: ImageMeta | null;
  onMetaChange?: (m: ImageMeta) => void;
}) {
  const tab = useUiStore((s) => s.rightRailTab);
  const setTab = useUiStore((s) => s.setRightRailTab);
  const showRightPanel = useUiStore((s) => s.showRightPanel);
  const uiShell = useUiStore((s) => s.uiShell);
  const modern = uiShell === "modern";
  const tabs = modern ? TABS_MODERN : TABS_FAITHFUL;

  if (!showRightPanel) return null;

  const toolNav = (
    <nav className="lr-tool-tabs" aria-label="Develop tools">
      {tabs.map(({ id, label, icon: TabIcon }) => (
        <button
          key={id}
          type="button"
          className={`lr-tool-tab ${tab === id ? "active" : ""}`}
          title={label}
          onClick={() => setTab(id)}
        >
          <TabIcon size={modern ? 22 : 18} />
          <span>{label}</span>
        </button>
      ))}
    </nav>
  );

  const main = (
    <div className="lr-right-main">
      {tab === "presets" && <PresetsPanel />}
      {tab === "edit" && <DevelopRail meta={meta} onMetaChange={onMetaChange} />}
      {tab === "crop" && <CropPanel />}
      {tab === "remove" && <RemovePanel modern={modern} />}
      {tab === "masking" && <MaskingPanel />}
    </div>
  );

  // Modern: horizontal tools above panels. Faithful: vertical wrap, tools below.
  if (modern) {
    return (
      <div className="lr-right-stack">
        {toolNav}
        {main}
      </div>
    );
  }

  return (
    <div className="lr-right-wrap">
      {main}
      {toolNav}
    </div>
  );
}
