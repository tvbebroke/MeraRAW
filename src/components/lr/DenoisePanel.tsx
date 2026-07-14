// Noise Reduction panel — Lightroom-simple by default, advanced disclosure
// for engine/strength/impulse/NLM (denoise/06-ui-ux-spec.md).
import { useEffect, useMemo, useState } from "react";
import {
  denoiseAiCancel,
  denoiseAiStart,
  denoiseModelsList,
  setParam,
} from "../../ipc/commands";
import type { ImageMeta, ParamSpec } from "../../ipc/types";
import { Panel, ParamSlider, useParam, useRegistry } from "./widgets";

function CheckRow({
  specs,
  path,
  label,
  meta,
}: {
  specs: ParamSpec[];
  path: string;
  label: string;
  meta: ImageMeta | null;
}) {
  const spec = specs.find((s) => s.path === path);
  const safe = spec ?? {
    path,
    ty: "f32" as const,
    min: 0,
    max: 1,
    default: 0,
    ui: { label, step: 1, scale: "linear", group: "Noise Reduction" },
  };
  const h = useParam(safe, meta);
  if (!spec) return null;
  const on = h.value >= 0.5;
  return (
    <label className="lr-check-row">
      <input
        type="checkbox"
        checked={on}
        onChange={(e) => h.commit(e.target.checked ? 1 : 0)}
      />
      <span>{label}</span>
    </label>
  );
}

function EngineSelect({
  specs,
  meta,
}: {
  specs: ParamSpec[];
  meta: ImageMeta | null;
}) {
  const spec = specs.find((s) => s.path === "detail.nr_engine");
  const safe = spec ?? {
    path: "detail.nr_engine",
    ty: "f32" as const,
    min: 0,
    max: 2,
    default: 0,
    ui: { label: "Engine", step: 1, scale: "linear", group: "Noise Reduction" },
  };
  const h = useParam(safe, meta);
  if (!spec) return null;
  const v = Math.round(h.value);
  return (
    <label className="lr-select-row">
      <span>Engine</span>
      <select
        value={v}
        onChange={(e) => h.commit(parseFloat(e.target.value))}
        title="NLM: smoother on heavy noise, slower"
      >
        <option value={0}>Wavelet (auto)</option>
        <option value={1}>Wavelet</option>
        <option value={2}>Non-local means</option>
      </select>
    </label>
  );
}

export function DenoisePanel({ meta }: { meta: ImageMeta | null }) {
  const specs = useRegistry();
  const [advanced, setAdvanced] = useState(false);
  const luma = useParam(
    specs.find((s) => s.path === "detail.noise_luma") ?? {
      path: "detail.noise_luma",
      ty: "f32",
      min: 0,
      max: 100,
      default: 0,
      ui: { label: "Luminance", step: 1, scale: "linear", group: "Noise Reduction" },
    },
    meta,
  );
  const suggestAi = useMemo(() => (meta?.iso ?? 0) >= 12800, [meta?.iso]);
  const [aiJob, setAiJob] = useState<number | null>(null);
  const [aiStatus, setAiStatus] = useState<string>("");
  const [standIn, setStandIn] = useState(false);

  useEffect(() => {
    void denoiseModelsList()
      .then((ms) => setStandIn(Boolean(ms[0]?.standIn)))
      .catch(() => setStandIn(false));
  }, []);

  async function runAi() {
    try {
      await setParam("detail.ai_enabled", 1);
      setAiStatus(standIn ? "Queuing (classical stand-in)…" : "Queuing…");
      const job = await denoiseAiStart();
      setAiJob(job);
      setAiStatus(`Running job #${job}`);
    } catch (e) {
      setAiStatus(e instanceof Error ? e.message : String(e));
      setAiJob(null);
    }
  }

  async function cancelAi() {
    if (aiJob == null) return;
    await denoiseAiCancel(aiJob);
    setAiStatus("Cancelled");
    setAiJob(null);
  }

  return (
    <Panel title="Noise Reduction" defaultOpen={false}>
      {suggestAi && (
        <p className="muted sm">ISO {meta?.iso} — AI Denoise recommended.</p>
      )}

      <div className="lr-denoise-section">
        <CheckRow specs={specs} path="detail.ai_enabled" label="AI Denoise" meta={meta} />
        <ParamSlider specs={specs} path="detail.ai_amount" label="Amount" meta={meta} />
        <div style={{ display: "flex", gap: 6, alignItems: "center", marginTop: 4 }}>
          <button type="button" className="lr-mini-btn" onClick={() => void runAi()}>
            Run Denoise
          </button>
          {aiJob != null && (
            <button type="button" className="lr-mini-btn" onClick={() => void cancelAi()}>
              Cancel
            </button>
          )}
        </div>
        {standIn && (
          <p className="muted sm">ONNX model not installed — Run uses classical stand-in.</p>
        )}
        {aiStatus && <p className="muted sm">{aiStatus}</p>}
      </div>

      <div className="lr-denoise-section">
        <span className="lr-denoise-label">Manual</span>
        <ParamSlider specs={specs} path="detail.noise_luma" label="Luminance" meta={meta} />
        <ParamSlider
          specs={specs}
          path="detail.detail_preserve"
          label="Detail"
          meta={meta}
        />
        <ParamSlider specs={specs} path="detail.noise_chroma" label="Color" meta={meta} />
        <CheckRow specs={specs} path="detail.chroma_auto" label="Color Auto" meta={meta} />
        {luma.value <= 0 && (
          <p className="muted sm">Detail recovery is most visible once Luminance &gt; 0.</p>
        )}
      </div>

      <button
        type="button"
        className="lr-mini-btn"
        onClick={() => setAdvanced((a) => !a)}
      >
        {advanced ? "Hide advanced" : "Advanced"}
      </button>

      {advanced && (
        <div className="lr-denoise-section">
          <EngineSelect specs={specs} meta={meta} />
          <ParamSlider specs={specs} path="detail.nr_strength" label="Strength" meta={meta} />
          <ParamSlider specs={specs} path="detail.impulse" label="Impulse" meta={meta} />
          <CheckRow specs={specs} path="detail.hot_pixels" label="Hot pixel removal" meta={meta} />
          <CheckRow specs={specs} path="detail.nr_aggressive" label="Aggressive mode" meta={meta} />
          <ParamSlider specs={specs} path="detail.nlm_patch" label="NLM patch" meta={meta} />
          <ParamSlider specs={specs} path="detail.nlm_search" label="NLM search" meta={meta} />
          <ParamSlider specs={specs} path="detail.nlm_center" label="NLM center" meta={meta} />
        </div>
      )}
    </Panel>
  );
}
