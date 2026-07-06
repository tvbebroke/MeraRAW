// Right rail — Edit tab: Profile → Light → Color → Effects → Detail (+ histogram, AI).
import { useState } from "react";
import {
  applyParamBatch,
  getDoc,
  getStats,
  pickLut,
  setCameraProfile,
  setDemosaic,
  setLut,
  setParam,
} from "../../ipc/commands";
import type { ImageMeta, ParamSpec } from "../../ipc/types";
import { useDocStore } from "../../state/docStore";
import { useUiStore } from "../../state/uiStore";
import { Histogram } from "../Histogram";
import { AiGrader } from "./AiGrader";
import { ColorGrading } from "./ColorGrading";
import { HslPicker } from "./HslPicker";
import { ToneCurve } from "./ToneCurve";
import { Icon, Panel, ParamSlider, useRegistry } from "./widgets";

function ProfilePanel({
  meta,
  onProfileChange,
}: {
  meta: ImageMeta | null;
  onProfileChange: (m: ImageMeta) => void;
}) {
  if (!meta) {
    return (
      <Panel title="Profile" defaultOpen>
        <p className="muted sm">Open a RAW file to choose a camera profile.</p>
      </Panel>
    );
  }
  const files = meta.availableProfileFiles ?? [];
  const names = meta.availableProfiles ?? [];
  if (files.length === 0) {
    return (
      <Panel title="Profile" defaultOpen>
        <p className="muted sm">
          No DCP profiles for {meta.cameraMake} {meta.cameraModel}.
        </p>
      </Panel>
    );
  }
  const currentIdx = names.findIndex((n) => n === meta.cameraProfile);
  return (
    <Panel title="Profile" defaultOpen>
      <p className="muted sm">
        Base color rendering before sliders. Switching profile does not change your edits.
      </p>
      <select
        value={files[currentIdx >= 0 ? currentIdx : 0] ?? files[0]}
        onChange={(e) => {
          const file = e.target.value;
          setCameraProfile(file)
            .then(onProfileChange)
            .catch(console.error);
        }}
      >
        {files.map((file, i) => (
          <option key={file} value={file}>
            {names[i] ?? file}
          </option>
        ))}
      </select>
    </Panel>
  );
}

const DEMOSAIC_OPTIONS: { id: string; label: string }[] = [
  // in-process: rawler built-in + merawler engine
  { id: "rawler", label: "Rawler (built-in)" },
  { id: "bilinear", label: "Bilinear" },
  { id: "malvar", label: "Malvar" },
  { id: "rcd", label: "RCD — darktable default" },
  { id: "lmmse", label: "LMMSE — best for noise" },
  { id: "amaze", label: "AMaZE — max detail" },
  { id: "igv", label: "IGV — anti-aliasing" },
  { id: "ddfapd", label: "DDFAPD (Menon)" },
  // sidecar: zerawler engine (external reference binaries)
  { id: "rt-rcd", label: "RCD — sidecar · RawTherapee" },
  { id: "rt-lmmse", label: "LMMSE — sidecar · RawTherapee" },
  { id: "rt-amaze", label: "AMaZE — sidecar · RawTherapee" },
  { id: "dht", label: "DHT — sidecar · LibRaw" },
];

function DemosaicPanel({
  meta,
  onMetaChange,
}: {
  meta: ImageMeta | null;
  onMetaChange: (m: ImageMeta) => void;
}) {
  // Demosaicing only applies to sensor RAW (rendered images are already RGB).
  if (!meta || meta.kind !== "raw") {
    return null;
  }
  const current = meta.demosaic || "rcd";
  return (
    <Panel title="Demosaic" defaultOpen={false}>
      <p className="muted sm">
        Bayer reconstruction algorithm — in-process (merawler) or sidecar
        (zerawler, external RawTherapee/LibRaw binaries). Switching re-decodes
        the RAW; the preview and exports update. Your edits are unchanged.
        Sidecar RT color lands via RawTherapee&rsquo;s camera profile, so its
        base rendition differs slightly from the in-process paths.
      </p>
      <select
        value={current}
        onChange={(e) => {
          setDemosaic(e.target.value).then(onMetaChange).catch(console.error);
        }}
      >
        {DEMOSAIC_OPTIONS.map((o) => (
          <option key={o.id} value={o.id}>
            {o.label}
          </option>
        ))}
      </select>
    </Panel>
  );
}

