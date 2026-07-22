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
  /** Vertical keystone −100..100 (engine maps to −1..1). */
  perspVertical: number;
  /** Horizontal keystone −100..100. */
  perspHorizontal: number;
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
    perspVertical: num("persp_vertical", 0),
    perspHorizontal: num("persp_horizontal", 0),
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
    "crop.persp_vertical": p.perspVertical,
    "crop.persp_horizontal": p.perspHorizontal,
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

// ---------------------------------------------------------------------------
// Content-space + rotation geometry (mirrors the engine's crop_common.wgsl /
// crop.rs contract — keep in lockstep).
// ---------------------------------------------------------------------------

/** Angle/rotate-90/flip/perspective only (rect ignored) — mirrors Rust. */
export function isGeometryIdentity(p: CropParams): boolean {
  return (
    Math.abs(p.angle) < 0.001 &&
    p.rotate90 === 0 &&
    !p.flipH &&
    !p.flipV &&
    Math.abs(p.perspVertical) < 0.1 &&
    Math.abs(p.perspHorizontal) < 0.1
  );
}

/** Inverse keystone — mirrors crop_inv_perspective in crop_common.wgsl. */
export function invPerspective(
  nx: number,
  ny: number,
  perspV: number,
  perspH: number,
): [number, number] | null {
  let x = nx;
  let y = ny;
  const v = perspV / 100;
  const h = perspH / 100;
  if (Math.abs(v) > 1e-5) {
    const sx = Math.max(1 + v * (1 - 2 * y), 0.05);
    x = 0.5 + (x - 0.5) / sx;
  }
  if (Math.abs(h) > 1e-5) {
    const sy = Math.max(1 + h * (1 - 2 * x), 0.05);
    y = 0.5 + (y - 0.5) / sy;
  }
  if (x < 0 || x > 1 || y < 0 || y > 1) return null;
  return [x, y];
}

/** Extract crop mode: 0 = none, 1 = committed crop, 2 = geometry-only preview. */
export function cropModeFor(p: CropParams, cropActive: boolean): 0 | 1 | 2 {
  if (!cropActive && !isDefaultCrop(p)) return 1;
  if (cropActive && !isGeometryIdentity(p)) return 2;
  return 0;
}

/** Physical px dims of the post-rotate-90 space the crop rect lives in. */
export function rotatedDims(p: CropParams, imgW: number, imgH: number): [number, number] {
  return p.rotate90 % 2 === 1 ? [imgH, imgW] : [imgW, imgH];
}

/**
 * Pixel dims of the displayed content — what the viewport's pan/zoom and the
 * engine's fit-scale are defined against.
 */
export function contentDims(
  p: CropParams,
  imgW: number,
  imgH: number,
  mode: 0 | 1 | 2,
): [number, number] {
  const [rw, rh] = rotatedDims(p, imgW, imgH);
  if (mode === 1) {
    return [
      Math.max(Math.max(p.rect.right - p.rect.left, 0.01) * rw, 1),
      Math.max(Math.max(p.rect.bottom - p.rect.top, 0.01) * rh, 1),
    ];
  }
  if (mode === 2) return [rw, rh];
  return [imgW, imgH];
}

/**
 * Largest centered axis-aligned rect of aspect `ratio` (w/h, px) inside the
 * rotated image (closed form): w·|cosθ| + h·|sinθ| ≤ W and w·|sinθ| + h·|cosθ| ≤ H.
 * Returns [w, h] in px of rotated space.
 */
export function maxInscribedSize(
  rotW: number,
  rotH: number,
  angleDeg: number,
  ratio: number,
): [number, number] {
  const t = (Math.abs(angleDeg) * Math.PI) / 180;
  const c = Math.cos(t);
  const s = Math.sin(t);
  const r = Math.max(ratio, 1e-6);
  const h = Math.min(rotW / (r * c + s), rotH / (r * s + c));
  return [r * h, h];
}

/**
 * Constrain-to-image: keep the rect's exact aspect, cap its size to the
 * largest rect of that aspect that fits anywhere in the rotated image, then
 * slide the center the minimal amount needed so no corner leaves the image
 * quad. Never shows blank pixels; idempotent; preserves the user's framing
 * as much as geometry allows. Rect is normalized in rotated space.
 */
