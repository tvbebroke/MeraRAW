export type CropRect = {
  left: number;
  top: number;
  right: number;
  bottom: number;
};

export const DEFAULT_CROP: CropRect = { left: 0, top: 0, right: 1, bottom: 1 };

export const MIN_CROP_SIZE = 0.02;