function LightPanel({
  meta,
  specs,
}: {
  meta: ImageMeta | null;
  specs: ParamSpec[];
}) {
  return (
    <Panel title="Light" defaultOpen>
      <ParamSlider specs={specs} path="exposure.stops" label="Exposure" meta={meta} />
      <ParamSlider specs={specs} path="tone_curve.contrast" label="Contrast" meta={meta} />
      <ParamSlider specs={specs} path="tone_curve.highlights" label="Highlights" meta={meta} />
      <ParamSlider specs={specs} path="tone_curve.shadows" label="Shadows" meta={meta} />
      <ParamSlider specs={specs} path="tone_curve.lights" label="Whites" meta={meta} />
      <ParamSlider specs={specs} path="tone_curve.darks" label="Blacks" meta={meta} />
    </Panel>
  );
}

function ColorPanel({
  meta,
  specs,
  tool,
  setTool,
}: {
  meta: ImageMeta | null;
  specs: ParamSpec[];
  tool: string;
  setTool: (t: "pan" | "wb" | "brush") => void;
}) {
  return (
    <>
      <Panel title="White Balance" defaultOpen>
        <div className="lr-subhead">
          <span>White Balance</span>
          <button
            type="button"
            className={`lr-mini-btn ${tool === "wb" ? "active" : ""}`}
            title="Eyedropper: click a neutral area"
            onClick={() => setTool(tool === "wb" ? "pan" : "wb")}
          >
            <Icon.Eyedropper size={12} />
          </button>
        </div>
        <ParamSlider specs={specs} path="white_balance.temp" label="Temp" meta={meta} />
        <ParamSlider specs={specs} path="white_balance.tint" label="Tint" meta={meta} />
      </Panel>
      <Panel title="Color" defaultOpen>
        <ParamSlider specs={specs} path="color_grade.perceptual_sat" label="Vibrance" meta={meta} />
        <ParamSlider specs={specs} path="color_grade.global_chroma" label="Saturation" meta={meta} />
      </Panel>
      <Panel title="Tone Curve" defaultOpen={false}>
        <ToneCurve />
      </Panel>
      <Panel title="HSL / Color Mix" defaultOpen>
        <HslPicker specs={specs} meta={meta} />
      </Panel>
    </>
  );
}

function EffectsPanel({ meta }: { meta: ImageMeta | null }) {
  return (
    <>
      <Panel title="Split Toning" defaultOpen>
        <p className="muted sm">Separate hue and saturation for shadows and highlights.</p>
        <ColorGrading specs={useRegistry()} meta={meta} />
      </Panel>
      <Panel title="Clarity & Dehaze" defaultOpen={false}>
        <p className="muted sm">Local contrast and haze controls — coming in a future build.</p>
      </Panel>
      <Panel title="Vignette" defaultOpen={false}>
        <p className="muted sm">Post-crop vignette — coming in a future build.</p>
      </Panel>
    </>
  );
}

