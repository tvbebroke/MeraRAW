import { describe, expect, it } from "vitest";
import { shouldCommitCropSession } from "./cropSession";

describe("shouldCommitCropSession", () => {
  it("skips a no-op leave and commits after any crop edit", () => {
    expect(shouldCommitCropSession(false, false)).toBe(false);
    expect(shouldCommitCropSession(true, false)).toBe(true);
    expect(shouldCommitCropSession(false, true)).toBe(true);
  });
});
