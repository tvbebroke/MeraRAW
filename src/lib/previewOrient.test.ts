import { describe, expect, it } from "vitest";
import { axesDisagree, hintDestDims, openingRotateDeg } from "./previewOrient";

describe("axesDisagree", () => {
  it("detects a landscape thumb vs a portrait working image", () => {
    expect(axesDisagree(640, 480, 4000, 6000)).toBe(true);
    expect(axesDisagree(480, 640, 4000, 6000)).toBe(false);
    expect(axesDisagree(640, 480, 6000, 4000)).toBe(false);
  });

  it("ignores degenerate sizes", () => {
    expect(axesDisagree(0, 10, 4000, 6000)).toBe(false);
  });
});

describe("openingRotateDeg", () => {
  it("uses 270 for EXIF 8-style tags", () => {
    expect(openingRotateDeg("Rotate270")).toBe(270);
    expect(openingRotateDeg("Transverse")).toBe(270);
  });

  it("defaults to 90 for EXIF 6 and unknown", () => {
    expect(openingRotateDeg("Rotate90")).toBe(90);
    expect(openingRotateDeg(null)).toBe(90);
  });
});

describe("hintDestDims", () => {
  it("keeps catalog dims when they are already portrait", () => {
    expect(hintDestDims({ w: 4000, h: 6000 })).toEqual({ w: 4000, h: 6000 });
  });

  it("swaps landscape catalog dims when EXIF says 90/270", () => {
    expect(hintDestDims({ w: 6000, h: 4000, orientation: "Rotate90" })).toEqual({
      w: 4000,
      h: 6000,
    });
    expect(hintDestDims({ w: 6000, h: 4000 })).toBeNull();
    expect(hintDestDims(null)).toBeNull();
  });
});
