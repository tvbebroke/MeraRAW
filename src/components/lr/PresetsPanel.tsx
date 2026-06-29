import { useEffect, useState } from "react";
import { applyPreset, listPresets, savePresetNamed } from "../../ipc/commands";
import { useDocStore } from "../../state/docStore";

const DEVELOP_MODULES = [
  "exposure",
  "white_balance",
  "calibration",
  "detail",
  "color_grade",
  "hsl",
  "tone_curve",
];

export function PresetsPanel() {
  const [presets, setPresets] = useState<string[]>([]);
  const [name, setName] = useState("");
  const reconcile = useDocStore((s) => s.reconcile);
  const refresh = () => listPresets().then(setPresets).catch(() => {});

  useEffect(() => {
    void refresh();
  }, []);

  return (
    <div className="lr-rail-panel">
      <header className="lr-rail-panel-head">
        <h2>Presets</h2>
      </header>
      <div className="lr-rail-panel-body">
        <div className="lr-list-panel">
          {presets.length === 0 && (
            <div className="muted sm">No presets yet — save your current look below.</div>
          )}
          {presets.map((p) => (
            <button
              key={p}
              type="button"
              className="lr-list-row"
              onClick={() => applyPreset(p).then(reconcile).catch(() => {})}
            >
              {p}
            </button>
          ))}
          <div className="lr-add-row">
            <input
              placeholder="Save preset as…"
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
      </div>
    </div>
  );
}
