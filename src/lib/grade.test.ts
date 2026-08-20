import { describe, expect, it } from "vitest";
import { GRADE_MODULES, pickGradeModules } from "./grade";

describe("copy grade", () => {
  it("snapshots color modules without crop or retouch", () => {
    const clip = pickGradeModules(
      {
        exposure: { ev: 0.3 },
        color_grade: { model: 3, shadows_hue: 12 },
        crop: { aspect: 1 },
        detail: { sharpen: 40 },
      },
      "bundled:soft_print",
    );
    expect(clip.lutFile).toBe("bundled:soft_print");
    expect(clip.modules.exposure).toEqual({ ev: 0.3 });
    expect(clip.modules.color_grade.model).toBe(3);
    expect(clip.modules.crop).toBeUndefined();
    expect(clip.modules.detail).toBeUndefined();
    for (const name of GRADE_MODULES) {
      expect(clip.modules[name]).toBeDefined();
    }
  });

  it("clears an empty lut path", () => {
    expect(pickGradeModules({}, "").lutFile).toBeNull();
    expect(pickGradeModules({}, null).lutFile).toBeNull();
  });
});
