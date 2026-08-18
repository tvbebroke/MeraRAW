import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { DEFAULT_SECTIONS, type SectionId } from "./editor";

const SECTION_IDS = Object.keys(DEFAULT_SECTIONS) as SectionId[];

describe("DEFAULT_SECTIONS", () => {
  it("gives every SectionId a boolean default", () => {
    expect(SECTION_IDS.length).toBeGreaterThan(0);
    for (const id of SECTION_IDS) {
      expect(typeof DEFAULT_SECTIONS[id]).toBe("boolean");
    }
  });

  it("covers every CollapsibleSection id in the edit panel", () => {
    const dir = join(dirname(fileURLToPath(import.meta.url)), "../lib/components/edit-panel");
    const used = new Set<string>();
    for (const file of readdirSync(dir)) {
      if (!file.endsWith(".svelte")) continue;
      const src = readFileSync(join(dir, file), "utf8");
      for (const m of src.matchAll(/<CollapsibleSection\s+id="([A-Za-z]+)"/g)) {
        used.add(m[1]);
      }
    }
    for (const id of used) {
      expect(SECTION_IDS, `unregistered CollapsibleSection id "${id}"`).toContain(id);
    }
    expect([...used].sort()).toEqual([...SECTION_IDS].sort());
  });
});
