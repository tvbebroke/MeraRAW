// Property tests for the crop geometry core (plan P0 exit criteria).
// The engine's crop_common.wgsl shares this contract — keep in lockstep.
import { describe, expect, it } from "vitest";
import type { CropRect } from "./cropConstants";
import {
  applyAspectToRect,
  clampRect,
  constrainRectToImage,
  contentDims,
  contentNormToImageNorm,
  cropModeFor,
  dragHandle,
  isGeometryIdentity,
  maxCenteredRect,
  maxInscribedSize,
  readCropFromDoc,
  rotatedDims,
  type CropParams,
} from "./cropMath";

const baseParams: CropParams = {
  rect: { left: 0, top: 0, right: 1, bottom: 1 },
  angle: 0,
  rotate90: 0,
  flipH: false,
  flipV: false,
  aspectLocked: false,
  aspectW: 0,
  aspectH: 0,
  constrainCrop: true,
};

// deterministic LCG so failures reproduce
function rng(seed: number) {
  let s = seed >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 2 ** 32;
  };
}

/** All 4 corners of a rotated-space rect must sit inside the image quad. */
function cornersInsideQuad(
  rect: CropRect,
  angleDeg: number,
  rotW: number,
  rotH: number,
  eps = 1e-3,
): boolean {
  const t = (angleDeg * Math.PI) / 180;
  const c = Math.cos(t);
  const s = Math.sin(t);
  for (const nx of [rect.left, rect.right]) {
    for (const ny of [rect.top, rect.bottom]) {
      const x = (nx - 0.5) * rotW;
      const y = (ny - 0.5) * rotH;
      const qx = x * c + y * s; // display → source (shader rotate by −θ)
      const qy = -x * s + y * c;
      if (Math.abs(qx) > rotW / 2 + eps * rotW) return false;
      if (Math.abs(qy) > rotH / 2 + eps * rotH) return false;
    }
  }
  return true;
}

describe("content dims / mode", () => {
  it("mode mirrors the engine contract", () => {
    expect(cropModeFor(baseParams, false)).toBe(0);
    expect(cropModeFor(baseParams, true)).toBe(0);
    const rectOnly = { ...baseParams, rect: { left: 0.1, top: 0, right: 1, bottom: 1 } };
    expect(cropModeFor(rectOnly, false)).toBe(1);
    expect(cropModeFor(rectOnly, true)).toBe(0);
    const withAngle = { ...rectOnly, angle: 3 };
    expect(cropModeFor(withAngle, true)).toBe(2);
    expect(isGeometryIdentity(withAngle)).toBe(false);
  });

  it("rotate-90 swaps rotated + content dims", () => {
    const p = {
      ...baseParams,
      rotate90: 1,
      rect: { left: 0, top: 0, right: 0.5, bottom: 1 },
    };
    expect(rotatedDims(p, 6000, 4000)).toEqual([4000, 6000]);
    const [w, h] = contentDims(p, 6000, 4000, 1);
    expect([Math.round(w), Math.round(h)]).toEqual([2000, 6000]);
  });
});

describe("maxInscribedSize", () => {
  it("zero angle fills the frame", () => {
    const [w, h] = maxInscribedSize(6000, 4000, 0, 1.5);
    expect(w).toBeCloseTo(6000, 3);
    expect(h).toBeCloseTo(4000, 3);
  });

  it("property: result always fits, at every angle/ratio", () => {
    const rand = rng(42);
    for (let i = 0; i < 20_000; i++) {
      const rotW = 100 + rand() * 8000;
      const rotH = 100 + rand() * 8000;
      const angle = (rand() * 2 - 1) * 45;
      const ratio = 0.2 + rand() * 5;
      const [w, h] = maxInscribedSize(rotW, rotH, angle, ratio);
      expect(w).toBeGreaterThan(0);
      expect(w / h).toBeCloseTo(ratio, 4);
      const rect = {
        left: 0.5 - w / rotW / 2,
        top: 0.5 - h / rotH / 2,
        right: 0.5 + w / rotW / 2,
        bottom: 0.5 + h / rotH / 2,
      };
      expect(cornersInsideQuad(rect, angle, rotW, rotH)).toBe(true);
    }
  });
});

