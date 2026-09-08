import { describe, expect, it } from "vitest";
import { shouldCommitCropSession } from "./cropSession";

describe("shouldCommitCropSession", () => {
  it("commits only when the crop actually changed", () => {
    expect(shouldCommitCropSession(false, false)).toBe(false);
    expect(shouldCommitCropSession(true, false)).toBe(true);
    expect(shouldCommitCropSession(false, true)).toBe(false);
  });
});
