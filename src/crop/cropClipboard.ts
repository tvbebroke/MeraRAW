// Copy/paste crop settings across images (plan P6). Rect coords are
// normalized, so they transfer between images of different sizes.
import type { CropParams } from "./cropMath";

const KEY = "meraraw.crop.clipboard";

export function copyCropToClipboard(p: CropParams) {
  try {
    localStorage.setItem(KEY, JSON.stringify(p));
  } catch {
    // best-effort
  }
}

export function readCropClipboard(): CropParams | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const p = JSON.parse(raw) as CropParams;
    if (!p || typeof p !== "object" || !p.rect) return null;
    return p;
  } catch {
    return null;
  }
}