describe("constrainRectToImage", () => {
  it("no-op when the rect already fits", () => {
    const rect = { left: 0.3, top: 0.3, right: 0.7, bottom: 0.7 };
    const out = constrainRectToImage(rect, { ...baseParams, angle: 5 }, 6000, 4000);
    expect(out.left).toBeCloseTo(rect.left, 4);
    expect(out.bottom).toBeCloseTo(rect.bottom, 4);
  });

  it("property: corners inside quad, ratio preserved, center kept when feasible", () => {
    const rand = rng(1337);
    for (let i = 0; i < 20_000; i++) {
      const imgW = Math.round(200 + rand() * 8000);
      const imgH = Math.round(200 + rand() * 8000);
      const angle = (rand() * 2 - 1) * 45;
      const rotate90 = Math.floor(rand() * 4);
      const l = rand() * 0.8;
      const t = rand() * 0.8;
      const rect = clampRect({
        left: l,
        top: t,
        right: l + 0.05 + rand() * (1 - l - 0.05),
        bottom: t + 0.05 + rand() * (1 - t - 0.05),
      });
      const p = { ...baseParams, angle, rotate90 };
      const out = constrainRectToImage(rect, p, imgW, imgH);
      const [rw, rh] = rotatedDims(p, imgW, imgH);
      // 1. always inside the rotated image quad
      expect(cornersInsideQuad(out, angle, rw, rh)).toBe(true);
      // 2. aspect preserved (unless the min-size clamp kicked in)
      const w0 = (rect.right - rect.left) * rw;
      const h0 = (rect.bottom - rect.top) * rh;
      const w1 = (out.right - out.left) * rw;
      const h1 = (out.bottom - out.top) * rh;
      if (w1 / rw > 0.021 && h1 / rh > 0.021) {
        expect(w1 / h1).toBeCloseTo(w0 / h0, 2);
      }
      // 3. never grows
      expect(w1).toBeLessThanOrEqual(w0 * (1 + 1e-6));
      // 4. idempotent-ish: constraining again barely moves it
      const again = constrainRectToImage(out, p, imgW, imgH);
      expect(Math.abs(again.left - out.left)).toBeLessThan(1e-6);
      expect(Math.abs(again.right - out.right)).toBeLessThan(1e-6);
    }
  });

  it("shrinks a full-frame rect under rotation", () => {
    const p = { ...baseParams, angle: 10 };
    const out = constrainRectToImage(baseParams.rect, p, 6000, 4000);
    expect(out.right - out.left).toBeLessThan(1);
    expect(cornersInsideQuad(out, 10, 6000, 4000)).toBe(true);
  });
});

describe("maxCenteredRect", () => {
  it("property: centered, correct ratio, inside quad", () => {
    const rand = rng(7);
    for (let i = 0; i < 5000; i++) {
      const imgW = Math.round(200 + rand() * 8000);
      const imgH = Math.round(200 + rand() * 8000);
      const angle = (rand() * 2 - 1) * 45;
      const ratio = 0.3 + rand() * 4;
      const p = { ...baseParams, angle };
      const out = maxCenteredRect(p, imgW, imgH, ratio);
      expect((out.left + out.right) / 2).toBeCloseTo(0.5, 4);
      expect((out.top + out.bottom) / 2).toBeCloseTo(0.5, 4);
      expect(cornersInsideQuad(out, angle, imgW, imgH)).toBe(true);
    }
  });
});

describe("rect editing invariants (P0 regression net)", () => {
  it("applyAspectToRect preserves center", () => {
    const rect = { left: 0.2, top: 0.1, right: 0.6, bottom: 0.9 };
    const out = applyAspectToRect(rect, 16 / 9, 6000, 4000);
    expect((out.left + out.right) / 2).toBeCloseTo((rect.left + rect.right) / 2, 3);
    expect((out.top + out.bottom) / 2).toBeCloseTo((rect.top + rect.bottom) / 2, 3);
  });

  it("dragHandle keeps rect in bounds under random drags", () => {
    const rand = rng(99);
    const handles = ["nw", "n", "ne", "e", "se", "s", "sw", "w"] as const;
    for (let i = 0; i < 20_000; i++) {
      const rect = clampRect({
        left: rand() * 0.6,
        top: rand() * 0.6,
        right: 0.4 + rand() * 0.6,
        bottom: 0.4 + rand() * 0.6,
      });
      const out = dragHandle(
        rect,
        handles[Math.floor(rand() * 8)],
        (rand() * 2 - 1) * 0.5,
        (rand() * 2 - 1) * 0.5,
        {
          imgW: 6000,
          imgH: 4000,
          aspectLocked: rand() > 0.5,
          aspectRatio: 0.5 + rand() * 2,
          fromCenter: rand() > 0.7,
          tempAspect: rand() > 0.7,
        },
      );
      expect(out.left).toBeGreaterThanOrEqual(0);
      expect(out.top).toBeGreaterThanOrEqual(0);
      expect(out.right).toBeLessThanOrEqual(1);
      expect(out.bottom).toBeLessThanOrEqual(1);
    }
  });

  it("contentNormToImageNorm matches the shader's discrete mapping", () => {
    // rot90=1 CW, mode 2: content (u,v) → original (v, 1−u) — same expectation
    // the Rust GPU test asserts against extract.wgsl
    const p = { ...baseParams, rotate90: 1 };
    const out = contentNormToImageNorm(0.25, 0.75, p, 6000, 4000, 2);
    expect(out).not.toBeNull();
    expect(out![0]).toBeCloseTo(0.75, 6);
    expect(out![1]).toBeCloseTo(0.75, 6);
    // mode 1 pulls through the rect window
    const p2 = {
      ...baseParams,
      rect: { left: 0.25, top: 0.25, right: 0.75, bottom: 0.75 },
    };
    const mid = contentNormToImageNorm(0.5, 0.5, p2, 6000, 4000, 1);
    expect(mid![0]).toBeCloseTo(0.5, 6);
    // blank corners return null under rotation
    const p3 = { ...baseParams, angle: 20 };
    expect(contentNormToImageNorm(0.001, 0.001, p3, 6000, 4000, 2)).toBeNull();
  });

  it("readCropFromDoc round-trips defaults", () => {
    const p = readCropFromDoc(undefined);
    expect(p.rect).toEqual({ left: 0, top: 0, right: 1, bottom: 1 });
    expect(cropModeFor(p, false)).toBe(0);
  });
});
