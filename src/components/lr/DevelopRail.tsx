// Right rail — Lightroom-order develop panels. Controls are registry-driven
// (ParamSlider resolves by path; renders nothing if a param is absent).
import { useState } from "react";
import type { ImageMeta } from "../../ipc/types";
import { useUiStore } from "../../state/uiStore";
import { Histogram } from "../Histogram";
import { MasksPanel } from "../MasksPanel";
import { AiGrader } from "./AiGrader";
import { ColorGrading } from "./ColorGrading";
import { HslPicker } from "./HslPicker";
import { ToneCurve } from "./ToneCurve";
import { Icon, Panel, ParamSlider, useRegistry } from "./widgets";

function ToolStrip() {
  const tool = useUiStore((s) => s.tool);
  const setTool = useUiStore((s) => s.setTool);
  const [masking, setMasking] = useState(false);

  return (
    <div className="tool-strip-wrap">
      <div className="tool-strip">
        <button title="Crop (R) — coming soon" disabled>
          <Icon.Crop size={16} />
        </button>
        <button
          className={masking ? "active" : ""}
          title="Masking (M)"
          onClick={() => setMasking((m) => !m)}
        >
          <Icon.Mask size={16} />
        </button>
        <button
          className={tool === "wb" ? "active" : ""}
          title="White balance eyedropper"
          onClick={() => setTool(tool === "wb" ? "pan" : "wb")}
        >
          <Icon.Eyedropper size={16} />
        </button>
        <button
          className={tool === "brush" ? "active" : ""}
          title="Brush (paint the selected brush mask)"
          onClick={() => setTool(tool === "brush" ? "pan" : "brush")}
        >
          <Icon.Brush size={16} />
        </button>
      </div>
      {masking && (
        <div className="tool-strip-body">
          <MasksPanel />
        </div>
      )}
    </div>
  );
}

function Basic({ meta }: { meta: ImageMeta | null }) {
  const specs = useRegistry();
  const tool = useUiStore((s) => s.tool);
  const setTool = useUiStore((s) => s.setTool);
  return (
    <Panel title="Basic">
      <div className="lr-subhead">
        <span>White Balance</span>
        <button
          className={`lr-mini-btn ${tool === "wb" ? "active" : ""}`}
          title="Eyedropper: click a neutral area"
          onClick={() => setTool(tool === "wb" ? "pan" : "wb")}
        >
          <Icon.Eyedropper size={12} />
        </button>
      </div>
      <ParamSlider specs={specs} path="white_balance.temp" label="Temp" meta={meta} />
      <ParamSlider specs={specs} path="white_balance.tint" label="Tint" meta={meta} />
      {meta?.cameraProfile ? (
        <div className="lr-profile-badge muted" title="Autoloaded camera color profile">
          Profile: {meta.cameraProfile}
        </div>
      ) : meta ? (
        <div className="lr-profile-badge muted">
          No camera profile for {meta.cameraMake} {meta.cameraModel}
        </div>
      ) : null}
      <div className="lr-subhead">
        <span>Tone</span>
      </div>
      <ParamSlider specs={specs} path="exposure.stops" label="Exposure" meta={meta} />
      <ParamSlider specs={specs} path="tone_curve.contrast" label="Contrast" meta={meta} />
      <ParamSlider specs={specs} path="tone_curve.highlights" label="Highlights" meta={meta} />
      <ParamSlider specs={specs} path="tone_curve.shadows" label="Shadows" meta={meta} />
      <ParamSlider specs={specs} path="tone_curve.lights" label="Whites" meta={meta} />
      <ParamSlider specs={specs} path="tone_curve.darks" label="Blacks" meta={meta} />
      <div className="lr-subhead">
        <span>Presence</span>
      </div>
      <ParamSlider specs={specs} path="color_grade.perceptual_sat" label="Vibrance" meta={meta} />
      <ParamSlider specs={specs} path="color_grade.global_chroma" label="Saturation" meta={meta} />
    </Panel>
  );
}

function GroupPanel({
  title,
  group,
  meta,
  defaultOpen = false,
}: {
  title: string;
  group: string;
  meta: ImageMeta | null;
  defaultOpen?: boolean;
}) {
  const specs = useRegistry();
  const inGroup = specs.filter((s) => s.ui.group === group && s.ty === "f32");
  if (inGroup.length === 0) return null;
  return (
    <Panel title={title} defaultOpen={defaultOpen}>
      {inGroup.map((s) => (
        <ParamSlider key={s.path} specs={specs} path={s.path} meta={meta} />
      ))}
    </Panel>
  );
}

export function DevelopRail({ meta }: { meta: ImageMeta | null }) {
  const specs = useRegistry();
  return (
    <div className="lr-right">
      <div className="lr-histogram-pin">
        <Histogram />
      </div>
      <ToolStrip />
      <Panel title="AI Color Grader" className="ai-panel">
        <AiGrader />
      </Panel>
      <Basic meta={meta} />
      <Panel title="Tone Curve" defaultOpen={false}>
        <ToneCurve />
        <div className="lr-divider" />
        <ParamSlider specs={specs} path="tone_curve.highlights" label="Highlights" meta={meta} />
        <ParamSlider specs={specs} path="tone_curve.lights" label="Lights" meta={meta} />
        <ParamSlider specs={specs} path="tone_curve.darks" label="Darks" meta={meta} />
        <ParamSlider specs={specs} path="tone_curve.shadows" label="Shadows" meta={meta} />
      </Panel>
      <Panel title="HSL / Color" defaultOpen={false}>
        <HslPicker specs={specs} meta={meta} />
      </Panel>
      <Panel title="Color Grading" defaultOpen={false}>
        <ColorGrading specs={specs} meta={meta} />
      </Panel>
      <GroupPanel title="Detail" group="Detail" meta={meta} />
      <GroupPanel title="Calibration" group="Calibration" meta={meta} />
    </div>
  );
}
