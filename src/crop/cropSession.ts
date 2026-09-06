import { atom } from "nanostores";
import { applyCropParams } from "./cropActions";
import { cropParamsEqual, cropWithDraft, type CropParams } from "./cropMath";
import { lastOpenedPath } from "../stores/app";
import { doc, reconcile } from "../stores/doc";

/** In-progress overlay crop. Written once when the crop tool closes. */
export const cropDraft = atom<CropParams | null>(null);

/** True after any crop edit this tool session (overlay, straighten, flip, …). */
let cropGesture = false;

let flushing: Promise<void> | null = null;

export function markCropGesture() {
  cropGesture = true;
}

export function abandonCropSession() {
  cropDraft.set(null);
  cropGesture = false;
}

export function setCropDraft(p: CropParams | null) {
  cropDraft.set(p);
  if (p) cropGesture = true;
}

export function shouldCommitCropSession(changed: boolean, gesture: boolean): boolean {
  return changed || gesture;
}

lastOpenedPath.listen(() => {
  abandonCropSession();
});

export async function flushCropDraft(): Promise<void> {
  if (flushing) return flushing;
  flushing = (async () => {
    const draft = cropDraft.get();
    const modules = doc.get()?.modules;
    const cur = cropWithDraft(modules, null);
    const merged = cropWithDraft(modules, draft);
    const changed = !cropParamsEqual(merged, cur);
    const gesture = cropGesture;
    cropDraft.set(null);
    cropGesture = false;
    if (!shouldCommitCropSession(changed, gesture)) return;
    try {
      reconcile(await applyCropParams(merged, false));
    } catch {
      cropDraft.set(draft);
      cropGesture = gesture;
    }
  })().finally(() => {
    flushing = null;
  });
  return flushing;
}
