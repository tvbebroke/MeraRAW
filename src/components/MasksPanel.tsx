// Mask management: create (AI + geometric), select (scoped develop
// controls), overlay, refine (opacity/feather/invert), delete. Every
// action is a guard-walled undoable op — the same ops Claude wraps in P6.
import { useState } from "react";
import { applyOp, setMaskOverlay } from "../ipc/commands";
import type { MaskMirror, Op } from "../ipc/types";
import { useDocStore } from "../state/docStore";
import { useUiStore } from "../state/uiStore";

const KIND_ICONS: Record<string, string> = {
  subject: "👤",
  sky: "☁️",
  background: "🏞",
  object: "📍",
  radial: "⭕",
  linear: "📐",
  brush: "🖌",
};

export function MasksPanel() {
  const doc = useDocStore((s) => s.doc);
  const reconcile = useDocStore((s) => s.reconcile);
  const { selectedMask, setSelectedMask, tool, setTool } = useUiStore();
  const [overlayId, setOverlayId] = useState<string | null>(null);
  const masks: MaskMirror[] = doc?.masks ?? [];

  async function dispatch(op: Op) {
    try {
      const delta = await applyOp(op);
      reconcile(delta);
      return delta;
    } catch (e) {
      console.error("mask op rejected", e);
    }
  }

  async function addMask(kind: string, source: unknown) {
    const delta = await dispatch({ op: "add_mask", kind, source });
    if (delta?.newMaskId) setSelectedMask(delta.newMaskId);
  }

  async function toggleOverlay(id: string) {
    const next = overlayId === id ? null : id;
    setOverlayId(next);
    await setMaskOverlay(next).catch(() => {});
  }

  async function removeMask(id: string) {
    if (selectedMask === id) setSelectedMask(null);
    if (overlayId === id) {
      setOverlayId(null);
      await setMaskOverlay(null).catch(() => {});
    }
    await dispatch({ op: "remove_mask", id });
  }

  return (
    <div className="stack">
      <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
        <button
          onClick={() =>
            addMask("subject", { type: "segmented", model: "subject_v1", hint: null })
          }
        >
          👤 Subject
        </button>
        <button
          onClick={() =>
            addMask("background", {
              type: "segmented",
              model: "subject_v1",
              hint: null,
            })
          }
        >
          🏞 Background
        </button>
        <button
          onClick={() =>
            addMask("sky", { type: "segmented", model: "sky_v1", hint: null })
          }
        >
          ☁️ Sky
        </button>
        <button
          onClick={() =>
            addMask("radial", {
              type: "radial",
              center: [0.5, 0.5],
              radii: [0.3, 0.25],
              rotation: 0,
            })
          }
        >
          ⭕ Radial
        </button>
        <button
          onClick={() =>
            addMask("linear", { type: "linear", start: [0.5, 0.0], end: [0.5, 0.6] })
          }
        >
          📐 Linear
        </button>
        <button
          onClick={() => addMask("brush", { type: "brush", strokes: [] })}
        >
          🖌 Brush
        </button>
      </div>
      {masks.length === 0 && <div className="muted">No masks yet.</div>}
      {masks.map((m) => (
        <div
          key={m.id}
          className="mask-row"
          style={{
            border:
              selectedMask === m.id
                ? "1px solid var(--accent)"
                : "1px solid var(--border)",
          }}
        >
          <div
            style={{ display: "flex", gap: 6, alignItems: "center", cursor: "pointer" }}
            onClick={() =>
              setSelectedMask(selectedMask === m.id ? null : m.id)
            }
          >
            <span>{KIND_ICONS[m.kind] ?? "▦"}</span>
            <span style={{ flex: 1 }}>{m.kind}</span>
            <button
              title="overlay"
              onClick={(e) => {
                e.stopPropagation();
                void toggleOverlay(m.id);
              }}
              style={overlayId === m.id ? { borderColor: "var(--accent)" } : undefined}
            >
              👁
            </button>
            <button
              title="delete"
              onClick={(e) => {
                e.stopPropagation();
                void removeMask(m.id);
              }}
            >
              ✕
            </button>
          </div>
          {selectedMask === m.id && (
            <div className="stack" style={{ marginTop: 6 }}>
              <div className="control">
                <div className="control-head">
                  <span>Opacity</span>
                  <span className="control-value">{m.opacity.toFixed(0)}</span>
                </div>
                <input
                  type="range"
                  min={0}
                  max={100}
                  step={1}
                  value={m.opacity}
                  onChange={(e) =>
                    void dispatch({
                      op: "refine_mask",
                      id: m.id,
                      opacity: parseFloat(e.target.value),
                    })
                  }
                />
              </div>
              <div className="control">
                <div className="control-head">
                  <span>Feather</span>
                  <span className="control-value">{m.feather.toFixed(0)}</span>
                </div>
                <input
                  type="range"
                  min={0}
                  max={100}
                  step={1}
                  value={m.feather}
                  onChange={(e) =>
                    void dispatch({
                      op: "refine_mask",
                      id: m.id,
                      feather: parseFloat(e.target.value),
                    })
                  }
                />
              </div>
              <label style={{ display: "flex", gap: 6, fontSize: 12 }}>
                <input
                  type="checkbox"
                  checked={m.invert}
                  onChange={(e) =>
                    void dispatch({
                      op: "refine_mask",
                      id: m.id,
                      invert: e.target.checked,
                    })
                  }
                />
                Invert
              </label>
              {m.kind === "brush" && (
                <button
                  onClick={() => setTool(tool === "brush" ? "pan" : "brush")}
                  style={tool === "brush" ? { borderColor: "var(--accent)" } : undefined}
                >
                  🖌 {tool === "brush" ? "Painting… (click to stop)" : "Paint"}
                </button>
              )}
              <div className="muted">
                Develop sliders now edit this mask.
              </div>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
