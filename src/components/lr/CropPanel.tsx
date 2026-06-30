import { useCallback, useMemo } from "react";
import {
  applyCropParams,
  resetCropModule,
} from "../../crop/cropActions";
import { ASPECT_PRESETS } from "../../crop/cropConstants";
import {
  applyAspectToRect,
  aspectRatio,
  flipOrientation,
  readCropFromDoc,
  type CropParams,
} from "../../crop/cropMath";
import { useDocStore } from "../../state/docStore";
import { useUiStore } from "../../state/uiStore";
import { Panel, Slider, useParam } from "./widgets";

export function CropPanel() {
  const doc = useDocStore((s) => s.doc);
  const reconcile = useDocStore((s) => s.reconcile);
  const imageDims = useUiStore((s) => s.imageDims);
  const aspectPreset = useUiStore((s) => s.cropAspectPreset);
  const setAspectPreset = useUiStore((s) => s.setCropAspectPreset);
  const aspectLocked = useUiStore((s) => s.cropAspectLocked);
  const setAspectLocked = useUiStore((s) => s.setCropAspectLocked);
  const cropActive = useUiStore((s) => s.cropActive);
  const enterCrop = useUiStore((s) => s.enterCropTool);
  const exitCrop = useUiStore((s) => s.exitCropTool);

  const crop = useMemo(() => readCropFromDoc(doc?.modules), [doc?.modules]);
  const angleHandle = useParam({
    path: "crop.angle",
    ty: "f32",
    min: -45,
    max: 45,
    default: 0,
    ui: { label: "Angle", step: 0.1, scale: "linear", group: "Crop" },
  });

  const applyCrop = useCallback(
    (next: CropParams, live = false) =>
      applyCropParams(next, live).then(reconcile).catch(() => {}),
    [reconcile],
  );

  const setAspect = useCallback(
    (presetId: string) => {
      setAspectPreset(presetId);
      if (!imageDims) return;
      const preset = ASPECT_PRESETS.find((p) => p.id === presetId);
      if (!preset) return;
      let w = preset.w;
      let h = preset.h;
      if (presetId === "as-shot" || presetId === "original") {
        w = imageDims.w;
        h = imageDims.h;
        while (w > 100 || h > 100) {
          w /= 2;
          h /= 2;
        }
      }
      if (w <= 0 || h <= 0) return;
      const ratio = w / h;
      const next = {
        ...crop,
        aspectW: w,
        aspectH: h,
        aspectLocked: true,
        rect: applyAspectToRect(crop.rect, ratio, imageDims.w, imageDims.h),
      };
      void applyCrop(next);
    },
    [applyCrop, crop, imageDims, setAspectPreset],
  );

  const rotate90 = (dir: -1 | 1) => {
    void applyCrop({ ...crop, rotate90: (crop.rotate90 + dir + 4) % 4 });
  };

  const flipH = () => void applyCrop({ ...crop, flipH: !crop.flipH });
  const flipV = () => void applyCrop({ ...crop, flipV: !crop.flipV });

  const flipOrientationBtn = () => {
    const [w, h] = flipOrientation(crop.aspectW, crop.aspectH);
    if (w <= 0 || h <= 0 || !imageDims) return;
    const ratio = w / h;
    void applyCrop({
      ...crop,
      aspectW: w,
      aspectH: h,
      aspectLocked: true,
      rect: applyAspectToRect(crop.rect, ratio, imageDims.w, imageDims.h),
    });
  };

  const currentRatio =
    imageDims && crop.aspectW > 0 && crop.aspectH > 0
      ? `${crop.aspectW}:${crop.aspectH}`
      : imageDims
        ? aspectRatio(crop.rect, imageDims.w, imageDims.h).toFixed(2)
        : "—";

  return (
    <div className="lr-rail-panel">
      <header className="lr-rail-panel-head">
        <h2>Crop</h2>
        {!cropActive ? (
          <button type="button" className="lr-crop-enter" onClick={() => enterCrop()}>
            Start crop
          </button>
        ) : (
          <div className="lr-crop-head-actions">
            <button type="button" onClick={() => exitCrop(true)} title="Apply (Enter)">
              Done
            </button>
            <button type="button" onClick={() => exitCrop(false)} title="Cancel (Esc)">
              Cancel
            </button>
          </div>
        )}
      </header>
      <div className="lr-rail-panel-body">
        <Panel title="Aspect Ratio" defaultOpen>
          <div className="lr-crop-row">
            <button
              type="button"
              className={`lr-crop-lock${aspectLocked || crop.aspectLocked ? " is-active" : ""}`}
              title="Lock aspect (A)"
              onClick={() => {
                const next = !crop.aspectLocked;
                setAspectLocked(next);
                void applyCrop({ ...crop, aspectLocked: next });
              }}
            >
              {aspectLocked || crop.aspectLocked ? "🔒" : "🔓"}
            </button>
            <select
              className="lr-crop-aspect-select"
              value={aspectPreset}
              onChange={(e) => setAspect(e.target.value)}
            >
              {ASPECT_PRESETS.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label}
                </option>
              ))}
            </select>
            <button type="button" className="lr-crop-icon-btn" title="Flip orientation (X)" onClick={flipOrientationBtn}>
              ⇄
            </button>
          </div>
          <div className="muted sm">Ratio: {currentRatio}</div>
        </Panel>

        <Panel title="Straighten" defaultOpen>
          <Slider label="Angle" h={angleHandle} />
          <p className="muted sm">Drag outside the crop frame to rotate. Hold ⌘ and drag inside to straighten.</p>
        </Panel>

        <Panel title="Transform" defaultOpen={false}>
          <div className="lr-crop-btn-row">
            <button type="button" onClick={() => rotate90(-1)} title="Rotate left 90°">
              ↺ 90°
            </button>
            <button type="button" onClick={() => rotate90(1)} title="Rotate right 90°">
              ↻ 90°
            </button>
            <button type="button" onClick={flipH} title="Flip horizontal">
              ↔
            </button>
            <button type="button" onClick={flipV} title="Flip vertical">
              ↕
            </button>
          </div>
        </Panel>

        <Panel title="Guides" defaultOpen={false}>
          <p className="muted sm">
            Press <kbd>O</kbd> to cycle overlays, <kbd>Shift+O</kbd> to rotate asymmetric guides, <kbd>H</kbd> to hide.
          </p>
        </Panel>

        <div className="lr-crop-footer">
          <button
            type="button"
            className="lr-crop-reset"
            onClick={() => resetCropModule().then(reconcile).catch(() => {})}
          >
            Reset crop
          </button>
        </div>
      </div>
    </div>
  );
}
