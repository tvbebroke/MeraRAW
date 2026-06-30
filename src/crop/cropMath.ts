import type { CropRect } from "./cropConstants";
import { DEFAULT_CROP, MIN_CROP_SIZE } from "./cropConstants";

export type CropParams = {
  rect: CropRect;
  angle: number;
  rotate90: number;
  flipH: boolean;
  flipV: boolean;
  aspectLocked: boolean;
  aspectW: number;
  aspectH: number;
  constrainCrop: boolean;
};

export function readCropFromDoc(
  modules: Record<string, Record<string, unknown>> | undefined,
): CropParams {
  const c = modules?.crop ?? {};
  const num = (k: string, d: number) => {
    const v = c[k];
    return typeof v === "number" && Number.isFinite(v) ? v : d;
  };
  const bool = (k: string, d: boolean) => {
    const v = c[k];
    return typeof v === "number" ? v >= 0.5 : d;
  };
  return {
    rect: {
      left: num("left", DEFAULT_CROP.left),
      top: num("top", DEFAULT_CROP.top),
      right: num("right", DEFAULT_CROP.right),
      bottom: num("bottom", DEFAULT_CROP.bottom),
    },
    angle: num("angle", 0),
    rotate90: Math.round(num("rotate_90", 0)) % 4,
    flipH: bool("flip_h", false),
    flipV: bool("flip_v", false),
    aspectLocked: bool("aspect_locked", false),
    aspectW: num("aspect_w", 0),
    aspectH: num("aspect_h", 0),
    constrainCrop: bool("constrain_crop", true),
  };
}

export function cropModulesPatch(p: CropParams): Record<string, number> {
  return {
    "crop.left": p.rect.left,
    "crop.top": p.rect.top,
    "crop.right": p.rect.right,
    "crop.bottom": p.rect.bottom,
    "crop.angle": p.angle,
    "crop.rotate_90": p.rotate90,
    "crop.flip_h": p.flipH ? 1 : 0,
    "crop.flip_v": p.flipV ? 1 : 0,
    "crop.aspect_locked": p.aspectLocked ? 1 : 0,
    "crop.aspect_w": p.aspectW,
    "crop.aspect_h": p.aspectH,
    "crop.constrain_crop": p.constrainCrop ? 1 : 0,
  };
}

export function clampRect(r: CropRect): CropRect {
  let { left, top, right, bottom } = r;
  left = Math.max(0, Math.min(1, left));
  top = Math.max(0, Math.min(1, top));
  right = Math.max(0, Math.min(1, right));
  bottom = Math.max(0, Math.min(1, bottom));
  if (right - left < MIN_CROP_SIZE) right = Math.min(1, left + MIN_CROP_SIZE);
  if (bottom - top < MIN_CROP_SIZE) bottom = Math.min(1, top + MIN_CROP_SIZE);
  return { left, top, right, bottom };
}

export function aspectRatio(r: CropRect, imgW: number, imgH: number): number {
  const w = (r.right - r.left) * imgW;
  const h = (r.bottom - r.top) * imgH;
  return w / Math.max(h, 1e-6);
}

export function applyAspectToRect(
  r: CropRect,
  ratio: number,
  imgW: number,
  imgH: number,
  anchor: "center" | "tl" | "tr" | "bl" | "br" = "center",
): CropRect {
  const cx = (r.left + r.right) * 0.5;
  const cy = (r.top + r.bottom) * 0.5;
  const curW = r.right - r.left;
  const curPxW = curW * imgW;
  const targetPxH = curPxW / ratio;
  let newH = targetPxH / imgH;
  if (newH > 1) {
    newH = 1;
  }
  let newW = (newH * imgH * ratio) / imgW;
  if (newW > 1) newW = 1;
  let left = cx - newW * 0.5;
  let top = cy - newH * 0.5;
  if (anchor === "tl") {
    left = r.left;
    top = r.top;
  } else if (anchor === "tr") {
    left = r.right - newW;
    top = r.top;
  } else if (anchor === "bl") {
    left = r.left;
    top = r.bottom - newH;
  } else if (anchor === "br") {
    left = r.right - newW;
    top = r.bottom - newH;
  }
  return clampRect({ left, top, right: left + newW, bottom: top + newH });
}

export type HandleId =
  | "nw"
  | "n"
  | "ne"
  | "e"
  | "se"
  | "s"
  | "sw"
  | "w";

