export type CropRect = {
  left: number;
  top: number;
  right: number;
  bottom: number;
};

export type CropOverlayKind =
  | "none"
  | "grid"
  | "thirds"
  | "diagonal"
  | "triangle"
  | "golden"
  | "spiral"
  | "aspects"
  | "center"
  | "epassport";

export const CROP_OVERLAY_CYCLE: CropOverlayKind[] = [
  "grid",
  "thirds",
  "diagonal",
  "triangle",
  "golden",
  "spiral",
  "aspects",
  "center",
  "epassport",
];

export type AspectPreset = {
  id: string;
  label: string;
  w: number;
  h: number;
  /** Pairs with a dedicated guide overlay (ePassport). */
  guide?: CropOverlayKind;
};

export const ASPECT_PRESETS: AspectPreset[] = [
  { id: "as-shot", label: "As Shot", w: 0, h: 0 },
  { id: "original", label: "Original", w: 0, h: 0 },
  { id: "free", label: "Free", w: 0, h: 0 },
  { id: "1:1", label: "1 × 1", w: 1, h: 1 },
  { id: "4:5", label: "4 × 5", w: 4, h: 5 },
  { id: "5:7", label: "5 × 7", w: 5, h: 7 },
  { id: "2:3", label: "2 × 3", w: 2, h: 3 },
  { id: "4:3", label: "4 × 3", w: 4, h: 3 },
  { id: "5:4", label: "5 × 4", w: 5, h: 4 },
  { id: "16:9", label: "16 × 9", w: 16, h: 9 },
  { id: "16:10", label: "16 × 10", w: 16, h: 10 },
  { id: "2:1", label: "2 × 1", w: 2, h: 1 },
  // print / niche pack (plan P5, RawTherapee-inspired)
  { id: "xpan", label: "XPan 65 × 24", w: 65, h: 24 },
  { id: "a-series", label: "A series (√2)", w: 1.4142, h: 1 },
  { id: "us-letter", label: "US Letter 8.5 × 11", w: 8.5, h: 11 },
  { id: "tabloid", label: "Tabloid 11 × 17", w: 11, h: 17 },
  { id: "epassport", label: "ePassport 45 × 35", w: 45, h: 35, guide: "epassport" },
  { id: "custom", label: "Enter Custom…", w: 0, h: 0 },
];

export const DEFAULT_CROP: CropRect = { left: 0, top: 0, right: 1, bottom: 1 };

export const MIN_CROP_SIZE = 0.02;

/** Parse "x:y", "x/y" or a decimal into a [w, h] ratio pair. */
export function parseRatioInput(input: string): [number, number] | null {
  const s = input.trim();
  const m = s.match(/^(\d+(?:\.\d+)?)\s*[:x/×]\s*(\d+(?:\.\d+)?)$/i);
  if (m) {
    const w = parseFloat(m[1]!);
    const h = parseFloat(m[2]!);
    if (w > 0 && h > 0 && w / h < 100 && h / w < 100) return [w, h];
    return null;
  }
  const dec = parseFloat(s);
  if (Number.isFinite(dec) && dec > 0.01 && dec < 100) return [dec, 1];
  return null;
}

/** Canonical "w:h" display string for a custom ratio. */
export function formatRatio(w: number, h: number): string {
  const fmt = (n: number) => (Number.isInteger(n) ? String(n) : n.toFixed(3).replace(/0+$/, "").replace(/\.$/, ""));
  return `${fmt(w)}:${fmt(h)}`;
}
