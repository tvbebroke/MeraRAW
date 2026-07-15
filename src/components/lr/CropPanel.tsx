import { useCallback, useMemo, useState } from "react";
import { applyCropParams, resetCropModule } from "../../crop/cropActions";
import { copyCropToClipboard, readCropClipboard } from "../../crop/cropClipboard";
import {
  ASPECT_PRESETS,
  formatRatio,
  parseRatioInput,
  type CropOverlayKind,
} from "../../crop/cropConstants";
import {
  applyAspectToRect,
  aspectRatio,
  constrainRectToImage,
  flipOrientation,
  maxCenteredRect,
  readCropFromDoc,
  rotatedDims,
  type CropParams,
} from "../../crop/cropMath";
import { GUIDES } from "../../crop/guides";
import { autoLevel } from "../../ipc/commands";
import { useDocStore } from "../../state/docStore";
import { useUiStore } from "../../state/uiStore";
import { Panel, Slider, type ParamHandle } from "./widgets";

export function CropPanel() {
  const doc = useDocStore((s) => s.doc);
  const reconcile = useDocStore((s) => s.reconcile);
  const imageDims = useUiStore((s) => s.imageDims);
  const aspectPreset = useUiStore((s) => s.cropAspectPreset);
  const setAspectPreset = useUiStore((s) => s.setCropAspectPreset);
  const cropActive = useUiStore((s) => s.cropActive);
  const enterCrop = useUiStore((s) => s.enterCropTool);
  const exitCrop = useUiStore((s) => s.exitCropTool);
  const customRatios = useUiStore((s) => s.cropCustomRatios);
  const rememberCustomRatio = useUiStore((s) => s.rememberCustomRatio);
  const guideMode = useUiStore((s) => s.cropGuideMode);
  const setGuideMode = useUiStore((s) => s.setCropGuideMode);
  const overlayKind = useUiStore((s) => s.cropOverlay);
  const guideColor = useUiStore((s) => s.cropGuideColor);
  const setGuideColor = useUiStore((s) => s.setCropGuideColor);
  const guideOpacity = useUiStore((s) => s.cropGuideOpacity);
  const setGuideOpacity = useUiStore((s) => s.setCropGuideOpacity);
  const gridSize = useUiStore((s) => s.cropGridSize);
  const setGridSize = useUiStore((s) => s.setCropGridSize);
  const aspectPreview = useUiStore((s) => s.cropAspectPreviewRatios);
  const setAspectPreview = useUiStore((s) => s.setCropAspectPreviewRatios);
  const maskOpacity = useUiStore((s) => s.cropMaskOpacity);
  const setMaskOpacity = useUiStore((s) => s.setCropMaskOpacity);
  const maskColor = useUiStore((s) => s.cropMaskColor);
  const setMaskColor = useUiStore((s) => s.setCropMaskColor);
  const ppi = useUiStore((s) => s.cropPpi);
  const setPpi = useUiStore((s) => s.setCropPpi);
  const setPreviousAspect = useUiStore((s) => s.setCropPreviousAspect);

  const [customInput, setCustomInput] = useState<string | null>(null);

  const crop = useMemo(() => readCropFromDoc(doc?.modules), [doc?.modules]);

  const applyCrop = useCallback(
    (next: CropParams, live = false) =>
      applyCropParams(next, live).then(reconcile).catch(() => {}),
    [reconcile],
  );

  const constrained = useCallback(
    (p: CropParams): CropParams => {
      if (!p.constrainCrop || !imageDims) return p;
      return {
        ...p,
        rect: constrainRectToImage(p.rect, p, imageDims.w, imageDims.h),
      };
    },
    [imageDims],
  );

  // ---- aspect ratio ----

  const applyRatio = useCallback(
    (w: number, h: number) => {
      if (!imageDims || w <= 0 || h <= 0) return;
      const ratio = w / h;
      const next = constrained({
        ...crop,
        aspectW: w,
        aspectH: h,
        aspectLocked: true,
        rect: applyAspectToRect(
          crop.rect,
          ratio,
          ...rotatedDims(crop, imageDims.w, imageDims.h),
        ),
      });
      setPreviousAspect(w, h);
      void applyCrop(next);
    },
    [applyCrop, constrained, crop, imageDims, setPreviousAspect],
  );

  const setAspect = useCallback(
    (presetId: string) => {
      if (presetId === "custom") {
        setCustomInput("");
        return;
      }
      setAspectPreset(presetId);
      if (!imageDims) return;
      if (presetId.startsWith("recent:")) {
        const parsed = parseRatioInput(presetId.slice(7));
        if (parsed) applyRatio(parsed[0], parsed[1]);
        return;
      }
      const preset = ASPECT_PRESETS.find((p) => p.id === presetId);
      if (!preset) return;
      if (preset.guide) {
        useUiStore.setState({ cropOverlay: preset.guide, cropOverlayVisible: true });
      }
      if (presetId === "free") {
        void applyCrop({ ...crop, aspectLocked: false, aspectW: 0, aspectH: 0 });
        return;
      }
      let w = preset.w;
      let h = preset.h;
      if (presetId === "as-shot") {
        // sensor ratio as displayed (orientation-aware)
        [w, h] = rotatedDims(crop, imageDims.w, imageDims.h);
      } else if (presetId === "original") {
        // ratio of the current crop (restores a prior crop's proportions)
        const [rw, rh] = rotatedDims(crop, imageDims.w, imageDims.h);
        w = (crop.rect.right - crop.rect.left) * rw;
        h = (crop.rect.bottom - crop.rect.top) * rh;
      }
      while (w > 1000 || h > 1000) {
        w /= 10;
        h /= 10;
      }
      if (w <= 0 || h <= 0) return;
      applyRatio(w, h);
    },
    [applyCrop, applyRatio, crop, imageDims, setAspectPreset],
  );

  const submitCustom = useCallback(() => {
    if (customInput === null) return;
    const parsed = parseRatioInput(customInput);
    setCustomInput(null);
    if (!parsed) return;
    const label = formatRatio(parsed[0], parsed[1]);
    rememberCustomRatio(label);
    setAspectPreset(`recent:${label}`);
    applyRatio(parsed[0], parsed[1]);
  }, [applyRatio, customInput, rememberCustomRatio, setAspectPreset]);

  // ---- straighten ----

  const applyAngle = useCallback(
    (angle: number, live: boolean) => {
      const a = Math.max(-45, Math.min(45, angle));
      void applyCrop(constrained({ ...crop, angle: a }), live);
    },
    [applyCrop, constrained, crop],
  );

  const angleHandle: ParamHandle = {
    value: crop.angle,
    default: 0,
    min: -45,
    max: 45,
    step: 0.05,
    setLive: (v) => applyAngle(v, true),
    commit: (v) => applyAngle(v, false),
    reset: () => applyAngle(0, false),
  };

  const [leveling, setLeveling] = useState(false);
  const runAutoLevel = useCallback(() => {
    setLeveling(true);
    autoLevel()
      .then((dOrig) => {
        if (Math.abs(dOrig) < 0.01) return;
        // fold the original-space deviation into display space: each flip and
        // an odd rotate-90 mirror the angle
        const parity =
          (crop.rotate90 % 2 === 1 ? -1 : 1) * (crop.flipH ? -1 : 1) * (crop.flipV ? -1 : 1);
        applyAngle(-parity * dOrig, false);
      })
      .catch(() => {})
      .finally(() => setLeveling(false));
  }, [applyAngle, crop.flipH, crop.flipV, crop.rotate90]);

  // ---- discrete transforms ----

  const rotate90 = (dir: -1 | 1) => {
    void applyCrop({ ...crop, rotate90: (crop.rotate90 + dir + 4) % 4 });
  };
  const flipH = () => void applyCrop({ ...crop, flipH: !crop.flipH });
  const flipV = () => void applyCrop({ ...crop, flipV: !crop.flipV });

  const flipOrientationBtn = () => {
    const [w, h] = flipOrientation(crop.aspectW, crop.aspectH);
    if (w <= 0 || h <= 0 || !imageDims) return;
    setPreviousAspect(w, h);
    void applyCrop(
      constrained({
        ...crop,
        aspectW: w,
        aspectH: h,
        aspectLocked: true,
        rect: applyAspectToRect(
          crop.rect,
          w / h,
          ...rotatedDims(crop, imageDims.w, imageDims.h),
        ),
      }),
    );
  };

  // ---- numeric fields (rotated-space px) ----

  const [rw, rh] = imageDims ? rotatedDims(crop, imageDims.w, imageDims.h) : [0, 0];
  const px = {
    x: Math.round(crop.rect.left * rw),
    y: Math.round(crop.rect.top * rh),
    w: Math.round((crop.rect.right - crop.rect.left) * rw),
    h: Math.round((crop.rect.bottom - crop.rect.top) * rh),
  };

  const setNumeric = (field: "x" | "y" | "w" | "h", value: number) => {
    if (!imageDims || !Number.isFinite(value)) return;
    const r = { ...crop.rect };
    const cw = r.right - r.left;
    const ch = r.bottom - r.top;
    if (field === "x") {
      const nx = Math.max(0, Math.min(1 - cw, value / rw));
      r.left = nx;
      r.right = nx + cw;
    } else if (field === "y") {
      const ny = Math.max(0, Math.min(1 - ch, value / rh));
      r.top = ny;
      r.bottom = ny + ch;
    } else if (field === "w") {
      const nw = Math.max(0.02, Math.min(1 - r.left, value / rw));
      r.right = r.left + nw;
      if (crop.aspectLocked && crop.aspectW > 0) {
        const nh = (nw * rw / (crop.aspectW / crop.aspectH)) / rh;
        r.bottom = Math.min(1, r.top + nh);
      }
    } else {
      const nh = Math.max(0.02, Math.min(1 - r.top, value / rh));
      r.bottom = r.top + nh;
      if (crop.aspectLocked && crop.aspectW > 0) {
        const nw = (nh * rh * (crop.aspectW / crop.aspectH)) / rw;
        r.right = Math.min(1, r.left + nw);
      }
    }
    void applyCrop(constrained({ ...crop, rect: r }));
  };

  /** Margin % of image (darktable-style): L/R/T/B as inset from each edge. */
  const margins = {
    left: Math.round(crop.rect.left * 1000) / 10,
    right: Math.round((1 - crop.rect.right) * 1000) / 10,
    top: Math.round(crop.rect.top * 1000) / 10,
    bottom: Math.round((1 - crop.rect.bottom) * 1000) / 10,
  };

  const setMargin = (edge: "left" | "right" | "top" | "bottom", pct: number) => {
    if (!imageDims || !Number.isFinite(pct)) return;
    const v = Math.max(0, Math.min(98, pct)) / 100;
    const r = { ...crop.rect };
    if (edge === "left") {
      r.left = Math.min(v, r.right - 0.02);
    } else if (edge === "right") {
      r.right = Math.max(1 - v, r.left + 0.02);
    } else if (edge === "top") {
      r.top = Math.min(v, r.bottom - 0.02);
    } else {
      r.bottom = Math.max(1 - v, r.top + 0.02);
    }
    void applyCrop(constrained({ ...crop, aspectLocked: false, rect: r }));
  };

  const currentRatio =
    crop.aspectW > 0 && crop.aspectH > 0
      ? formatRatio(crop.aspectW, crop.aspectH)
      : imageDims
        ? aspectRatio(crop.rect, rw, rh).toFixed(2)
        : "—";

  const printSize =
    ppi > 0 && px.w > 0
      ? `${(px.w / ppi).toFixed(1)} × ${(px.h / ppi).toFixed(1)} in · ${((px.w / ppi) * 2.54).toFixed(1)} × ${((px.h / ppi) * 2.54).toFixed(1)} cm`
      : null;

  const pasteCrop = () => {
    const p = readCropClipboard();
    if (!p) return;
    void applyCrop(constrained({ ...p, constrainCrop: crop.constrainCrop }));
  };

  const fitLargest = () => {
    if (!imageDims) return;
    const ratio =
      crop.aspectW > 0 && crop.aspectH > 0
        ? crop.aspectW / crop.aspectH
        : (crop.rect.right - crop.rect.left) * rw /
          Math.max((crop.rect.bottom - crop.rect.top) * rh, 1e-6);
    void applyCrop({
      ...crop,
      rect: maxCenteredRect(crop, imageDims.w, imageDims.h, ratio),
    });
  };

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
              className={`lr-crop-lock${crop.aspectLocked ? " is-active" : ""}`}
              title="Lock aspect (A)"
              onClick={() => void applyCrop({ ...crop, aspectLocked: !crop.aspectLocked })}
            >
              {crop.aspectLocked ? "🔒" : "🔓"}
            </button>
            <select
              className="lr-crop-aspect-select"
              value={aspectPreset}
              onChange={(e) => setAspect(e.target.value)}
            >
              {ASPECT_PRESETS.filter((p) => p.id !== "custom").map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label}
                </option>
              ))}
              {customRatios.map((r) => (
                <option key={`recent:${r}`} value={`recent:${r}`}>
                  {r} (recent)
                </option>
              ))}
              <option value="custom">Enter Custom…</option>
            </select>
            <button
              type="button"
              className="lr-crop-icon-btn"
              title="Flip orientation (X)"
              onClick={flipOrientationBtn}
            >
              ⇄
            </button>
          </div>
          {customInput !== null && (
            <div className="lr-crop-row">
              <input
                autoFocus
                placeholder="e.g. 7:5 or 1.91"
                value={customInput}
                onChange={(e) => setCustomInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") submitCustom();
                  if (e.key === "Escape") setCustomInput(null);
                }}
                onBlur={submitCustom}
                style={{ flex: 1 }}
              />
            </div>
          )}
          <div className="muted sm">Ratio: {currentRatio}</div>
        </Panel>

        <Panel title="Straighten" defaultOpen>
          <Slider label="Angle" h={angleHandle} />
          <div className="lr-crop-btn-row">
            <button
              type="button"
              onClick={runAutoLevel}
              disabled={leveling || !cropActive}
              title="Detect the dominant horizon/vertical and level it"
            >
              {leveling ? "…" : "Auto"}
            </button>
            <button type="button" onClick={() => applyAngle(0, false)} title="Reset angle (0)">
              0°
            </button>
            <label className="muted sm" style={{ display: "flex", alignItems: "center", gap: 4 }}>
              <input
                type="checkbox"
                checked={crop.constrainCrop}
                onChange={(e) =>
                  void applyCrop(
                    e.target.checked
                      ? constrained({ ...crop, constrainCrop: true })
                      : { ...crop, constrainCrop: false },
                  )
                }
              />
              Constrain
            </label>
          </div>
          <p className="muted sm">
            Drag outside the frame to rotate · ⌘-drag along a horizon to straighten.
          </p>
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

        <Panel title="Position & Size" defaultOpen={false}>
          <div className="lr-crop-btn-row" style={{ flexWrap: "wrap", gap: 6 }}>
            {(
              [
                ["x", px.x],
                ["y", px.y],
                ["w", px.w],
                ["h", px.h],
              ] as const
            ).map(([field, value]) => (
              <label key={field} className="muted sm" style={{ display: "flex", gap: 3, alignItems: "center" }}>
                {field.toUpperCase()}
                <NumInput value={value} onCommit={(v) => setNumeric(field, v)} />
              </label>
            ))}
          </div>
          <button type="button" onClick={fitLargest} title="Largest crop of the current ratio">
            Fit largest
          </button>
          <div className="lr-crop-btn-row" style={{ flexWrap: "wrap", gap: 6, marginTop: 6 }}>
            {(
              [
                ["L", "left"],
                ["R", "right"],
                ["T", "top"],
                ["B", "bottom"],
              ] as const
            ).map(([label, edge]) => (
              <label key={edge} className="muted sm" style={{ display: "flex", gap: 3, alignItems: "center" }}>
                {label}%
                <NumInput value={margins[edge]} onCommit={(v) => setMargin(edge, v)} width={48} />
              </label>
            ))}
          </div>
          <div className="lr-crop-row" style={{ marginTop: 6, alignItems: "center", gap: 6 }}>
            <label className="muted sm" style={{ display: "flex", gap: 3, alignItems: "center" }}>
              PPI
              <NumInput value={ppi} onCommit={setPpi} width={52} />
            </label>
            {printSize && <span className="muted sm">{printSize}</span>}
          </div>
        </Panel>

        <Panel title="Guides" defaultOpen={false}>
          <div className="lr-crop-row" style={{ gap: 6 }}>
            <select
              value={overlayKind}
              onChange={(e) =>
                useUiStore.setState({
                  cropOverlay: e.target.value as CropOverlayKind,
                  cropOverlayVisible: true,
                })
              }
              style={{ flex: 1 }}
            >
              <option value="none">None</option>
              {GUIDES.map((g) => (
                <option key={g.id} value={g.id}>
                  {g.label}
                </option>
              ))}
            </select>
            <select
              value={guideMode}
              onChange={(e) => setGuideMode(e.target.value as "auto" | "always" | "never")}
              title="Show guides: only while dragging (Auto), Always, or Never"
            >
              <option value="auto">Auto</option>
              <option value="always">Always</option>
              <option value="never">Never</option>
            </select>
          </div>
          <div className="lr-crop-row" style={{ gap: 6, marginTop: 6, alignItems: "center" }}>
            <label className="muted sm" style={{ display: "flex", gap: 3, alignItems: "center" }}>
              Color
              <input
                type="color"
                value={guideColor}
                onChange={(e) => setGuideColor(e.target.value)}
              />
            </label>
            <label className="muted sm" style={{ display: "flex", gap: 3, alignItems: "center", flex: 1 }}>
              Opacity
              <input
                type="range"
                min={0.1}
                max={1}
                step={0.05}
                value={guideOpacity}
                onChange={(e) => setGuideOpacity(parseFloat(e.target.value))}
              />
            </label>
          </div>
          {overlayKind === "grid" && (
            <label className="muted sm" style={{ display: "flex", gap: 3, alignItems: "center", marginTop: 6 }}>
              Grid divisions
              <NumInput value={gridSize} onCommit={setGridSize} width={40} />
            </label>
          )}
          {overlayKind === "aspects" && (
            <label className="muted sm" style={{ display: "flex", gap: 3, alignItems: "center", marginTop: 6 }}>
              Ratios
              <input
                defaultValue={aspectPreview.join(", ")}
                onBlur={(e) =>
                  setAspectPreview(
                    e.target.value
                      .split(",")
                      .map((s) => s.trim())
                      .filter((s) => parseRatioInput(s) !== null),
                  )
                }
                style={{ flex: 1 }}
                placeholder="1:1, 4:5, 16:9"
              />
            </label>
          )}
          <div className="lr-crop-row" style={{ gap: 6, marginTop: 6, alignItems: "center" }}>
            <label className="muted sm" style={{ display: "flex", gap: 3, alignItems: "center" }}>
              Mask
              <input
                type="color"
                value={maskColor}
                onChange={(e) => setMaskColor(e.target.value)}
              />
            </label>
            <label className="muted sm" style={{ display: "flex", gap: 3, alignItems: "center", flex: 1 }}>
              Dim
              <input
                type="range"
                min={0.1}
                max={0.95}
                step={0.05}
                value={maskOpacity}
                onChange={(e) => setMaskOpacity(parseFloat(e.target.value))}
              />
            </label>
          </div>
          <p className="muted sm">
            <kbd>O</kbd> cycle · <kbd>Shift+O</kbd> flip · <kbd>H</kbd> hide ·{" "}
            <kbd>L</kbd> lights out
          </p>
        </Panel>

        <div className="lr-crop-footer" style={{ display: "flex", gap: 6 }}>
          <button
            type="button"
            className="lr-crop-reset"
            onClick={() => resetCropModule().then(reconcile).catch(() => {})}
          >
            Reset crop
          </button>
          <button type="button" onClick={() => copyCropToClipboard(crop)} title="⌃⌥C">
            Copy
          </button>
          <button type="button" onClick={pasteCrop} title="⌃⌥V">
            Paste
          </button>
        </div>
      </div>
    </div>
  );
}

/** Small numeric input that commits on Enter/blur. */
function NumInput({
  value,
  onCommit,
  width = 64,
}: {
  value: number;
  onCommit: (v: number) => void;
  width?: number;
}) {
  const [text, setText] = useState<string | null>(null);
  return (
    <input
      style={{ width }}
      value={text ?? String(value)}
      onChange={(e) => setText(e.target.value)}
      onFocus={() => setText(String(value))}
      onBlur={() => {
        if (text !== null) onCommit(parseFloat(text));
        setText(null);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          if (text !== null) onCommit(parseFloat(text));
          setText(null);
          (e.target as HTMLInputElement).blur();
        }
        if (e.key === "Escape") setText(null);
      }}
    />
  );
}
