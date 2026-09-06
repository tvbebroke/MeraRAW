import { describe, expect, it } from "vitest";
import { frameUrl } from "./frame";

describe("frameUrl", () => {
  it("puts jpeg version in the path and keeps fmt=jpeg", () => {
    const a = frameUrl(3, "jpeg");
    const b = frameUrl(4, "jpeg");
    expect(a).toContain("/current/3.jpg");
    expect(b).toContain("/current/4.jpg");
    expect(a).toContain("fmt=jpeg");
    expect(a).not.toBe(b);
  });
});
