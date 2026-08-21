import { describe, expect, it } from "vitest";
import {
  editorRoute,
  isEditorRoute,
  isLibraryRoute,
  isVideoRoute,
  libraryRoute,
  workspaceForOpenPath,
} from "./workspace";

describe("photo vs video workspaces", () => {
  it("keeps original MeraRAW on photo routes", () => {
    expect(libraryRoute("photo")).toBe("/library");
    expect(editorRoute("photo")).toBe("/edit");
    expect(isLibraryRoute("/library")).toBe(true);
    expect(isEditorRoute("/edit")).toBe(true);
    expect(isVideoRoute("/library")).toBe(false);
  });

  it("puts the colorist on separate clip/grade routes", () => {
    expect(libraryRoute("video")).toBe("/clips");
    expect(editorRoute("video")).toBe("/grade");
    expect(isLibraryRoute("/clips")).toBe(true);
    expect(isEditorRoute("/grade")).toBe(true);
    expect(isVideoRoute("/grade")).toBe(true);
  });

  it("does not enter the video workspace while it is gated", () => {
    expect(workspaceForOpenPath("/photos/DSC01234.ARW")).toBe("photo");
    expect(workspaceForOpenPath("/clips/A001.mov")).toBe("photo");
    expect(workspaceForOpenPath("n.mp4", "raw")).toBe("photo");
    expect(workspaceForOpenPath("still.tif", "video")).toBe("photo");
  });
});
