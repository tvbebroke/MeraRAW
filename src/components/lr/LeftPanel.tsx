// LR left rail: Navigator · Presets · Snapshots · History.
import { useEffect, useRef, useState } from "react";
import {
  applyPreset,
  getHistory,
  listPresets,
  listSnapshots,
  restoreSnapshot,
  savePresetNamed,
  snapshot,
} from "../../ipc/commands";
import { onFrameReady } from "../../ipc/events";
import { useDocStore } from "../../state/docStore";
import { useUiStore } from "../../state/uiStore";
import { frameUrl } from "../../viewport/Viewport";
import { Panel } from "./widgets";

const DEVELOP_MODULES = [
  "exposure",
  "white_balance",
  "calibration",
  "detail",
  "color_grade",
  "hsl",
  "tone_curve",
];

function Navigator() {
  const ref = useRef<HTMLCanvasElement>(null);
  const sendViewCmd = useUiStore((s) => s.sendViewCmd);

  useEffect(() => {
    let cancelled = false;
    async function draw(version: number) {
      try {
        const res = await fetch(frameUrl(version));
        if (!res.ok) return;
        const w = parseInt(res.headers.get("X-Frame-Width") ?? "0", 10);
        const h = parseInt(res.headers.get("X-Frame-Height") ?? "0", 10);
        if (!w || !h) return;
        const buf = new Uint8ClampedArray(await res.arrayBuffer());
        if (buf.length !== w * h * 4 || cancelled) return;
        const c = ref.current;
        if (!c) return;
        c.width = w;
        c.height = h;
        c.getContext("2d")?.putImageData(new ImageData(buf, w, h), 0, 0);
      } catch {
        /* ignore */
      }
    }
    const un = onFrameReady(draw);
    return () => {
      cancelled = true;
      un.then((f) => f());
    };
  }, []);

  return (
    <div className="navigator" title="click to fit" onClick={() => sendViewCmd("fit")}>
      <canvas ref={ref} className="navigator-canvas" />
    </div>
  );
}

function Presets() {
  const [presets, setPresets] = useState<string[]>([]);
  const [name, setName] = useState("");
  const reconcile = useDocStore((s) => s.reconcile);
  const refresh = () => listPresets().then(setPresets).catch(() => {});
  useEffect(() => {
    void refresh();
  }, []);
  return (
    <div className="lr-list-panel">
      {presets.length === 0 && <div className="muted sm">No presets yet.</div>}
      {presets.map((p) => (
        <button
          key={p}
          className="lr-list-row"
          onClick={() => applyPreset(p).then(reconcile).catch(() => {})}
        >
          {p}
        </button>
      ))}
      <div className="lr-add-row">
        <input
          placeholder="new preset…"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && name.trim()) {
              savePresetNamed(name.trim(), DEVELOP_MODULES)
                .then(() => {
                  setName("");
                  void refresh();
                })
                .catch(() => {});
            }
          }}
        />
      </div>
    </div>
  );
}

function Snapshots() {
  const [snaps, setSnaps] = useState<string[]>([]);
  const [name, setName] = useState("");
  const reconcile = useDocStore((s) => s.reconcile);
  const docVersion = useDocStore((s) => s.docVersion);
  const refresh = () => listSnapshots().then(setSnaps).catch(() => {});
  useEffect(() => {
    void refresh();
  }, [docVersion]);
  return (
    <div className="lr-list-panel">
      {snaps.length === 0 && <div className="muted sm">No snapshots.</div>}
      {snaps.map((s) => (
        <button
          key={s}
          className="lr-list-row"
          onClick={() => restoreSnapshot(s).then(reconcile).catch(() => {})}
          title="restore"
        >
          {s}
        </button>
      ))}
      <div className="lr-add-row">
        <input
          placeholder="snapshot name…"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && name.trim()) {
              snapshot(name.trim())
                .then(() => {
                  setName("");
                  void refresh();
                })
                .catch(() => {});
            }
          }}
        />
      </div>
    </div>
  );
}

function History() {
  const docVersion = useDocStore((s) => s.docVersion);
  const history = useDocStore((s) => s.history);
  const setHistory = useDocStore((s) => s.setHistory);
  useEffect(() => {
    getHistory().then(setHistory).catch(() => {});
  }, [docVersion, setHistory]);
  if (history.length === 0)
    return <div className="muted sm">No edits yet.</div>;
  return (
    <div className="lr-history">
      {history
        .slice(-30)
        .reverse()
        .map((h, i) => (
          <div key={i} className="lr-history-row">
            {h}
          </div>
        ))}
    </div>
  );
}

export function LeftPanel() {
  return (
    <div className="lr-left">
      <Panel title="Navigator">
        <Navigator />
      </Panel>
      <Panel title="Presets">
        <Presets />
      </Panel>
      <Panel title="Snapshots" defaultOpen={false}>
        <Snapshots />
      </Panel>
      <Panel title="History" defaultOpen={false}>
        <History />
      </Panel>
    </div>
  );
}
