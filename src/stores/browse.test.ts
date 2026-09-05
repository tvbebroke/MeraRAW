import { describe, expect, it } from "vitest";
import {
  folderKindFor,
  folderSections,
  gridKey,
  gridQueryFromState,
  isLibraryFileRoot,
  isPathInFolder,
  libraryFiltersActive,
  sameFolderPath,
  DEFAULT_LIBRARY_FILTERS,
  type LibraryFilters,
} from "./browse";
import type { FolderItem } from "../ipc/types";

function folder(partial: Partial<FolderItem> & Pick<FolderItem, "root" | "name">): FolderItem {
  return {
    photoCount: 0,
    videoCount: 0,
    accessible: true,
    ...partial,
  };
}

describe("sameFolderPath", () => {
  it("matches slash and trailing-slash variants", () => {
    expect(sameFolderPath("/Users/kai/photos", "/Users/kai/photos/")).toBe(true);
    expect(sameFolderPath(String.raw`C:\Users\kai\photos`, "C:/Users/kai/photos")).toBe(true);
  });

  it("matches macOS /private prefix and //?/ verbatim roots", () => {
    expect(sameFolderPath("/tmp/shoot", "/private/tmp/shoot")).toBe(true);
    expect(sameFolderPath("//?/C:/Users/kai/photos", "C:/Users/kai/photos")).toBe(true);
  });

  it("does not treat unrelated folders as the same because of a shared suffix", () => {
    expect(sameFolderPath("/Users/kai/photos", "/Users/kai/backup/photos")).toBe(false);
    expect(sameFolderPath("/Users/kai/myphotos", "/Users/kai/photos")).toBe(false);
    expect(sameFolderPath("/Users/kai/Desktop/a7iii video", "/Users/kai/Movies/a7iii video")).toBe(
      false,
    );
  });
});

describe("folderSections", () => {
  it("puts mixed auto folders in both rails", () => {
    const mixed = folder({ root: "/shoot", name: "shoot", photoCount: 4, videoCount: 2 });
    expect(folderSections(mixed, "auto")).toEqual(["photo", "video"]);
    expect(folderSections(mixed, undefined)).toEqual(["photo", "video"]);
  });

  it("honors an explicit keep-in override", () => {
    const mixed = folder({ root: "/shoot", name: "shoot", photoCount: 4, videoCount: 2 });
    expect(folderSections(mixed, "photo")).toEqual(["photo"]);
    expect(folderSections(mixed, "video")).toEqual(["video"]);
  });

  it("sends clip-only folders to Videos", () => {
    const clips = folder({ root: "/reels", name: "reels", videoCount: 3 });
    expect(folderSections(clips, "auto")).toEqual(["video"]);
  });
});

describe("folderKindFor", () => {
  it("resolves a stored kind across path variants", () => {
    expect(
      folderKindFor("/private/tmp/shoot", { "/tmp/shoot": "video" }),
    ).toBe("video");
  });
});

describe("isPathInFolder", () => {
  it("treats nested folders as inside the pin", () => {
    expect(isPathInFolder("/Users/kai/photos/day1", "/Users/kai/photos")).toBe(true);
    expect(isPathInFolder("/Users/kai/photos", "/Users/kai/photos")).toBe(true);
    expect(isPathInFolder("/Users/kai/photos-backup/day1", "/Users/kai/photos")).toBe(false);
  });
});

describe("gridQueryFromState", () => {
  const base: LibraryFilters = { ...DEFAULT_LIBRARY_FILTERS };

  it("omits unset filters so the catalog default is unfiltered", () => {
    expect(gridQueryFromState("/shoot", base)).toEqual({
      folder: "/shoot",
      limit: 2000,
      sort: "captured",
      ratingMin: undefined,
      flag: undefined,
      hasEdits: undefined,
      blurryOnly: undefined,
      dupesOnly: undefined,
      text: undefined,
    });
  });

  it("maps pinned collection filters onto GridQuery", () => {
    const filters: LibraryFilters = {
      ...base,
      ratingMin: 4,
      flag: "pick",
      hasEdits: true,
      blurryOnly: true,
      dupesOnly: true,
      text: "  7M4  ",
    };
    expect(gridQueryFromState("/shoot/day1", filters, { limit: 100, sort: "rating" })).toEqual({
      folder: "/shoot/day1",
      limit: 100,
      sort: "rating",
      ratingMin: 4,
      flag: "pick",
      hasEdits: true,
      blurryOnly: true,
      dupesOnly: true,
      text: "7M4",
    });
  });

  it("treats a null folder as catalog-wide", () => {
    expect(gridQueryFromState(null, base).folder).toBeUndefined();
  });

  it("does not send flag when the pin is any", () => {
    expect(gridQueryFromState("/x", { ...base, flag: "any" }).flag).toBeUndefined();
    expect(gridQueryFromState("/x", { ...base, flag: "none" }).flag).toBe("none");
  });
});

describe("libraryFiltersActive", () => {
  it("is false for defaults and true once any pin is set", () => {
    expect(libraryFiltersActive(DEFAULT_LIBRARY_FILTERS)).toBe(false);
    expect(libraryFiltersActive({ ...DEFAULT_LIBRARY_FILTERS, ratingMin: 1 })).toBe(true);
    expect(libraryFiltersActive({ ...DEFAULT_LIBRARY_FILTERS, text: "  " })).toBe(false);
    expect(libraryFiltersActive({ ...DEFAULT_LIBRARY_FILTERS, text: "a7" })).toBe(true);
  });
});

describe("isLibraryFileRoot", () => {
  it("treats a single imported photo as a file, not a folder", () => {
    expect(
      isLibraryFileRoot(folder({ root: "/Alaska/DSCF1234.RAF", name: "DSCF1234.RAF", isFile: true })),
    ).toBe(true);
    expect(
      isLibraryFileRoot(folder({ root: "/Alaska/DSCF1234.RAF", name: "DSCF1234.RAF" })),
    ).toBe(true);
    expect(isLibraryFileRoot(folder({ root: "/Alaska", name: "Alaska" }))).toBe(false);
  });
});

describe("gridKey", () => {
  it("distinguishes virtual copies that share a path", () => {
    expect(gridKey({ path: "/a.ARW" })).toBe("/a.ARW");
    expect(gridKey({ path: "/a.ARW", docId: "vc-1" })).toBe("/a.ARW::vc-1");
    expect(gridKey({ path: "/a.ARW", docId: "vc-1" })).not.toBe(
      gridKey({ path: "/a.ARW", docId: "master" }),
    );
  });
});
