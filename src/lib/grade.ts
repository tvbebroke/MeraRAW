// Copy/paste grade between stills and clips. Same EditDoc modules + LUT slot.
// Crop / mask / retouch / calibration / detail stay on the destination.
import { atom } from "nanostores";
import { applyOp, getDoc, setLut } from "../ipc/commands";
import { statusMessage } from "../stores/app";
import { reconcile, setDoc } from "../stores/doc";

export const GRADE_MODULES = [
  "exposure",
  "white_balance",
  "color_grade",
  "hsl",
  "tone_curve",
  "lut",
  "effects",
  "input",
] as const;

export type GradeClip = {
  modules: Record<string, Record<string, unknown>>;
  lutFile: string | null;
};

export const gradeClipboard = atom<GradeClip | null>(null);

export function pickGradeModules(
  modules: Record<string, Record<string, unknown>> | undefined,
  lutFile: unknown,
): GradeClip {
  const out: Record<string, Record<string, unknown>> = {};
  for (const name of GRADE_MODULES) {
    out[name] = { ...(modules?.[name] ?? {}) };
  }
  return {
    modules: out,
    lutFile: typeof lutFile === "string" && lutFile.length > 0 ? lutFile : null,
  };
}

export async function copyGrade(): Promise<boolean> {
  const d = await getDoc();
  if (!d) {
    statusMessage.set("Open a photo or clip first");
    return false;
  }
  const clip = pickGradeModules(d.modules, d.meta?.lut_file);
  gradeClipboard.set(clip);
  statusMessage.set("Grade copied");
  return true;
}

export async function pasteGrade(): Promise<boolean> {
  const clip = gradeClipboard.get();
  if (!clip) {
    statusMessage.set("Nothing to paste — copy a grade first");
    return false;
  }
  const d = await getDoc();
  if (!d) {
    statusMessage.set("Open a photo or clip first");
    return false;
  }
  try {
    let last = await applyOp({ op: "reset_module", module: GRADE_MODULES[0] });
    for (const module of GRADE_MODULES.slice(1)) {
      last = await applyOp({ op: "reset_module", module });
    }
    await setLut(clip.lutFile);
    last = await applyOp({ op: "apply_preset", preset: { modules: clip.modules } });
    reconcile(last);
    const fresh = await getDoc();
    if (fresh) setDoc(fresh);
    statusMessage.set("Grade pasted");
    return true;
  } catch (e) {
    statusMessage.set(e instanceof Error ? e.message : String(e));
    return false;
  }
}
