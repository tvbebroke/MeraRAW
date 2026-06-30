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
  | "aspects";

export const CROP_OVERLAY_CYCLE: CropOverlayKind[] = [
  "grid",
  "thirds",
  "diagonal",
  "triangle",
  "golden",
  "spiral",
  "aspects",
];

export const ASPECT_PRESETS: { id: string; label: string; w: number; h: number }[] = [
  { id: "as-shot", label: "As Shot", w: 0, h: 0 },
  { id: "original", label: "Original", w: 0, h: 0 },
  { id: "1:1", label: "1 × 1", w: 1, h: 1 },
  { id: "4:5", label: "4 × 5", w: 4, h: 5 },
  { id: "5:7", label: "5 × 7", w: 5, h: 7 },
  { id: "2:3", label: "2 × 3", w: 2, h: 3 },
  { id: "4:3", label: "4 × 3", w: 4, h: 3 },
  { id: "5:4", label: "5 × 4", w: 5, h: 4 },
  { id: "16:9", label: "16 × 9", w: 16, h: 9 },
  { id: "custom", label: "Custom…", w: 0, h: 0 },
];

export const DEFAULT_CROP: CropRect = { left: 0, top: 0, right: 1, bottom: 1 };

export const MIN_CROP_SIZE = 0.02;
