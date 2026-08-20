import { describe, expect, it } from "vitest";
import { extOf, isVideoPath } from "./media";

describe("isVideoPath", () => {
  it("detects common clip containers", () => {
    expect(isVideoPath("/a/b/clip.mov")).toBe(true);
    expect(isVideoPath("C:\\\\media\\\\A.MP4")).toBe(true);
    expect(isVideoPath("n.mkv")).toBe(true);
  });

  it("leaves stills in the photo workspace", () => {
    expect(isVideoPath("/photos/DSC01234.ARW")).toBe(false);
    expect(isVideoPath("shot.jpeg")).toBe(false);
    expect(isVideoPath("scan.tif")).toBe(false);
    expect(isVideoPath(null)).toBe(false);
  });

  it("reads the last extension", () => {
    expect(extOf("/tmp/foo.bar.mov")).toBe("mov");
  });
});
