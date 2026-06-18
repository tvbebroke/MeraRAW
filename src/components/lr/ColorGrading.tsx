// 3-way color grading wheels (shadows / midtones / highlights) + global.
// Wheel angle = hue (0-360), radius = saturation (0-100); a luminance
// slider sits under each. One wheel drag = ONE history step via an
// apply_preset batch (global) — never a side channel.
import { useCallback, useRef, useState } from "react";
import { applyParamBatch, setParam } from "../../ipc/commands";
import type { ImageMeta, ParamSpec } from "../../ipc/types";
import { useDocStore } from "../../state/docStore";
import { useUiStore } from "../../state/uiStore";
import { ParamSlider } from "./widgets";

const W = 96;
const R = W / 2 - 6;

function readVal(
  doc: ReturnType<typeof useDocStore.getState>["doc"],
  selectedMask: string | null,
  key: string,
): number {
  if (selectedMask) {
    return (
      (doc?.masks?.find((m) => m.id === selectedMask)?.modules?.color_grade?.[
        key
      ] as number | undefined) ?? 0
    );
  }
  return (doc?.modules?.color_grade?.[key] as number | undefined) ?? 0;
}

function Wheel({ zone, label }: { zone: string; label: string }) {
  const doc = useDocStore((s) => s.doc);
  const reconcile = useDocStore((s) => s.reconcile);
  const selectedMask = useUiStore((s) => s.selectedMask);
  const ref = useRef<HTMLDivElement>(null);
  const dragging = useRef(false);
  const [local, setLocal] = useState<{ hue: number; sat: number } | null>(null);

  const hue = local?.hue ?? readVal(doc, selectedMask, `${zone}_hue`);
  const sat = local?.sat ?? readVal(doc, selectedMask, `${zone}_sat`);

  const commit = useCallback(
    (h: number, s: number, final: boolean) => {
      if (selectedMask) {
        // mask-scoped: two ops (acceptable extra history entries)
        setParam(`mask.${selectedMask}.color_grade.${zone}_hue`, h)
          .then(reconcile)
          .catch(() => {});
        setParam(`mask.${selectedMask}.color_grade.${zone}_sat`, s)
          .then(reconcile)
          .catch(() => {});
      } else {
        // global: one coherent history step
        applyParamBatch({
          color_grade: { [`${zone}_hue`]: h, [`${zone}_sat`]: s },
        })
          .then(reconcile)
          .catch(() => {});
      }
      if (final) setLocal(null);
    },
    [zone, selectedMask, reconcile],
  );

  const fromPointer = (e: React.PointerEvent): { hue: number; sat: number } => {
    const r = ref.current!.getBoundingClientRect();
    const dx = e.clientX - (r.left + r.width / 2);
    const dy = e.clientY - (r.top + r.height / 2);
    let h = (Math.atan2(-dy, dx) * 180) / Math.PI;
    h = (h + 360) % 360;
    const dist = Math.min(1, Math.hypot(dx, dy) / ((r.width / 2) * (R / (W / 2))));
    return { hue: Math.round(h), sat: Math.round(dist * 100) };
  };

  const knobAngle = (hue * Math.PI) / 180;
  const knobR = (sat / 100) * R;
  const knobX = W / 2 + Math.cos(knobAngle) * knobR;
  const knobY = W / 2 - Math.sin(knobAngle) * knobR;

  return (
    <div className="cg-wheel">
      <div
        ref={ref}
        className="cg-disc"
        style={{ width: W, height: W }}
        onPointerDown={(e) => {
          dragging.current = true;
          (e.target as Element).setPointerCapture(e.pointerId);
          const v = fromPointer(e);
          setLocal(v);
          commit(v.hue, v.sat, false);
        }}
        onPointerMove={(e) => {
          if (!dragging.current) return;
          const v = fromPointer(e);
          setLocal(v);
          commit(v.hue, v.sat, false);
        }}
        onPointerUp={() => {
          dragging.current = false;
          commit(hue, sat, true);
        }}
        onDoubleClick={() => commit(0, 0, true)}
      >
        <svg width={W} height={W} style={{ position: "absolute", inset: 0 }}>
          <circle cx={W / 2} cy={W / 2} r={R} className="cg-ring" />
          <line
            x1={W / 2}
            y1={W / 2}
            x2={knobX}
            y2={knobY}
            className="cg-arm"
          />
          <circle cx={knobX} cy={knobY} r={5} className="cg-knob" />
        </svg>
      </div>
      <div className="cg-label">{label}</div>
    </div>
  );
}

export function ColorGrading({
  specs,
  meta,
}: {
  specs: ParamSpec[];
  meta: ImageMeta | null;
}) {
  return (
    <div className="cg">
      <div className="cg-row">
        <Wheel zone="shadows" label="Shadows" />
        <Wheel zone="midtones" label="Midtones" />
        <Wheel zone="highlights" label="Highlights" />
      </div>
      <ParamSlider specs={specs} path="color_grade.shadows_lum" label="Shadow Lum" meta={meta} />
      <ParamSlider specs={specs} path="color_grade.midtones_lum" label="Mid Lum" meta={meta} />
      <ParamSlider specs={specs} path="color_grade.highlights_lum" label="High Lum" meta={meta} />
      <div className="lr-divider" />
      <ParamSlider specs={specs} path="color_grade.global_chroma" label="Chroma" meta={meta} />
      <ParamSlider specs={specs} path="color_grade.perceptual_sat" label="Saturation" meta={meta} />
      <ParamSlider specs={specs} path="color_grade.shadow_range" label="Shadow Range" meta={meta} />
      <ParamSlider specs={specs} path="color_grade.highlight_range" label="Highlight Range" meta={meta} />
    </div>
  );
}