export function constrainRectToImage(
  rect: CropRect,
  p: CropParams,
  imgW: number,
  imgH: number,
): CropRect {
  const [rw, rh] = rotatedDims(p, imgW, imgH);
  const t = (p.angle * Math.PI) / 180;
  const c = Math.cos(t);
  const s = Math.sin(t);
  const hw = rw / 2;
  const hh = rh / 2;

  const w0 = Math.max((rect.right - rect.left) * rw, 1e-6);
  const h0 = Math.max((rect.bottom - rect.top) * rh, 1e-6);
  let dx = ((rect.left + rect.right) / 2 - 0.5) * rw;
  let dy = ((rect.top + rect.bottom) / 2 - 0.5) * rh;

  // display → source (matches the shader's rotate-by-−θ)
  const inv = (x: number, y: number): [number, number] => [x * c + y * s, -x * s + y * c];

  // 1. If rotation left the rect's CENTER outside the image quad, pull it
  //    radially toward the image center just far enough to get back inside
  //    (the origin is inside both the quad and the [0,1] box, so the segment
  //    always crosses into the feasible region).
  {
    const [ax, ay] = inv(dx, dy);
    const over = Math.max(Math.abs(ax) / hw, Math.abs(ay) / hh);
    if (over > 1) {
      const shrink = 1 / over;
      dx *= shrink;
      dy *= shrink;
    }
  }

  // 2. Largest k ∈ [0,1] scaling (w0,h0) about the fixed center such that
  //    every corner stays inside BOTH the rotated image quad (source-space
  //    box) and the [0,1] storage box (display-space box). All constraints
  //    are linear in k, so this is exact and idempotent.
  let k = 1;
  const [ax, ay] = inv(dx, dy);
  for (const sx of [-1, 1]) {
    for (const sy of [-1, 1]) {
      const [bx, by] = inv((sx * w0) / 2, (sy * h0) / 2);
      for (const [A, B, C] of [
        [ax, bx, hw],
        [ay, by, hh],
        [dx, (sx * w0) / 2, hw],
        [dy, (sy * h0) / 2, hh],
      ] as const) {
        if (Math.abs(B) < 1e-12) continue;
        const lim = B > 0 ? (C - A) / B : (C + A) / -B;
        k = Math.min(k, Math.max(lim, 0));
      }
    }
  }

  const w = w0 * k;
  const h = h0 * k;
  return {
    left: (dx - w / 2) / rw + 0.5,
    top: (dy - h / 2) / rh + 0.5,
    right: (dx + w / 2) / rw + 0.5,
    bottom: (dy + h / 2) / rh + 0.5,
  };
}

/**
 * Largest-area fit: recenter to the image center with the given ratio (px
 * ratio in rotated space) — darktable's "largest area" auto-crop.
 */
export function maxCenteredRect(
  p: CropParams,
  imgW: number,
  imgH: number,
  ratio: number,
): CropRect {
  const [rw, rh] = rotatedDims(p, imgW, imgH);
  const [w, h] = maxInscribedSize(rw, rh, p.angle, ratio);
  const wn = Math.min(w / rw, 1);
  const hn = Math.min(h / rh, 1);
  return {
    left: 0.5 - wn / 2,
    top: 0.5 - hn / 2,
    right: 0.5 + wn / 2,
    bottom: 0.5 + hn / 2,
  };
}

/**
 * Map a point in CONTENT-normalized coords (what the viewport displays) back
 * to ORIGINAL-image normalized coords (mask/WB space) — TS mirror of the
 * shader chain in crop_common.wgsl. Returns null when the point falls on
 * blank (outside-the-image) pixels.
 */
export function contentNormToImageNorm(
  nx: number,
  ny: number,
  p: CropParams,
  imgW: number,
  imgH: number,
  mode: 0 | 1 | 2,
): [number, number] | null {
  if (nx < 0 || nx > 1 || ny < 0 || ny > 1) return null;
  if (mode === 0) return [nx, ny];
  let rx = nx;
  let ry = ny;
  if (mode === 1) {
    rx = p.rect.left + nx * Math.max(p.rect.right - p.rect.left, 0.01);
    ry = p.rect.top + ny * Math.max(p.rect.bottom - p.rect.top, 0.01);
  }
  const persp = invPerspective(rx, ry, p.perspVertical, p.perspHorizontal);
  if (!persp) return null;
  rx = persp[0];
  ry = persp[1];
  const [rw, rh] = rotatedDims(p, imgW, imgH);
  const t = (-p.angle * Math.PI) / 180; // shader rotates by −angle
  const c = Math.cos(t);
  const s = Math.sin(t);
  const px = (rx - 0.5) * rw;
  const py = (ry - 0.5) * rh;
  let ux = (px * c - py * s) / rw + 0.5;
  let uy = (px * s + py * c) / rh + 0.5;
  // inverse discrete (flips first, then rotate-90 — same as the shader)
  if (p.flipV) uy = 1 - uy;
  if (p.flipH) ux = 1 - ux;
  if (p.rotate90 === 3) [ux, uy] = [1 - uy, ux];
  else if (p.rotate90 === 2) [ux, uy] = [1 - ux, 1 - uy];
  else if (p.rotate90 === 1) [ux, uy] = [uy, 1 - ux];
  if (ux < 0 || ux > 1 || uy < 0 || uy > 1) return null;
  return [ux, uy];
}

/** Internal — only `cropModeFor` needs this; not part of the module's API. */
function isDefaultCrop(p: CropParams): boolean {
  const r = p.rect;
  return (
    r.left <= 0.001 &&
    r.top <= 0.001 &&
    r.right >= 0.999 &&
    r.bottom >= 0.999 &&
    Math.abs(p.angle) < 0.01 &&
    p.rotate90 === 0 &&
    !p.flipH &&
    !p.flipV &&
    Math.abs(p.perspVertical) < 0.1 &&
    Math.abs(p.perspHorizontal) < 0.1
  );
}