function LutPanel({
  meta,
  specs,
}: {
  meta: ImageMeta | null;
  specs: ParamSpec[];
}) {
  const doc = useDocStore((s) => s.doc);
  const setDoc = useDocStore((s) => s.setDoc);
  const [busy, setBusy] = useState(false);

  const lutPath = (doc?.meta?.lut_file as string | undefined) ?? undefined;
  const lutName = lutPath ? lutPath.split(/[\\/]/).pop() : undefined;

  async function load() {
    setBusy(true);
    try {
      const path = await pickLut();
      if (path) {
        await setLut(path);
        setDoc(await getDoc());
      }
    } catch (e) {
      console.error(e);
    } finally {
      setBusy(false);
    }
  }

  async function clear() {
    setBusy(true);
    try {
      await setLut(null);
      setDoc(await getDoc());
    } catch (e) {
      console.error(e);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel title="Look LUT" defaultOpen={false}>
      <p className="muted sm">
        Apply a 3D <code>.cube</code> look. Sits after tone, before sharpening.
      </p>
      <div className="lr-subhead">
        <span title={lutPath}>{lutName ?? "No LUT loaded"}</span>
      </div>
      <div style={{ display: "flex", gap: "6px" }}>
        <button
          type="button"
          className="lr-mini-btn"
          disabled={busy || !meta}
          onClick={() => void load()}
        >
          {lutName ? "Replace…" : "Load LUT…"}
        </button>
        {lutName && (
          <button
            type="button"
            className="lr-mini-btn"
            disabled={busy}
            onClick={() => void clear()}
          >
            Remove
          </button>
        )}
      </div>
      {lutName && (
        <ParamSlider specs={specs} path="lut.opacity" label="Opacity" meta={meta} />
      )}
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

export function DevelopRail({
  meta,
  onMetaChange,
}: {
  meta: ImageMeta | null;
  onMetaChange?: (m: ImageMeta) => void;
}) {
  const specs = useRegistry();
  const reconcile = useDocStore((s) => s.reconcile);
  const tool = useUiStore((s) => s.tool);
  const setTool = useUiStore((s) => s.setTool);

  async function autoTone() {
    const stats = await getStats().catch(() => null);
    const clipHigh = stats?.clipHighPct ?? 0;
    const clipLow = stats?.clipLowPct ?? 0;
    const exposure = clipHigh > 2 ? -0.35 : clipLow > 2 ? 0.35 : 0;
    await applyParamBatch({
      exposure: { stops: exposure },
      tone_curve: {
        contrast: 8,
        highlights: clipHigh > 1 ? -18 : 0,
        shadows: clipLow > 1 ? 22 : 0,
        lights: 0,
        darks: 0,
      },
      color_grade: { perceptual_sat: 6, global_chroma: 4 },
    }).then(reconcile);
  }

  function setBw(on: boolean) {
    setParam("color_grade.global_chroma", on ? -100 : 0)
      .then(reconcile)
      .catch(console.error);
    setParam("color_grade.perceptual_sat", on ? -100 : 0)
      .then(reconcile)
      .catch(console.error);
  }

  return (
    <div className="lr-rail-panel lr-edit-panel">
      <header className="lr-rail-panel-head lr-edit-head">
        <h2>Edit</h2>
        <div className="lr-edit-quick">
          <button type="button" className="lr-mini-btn" onClick={() => void autoTone()}>
            Auto
          </button>
          <button type="button" className="lr-mini-btn" onClick={() => setBw(true)}>
            B&amp;W
          </button>
        </div>
      </header>
      <div className="lr-histogram-pin">
        <Histogram />
      </div>
      <div className="lr-ai-pin">
        <Panel title="AI Color Grader" className="ai-panel" defaultOpen>
          <AiGrader />
        </Panel>
      </div>
      <div className="lr-rail-scroll">
        <ProfilePanel
          meta={meta}
          onProfileChange={(m) => onMetaChange?.(m)}
        />
        <DemosaicPanel meta={meta} onMetaChange={(m) => onMetaChange?.(m)} />
        <LightPanel meta={meta} specs={specs} />
        <ColorPanel meta={meta} specs={specs} tool={tool} setTool={setTool} />
        <EffectsPanel meta={meta} />
        <LutPanel meta={meta} specs={specs} />
        <GroupPanel title="Detail" group="Detail" meta={meta} defaultOpen />
        <GroupPanel title="Calibration" group="Calibration" meta={meta} defaultOpen={false} />
        <Panel title="Optics" defaultOpen={false}>
          <p className="muted sm">Lens corrections and chromatic aberration — coming soon.</p>
        </Panel>
        <Panel title="Geometry" defaultOpen={false}>
          <p className="muted sm">Upright transforms — coming soon.</p>
        </Panel>
      </div>
    </div>
  );
}
