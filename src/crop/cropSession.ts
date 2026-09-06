import { atom } from "nanostores";
import { applyCropParams } from "./cropActions";
import { cropParamsEqual, cropWithDraft, type CropParams } from "./cropMath";
import { doc, reconcile } from "../stores/doc";

/** In-progress overlay crop. Flushed on pointer-up, Enter, Done, or tab leave. */
export const cropDraft = atom<CropParams | null>(null);

let flushing: Promise<void> | null = null;

export function setCropDraft(p: CropParams | null) {
  cropDraft.set(p);
}

export async function flushCropDraft(): Promise<void> {
  if (flushing) return flushing;
  flushing = (async () => {
    const draft = cropDraft.get();
    if (!draft) return;
    const modules = doc.get()?.modules;
    const cur = cropWithDraft(modules, null);
    const merged = cropWithDraft(modules, draft);
    if (cropParamsEqual(merged, cur)) {
      cropDraft.set(null);
      return;
    }
    try {
      reconcile(await applyCropParams(merged, false));
      cropDraft.set(null);
    } catch {
      /* keep the draft so Enter / Done can retry */
    }
  })().finally(() => {
    flushing = null;
  });
  return flushing;
}
