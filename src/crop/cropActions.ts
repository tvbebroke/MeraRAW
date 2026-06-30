import { applyOp } from "../ipc/commands";
import type { CropParams } from "./cropMath";
import { cropModulesPatch } from "./cropMath";

export function applyCropParams(p: CropParams, live = false) {
  const crop: Record<string, number> = {};
  for (const [path, value] of Object.entries(cropModulesPatch(p))) {
    crop[path.replace("crop.", "")] = value;
  }
  return applyOp({ op: "apply_preset", preset: { modules: { crop } } }, live);
}

export async function resetCropModule() {
  return applyOp({ op: "reset_module", module: "crop" });
}