export function dragHandle(
  r: CropRect,
  handle: HandleId,
  dx: number,
  dy: number,
  opts: {
    imgW: number;
    imgH: number;
    aspectLocked: boolean;
    aspectRatio: number;
    fromCenter: boolean;
    tempAspect: boolean;
  },
): CropRect {
  let { left, top, right, bottom } = r;
  const lock = opts.aspectLocked || opts.tempAspect;
  const ratio = opts.aspectRatio;

  const moveEdge = () => {
    if (handle.includes("w")) left += dx;
    if (handle.includes("e")) right += dx;
    if (handle.includes("n")) top += dy;
    if (handle.includes("s")) bottom += dy;
  };

  if (opts.fromCenter) {
    const cx = (left + right) * 0.5;
    const cy = (top + bottom) * 0.5;
    const hw = (right - left) * 0.5;
    const hh = (bottom - top) * 0.5;
    let nhw = hw;
    let nhh = hh;
    if (handle.includes("w") || handle.includes("e")) nhw = Math.max(MIN_CROP_SIZE * 0.5, hw + dx * (handle.includes("w") ? -1 : 1));
    if (handle.includes("n") || handle.includes("s")) nhh = Math.max(MIN_CROP_SIZE * 0.5, hh + dy * (handle.includes("n") ? -1 : 1));
    if (lock && ratio > 0) {
      const pxW = nhw * 2 * opts.imgW;
      nhh = pxW / ratio / opts.imgH * 0.5;
    }
    return clampRect({
      left: cx - nhw,
      top: cy - nhh,
      right: cx + nhw,
      bottom: cy + nhh,
    });
  }

  moveEdge();

  if (lock && ratio > 0) {
    const w = right - left;
    const pxW = w * opts.imgW;
    const newH = pxW / ratio / opts.imgH;
    if (handle.includes("n")) top = bottom - newH;
    else if (handle.includes("s")) bottom = top + newH;
    else if (handle.includes("w") || handle.includes("e")) {
      const h = bottom - top;
      const pxH = h * opts.imgH;
      const newW = (pxH * ratio) / opts.imgW;
      if (handle.includes("w")) left = right - newW;
      else right = left + newW;
    } else {
      // corner
      const anchor = handle === "nw" ? "br" : handle === "ne" ? "bl" : handle === "sw" ? "tr" : "tl";
      const fixed =
        anchor === "br"
          ? { x: right, y: bottom }
          : anchor === "bl"
            ? { x: left, y: bottom }
            : anchor === "tr"
              ? { x: right, y: top }
              : { x: left, y: top };
      let w2 = Math.abs(fixed.x - (handle.includes("w") ? left : right));
      let h2 = (w2 * opts.imgW) / ratio / opts.imgH;
      if (handle.includes("n")) {
        top = fixed.y - h2;
        if (handle.includes("w")) left = fixed.x - w2;
        else right = fixed.x + w2;
      } else {
        bottom = fixed.y + h2;
        if (handle.includes("w")) left = fixed.x - w2;
        else right = fixed.x + w2;
      }
    }
  }

  return clampRect({ left, top, right, bottom });
}

export function panCrop(r: CropRect, dx: number, dy: number): CropRect {
  const w = r.right - r.left;
  const h = r.bottom - r.top;
  let left = r.left + dx;
  let top = r.top + dy;
  left = Math.max(0, Math.min(1 - w, left));
  top = Math.max(0, Math.min(1 - h, top));
  return { left, top, right: left + w, bottom: top + h };
}

export function angleFromDrag(start: [number, number], cur: [number, number], center: [number, number]): number {
  const a0 = Math.atan2(start[1] - center[1], start[0] - center[0]);
  const a1 = Math.atan2(cur[1] - center[1], cur[0] - center[0]);
  return ((a1 - a0) * 180) / Math.PI;
}

export function straightenFromLine(
  a: [number, number],
  b: [number, number],
  currentAngle: number,
): number {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  if (Math.hypot(dx, dy) < 0.001) return currentAngle;
  const lineAngle = (Math.atan2(dy, dx) * 180) / Math.PI;
  const target = Math.abs(lineAngle) < 45 ? -lineAngle : lineAngle > 0 ? 90 - lineAngle : -90 - lineAngle;
  return Math.max(-45, Math.min(45, currentAngle + target));
}

export function flipOrientation(aspectW: number, aspectH: number): [number, number] {
  if (aspectW <= 0 || aspectH <= 0) return [aspectW, aspectH];
  return [aspectH, aspectW];
}

export function screenToImageNorm(
  px: number,
  py: number,
  view: { scale: number; centerX: number; centerY: number },
  imgW: number,
  imgH: number,
  outW: number,
  outH: number,
): [number, number] {
  const nx = (view.centerX * imgW + (px - outW / 2) / view.scale) / imgW;
  const ny = (view.centerY * imgH + (py - outH / 2) / view.scale) / imgH;
  return [nx, ny];
}

export function imageNormToScreen(
  nx: number,
  ny: number,
  view: { scale: number; centerX: number; centerY: number },
  imgW: number,
  imgH: number,
  outW: number,
  outH: number,
): [number, number] {
  const px = (nx * imgW - view.centerX * imgW) * view.scale + outW / 2;
  const py = (ny * imgH - view.centerY * imgH) * view.scale + outH / 2;
  return [px, py];
}

export function isDefaultCrop(p: CropParams): boolean {
  const r = p.rect;
  return (
    r.left <= 0.001 &&
    r.top <= 0.001 &&
    r.right >= 0.999 &&
    r.bottom >= 0.999 &&
    Math.abs(p.angle) < 0.01 &&
    p.rotate90 === 0 &&
    !p.flipH &&
    !p.flipV
  );
}
