import { applyOp, setParam } from "../../ipc/commands";
import type { EditDocMirror, ImageMeta, ParamSpec } from "../../ipc/types";
import { defaultValue, effectiveValue } from "../engine/params";
import { reconcile } from "../../stores/doc";
import type { SectionId } from "../../stores/editor";

/** Registry path prefixes (or exact paths) owned by each accordion. */
export const SECTION_PREFIXES: Record<SectionId, string[]> = {
  light: [
    "exposure.",
    "tone_curve.contrast",
    "tone_curve.highlights",
    "tone_curve.shadows",
    "tone_curve.lights",
    "tone_curve.darks",
  ],
  color: ["white_balance.", "hsl.", "color_grade.perceptual_sat", "color_grade.global_chroma"],
  curve: ["tone_curve.points"],
  detail: ["detail."],
  grading: [
    "color_grade.model",
    "color_grade.shadows_",
    "color_grade.midtones_",
    "color_grade.highlights_",
    "color_grade.shadow_range",
    "color_grade.highlight_range",
  ],
  crop: ["crop."],
  mask: [],
  retouch: [],
  camera: ["calibration.", "lut."],
  presets: [],
};

function matchesSection(path: string, id: SectionId): boolean {
  return SECTION_PREFIXES[id].some((p) => path === p || path.startsWith(p));
}

function specsFor(id: SectionId, specs: readonly ParamSpec[]): ParamSpec[] {
  return specs.filter((s) => matchesSection(s.path, id));
}

function curveModified(d: EditDocMirror | null): boolean {
  const mod = d?.modules?.tone_curve;
  if (!mod) return false;
  for (const key of ["points", "points_r", "points_g", "points_b"]) {
    const v = mod[key];
    if (Array.isArray(v) && v.length > 0) return true;
  }
  return false;
}

export function sectionIsModified(
  id: SectionId,
  d: EditDocMirror | null,
  specs: readonly ParamSpec[],
  meta: ImageMeta | null,
  maskId: string | null,
): boolean {
  if (id === "mask") return (d?.masks?.length ?? 0) > 0;
  if (id === "retouch") return (d?.retouch?.length ?? 0) > 0;
  if (id === "curve" && curveModified(d)) return true;
  if (id === "camera" && (d?.meta as { lut_file?: string } | undefined)?.lut_file) return true;
  for (const spec of specsFor(id, specs)) {
    if (spec.path.startsWith("tone_curve.points")) continue;
    const v = effectiveValue(d, spec, meta, maskId);
    const def = defaultValue(spec, meta, maskId);
    if (v !== def) return true;
  }
  return false;
}

export function sectionCanReset(id: SectionId): boolean {
  return id !== "mask" && id !== "retouch" && id !== "presets";
}

export async function resetSection(
  id: SectionId,
  specs: readonly ParamSpec[],
  meta: ImageMeta | null,
  maskId: string | null,
): Promise<void> {
  if (!sectionCanReset(id)) return;
  if (id === "crop") {
    reconcile(await applyOp({ op: "reset_module", module: "crop" }));
    return;
  }
  if (id === "detail") {
    reconcile(await applyOp({ op: "reset_module", module: "detail" }));
    return;
  }
  if (id === "curve") {
    for (const path of [
      "tone_curve.points",
      "tone_curve.points_r",
      "tone_curve.points_g",
      "tone_curve.points_b",
    ]) {
      reconcile(await setParam(path, []));
    }
    return;
  }
  for (const spec of specsFor(id, specs)) {
    if (spec.path.startsWith("tone_curve.points")) continue;
    const def = defaultValue(spec, meta, maskId);
    reconcile(await setParam(spec.path, def));
  }
}
