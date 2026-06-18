// HSL / Color band picker. Click a hue band to reveal its hue/sat/lum
// sliders (registry-driven, op-dispatching) — LR-style, instead of 24
// flat sliders. Orange is the skin band (highlighted).
import { useState } from "react";
import type { ImageMeta, ParamSpec } from "../../ipc/types";
import { useDocStore } from "../../state/docStore";
import { ParamSlider } from "./widgets";

const BANDS: { key: string; label: string; color: string }[] = [
  { key: "red", label: "Red", color: "#e0564f" },
  { key: "orange", label: "Orange", color: "#e08a3c" },
  { key: "yellow", label: "Yellow", color: "#d6c34a" },
  { key: "green", label: "Green", color: "#6db86d" },
  { key: "aqua", label: "Aqua", color: "#54b3b0" },
  { key: "blue", label: "Blue", color: "#5a8fe0" },
  { key: "purple", label: "Purple", color: "#9a6fd0" },
  { key: "magenta", label: "Magenta", color: "#d062b0" },
];

export function HslPicker({
  specs,
  meta,
}: {
  specs: ParamSpec[];
  meta: ImageMeta | null;
}) {
  const [band, setBand] = useState("orange");
  const doc = useDocStore((s) => s.doc);

  const edited = (key: string) => {
    const m = doc?.modules?.hsl as Record<string, number> | undefined;
    return (
      m &&
      ([`${key}.hue`, `${key}.sat`, `${key}.lum`].some(
        (p) => typeof m[p] === "number" && m[p] !== 0,
      ))
    );
  };

  return (
    <div className="hsl">
      <div className="hsl-bands">
        {BANDS.map((b) => (
          <button
            key={b.key}
            className={`hsl-swatch ${band === b.key ? "active" : ""}`}
            style={{ background: b.color }}
            title={b.label + (b.key === "orange" ? " (skin)" : "")}
            onClick={() => setBand(b.key)}
          >
            {edited(b.key) && <span className="hsl-dot" />}
          </button>
        ))}
      </div>
      <div className="hsl-name">
        {BANDS.find((b) => b.key === band)?.label}
        {band === "orange" && <span className="muted"> · skin</span>}
      </div>
      <ParamSlider specs={specs} path={`hsl.${band}.hue`} label="Hue" meta={meta} />
      <ParamSlider specs={specs} path={`hsl.${band}.sat`} label="Saturation" meta={meta} />
      <ParamSlider specs={specs} path={`hsl.${band}.lum`} label="Luminance" meta={meta} />
    </div>
  );
}
