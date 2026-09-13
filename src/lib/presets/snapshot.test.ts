import { describe, expect, it } from "vitest";
import {
  applyPresetLook,
  f32,
  paintBaseScene,
  renderPostcardSnapshot,
  SNAP_H,
  SNAP_W,
} from "./snapshot";

function grayPixel(v = 128): Uint8ClampedArray {
  return new Uint8ClampedArray([v, v, v, 255]);
}

describe("preset snapshot grade", () => {
  it("reads numeric module params", () => {
    expect(f32({ exposure: { stops: 0.5 } }, "exposure", "stops")).toBe(0.5);
    expect(f32({}, "exposure", "stops")).toBe(0);
  });

  it("brightens mid gray with +1 stop", () => {
    const a = grayPixel();
    applyPresetLook(a, 1, 1, { exposure: { stops: 1 } });
    expect(a[0]).toBeGreaterThan(200);
    expect(a[0]).toBe(a[1]);
  });

  it("collapses chroma for a B&W preset", () => {
    const a = new Uint8ClampedArray([200, 80, 60, 255]);
    applyPresetLook(a, 1, 1, { color_grade: { global_chroma: -100 } });
    expect(a[0]).toBe(a[1]);
    expect(a[1]).toBe(a[2]);
  });

  it("warms a pixel when temperature is high", () => {
    const a = grayPixel(160);
    applyPresetLook(a, 1, 1, { white_balance: { temp: 7500 } });
    expect(a[0]).toBeGreaterThan(a[2]);
  });

  it("paints an opaque postcard", () => {
    const data = new Uint8ClampedArray(SNAP_W * SNAP_H * 4);
    paintBaseScene(data, SNAP_W, SNAP_H);
    expect(data[3]).toBe(255);
    const sky = data[2];
    const ground = data[(SNAP_H - 1) * SNAP_W * 4 + 1];
    expect(sky).toBeGreaterThan(ground);
  });

  it("encodes a data URL without a photo", () => {
    // jsdom may lack canvas — skip when unavailable.
    if (typeof document === "undefined" || !document.createElement("canvas").getContext) {
      return;
    }
    const url = renderPostcardSnapshot({
      color_grade: { global_chroma: -100 },
      tone_curve: { contrast: 20 },
    });
    expect(url.startsWith("data:image/")).toBe(true);
  });
});
