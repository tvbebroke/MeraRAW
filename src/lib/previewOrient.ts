/** Catalog thumbs may be stored landscape while the working image is portrait. */

export function axesDisagree(
  srcW: number,
  srcH: number,
  destW: number,
  destH: number,
): boolean {
  if (srcW < 2 || srcH < 2 || destW < 2 || destH < 2) return false;
  return srcH > srcW !== destH > destW;
}

/** Degrees to stand a sideways catalog preview up. EXIF 6 → 90, EXIF 8 → 270. */
export function openingRotateDeg(orientation?: string | null): 90 | 270 {
  if (orientation === "Rotate270" || orientation === "Transverse") return 270;
  return 90;
}

const AXIS_SWAP = new Set(["Rotate90", "Rotate270", "Transpose", "Transverse"]);

export function orientationSwapsAxes(orientation?: string | null): boolean {
  return !!orientation && AXIS_SWAP.has(orientation);
}

/** Working-image size implied by a catalog row + optional EXIF probe. */
export function hintDestDims(
  hint: { w: number; h: number; orientation?: string } | null,
): { w: number; h: number } | null {
  if (!hint || hint.w < 2 || hint.h < 2) return null;
  if (orientationSwapsAxes(hint.orientation)) {
    return { w: Math.min(hint.w, hint.h), h: Math.max(hint.w, hint.h) };
  }
  if (hint.h > hint.w) return hint;
  return null;
}
