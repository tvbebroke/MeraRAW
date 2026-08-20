import { existsSync, readdirSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { GROUP_PANELS, KNOWN_UNEXPOSED } from "./contractAllowlist";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../..");
const COMPONENTS = join(ROOT, "src/lib/components");
const FIXTURE = join(ROOT, "src-tauri/core/tests/fixtures/registry.json");

type SpecDump = { path: string; ty: string; ui: { group: string } };

function walkSvelte(dir: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, name.name);
    if (name.isDirectory()) out.push(...walkSvelte(p));
    else if (name.name.endsWith(".svelte") || name.name.endsWith(".ts")) out.push(p);
  }
  return out;
}

/** Extract ParamRow path literals and setParam("…") paths from source text. */
export function extractReferencedPaths(src: string): string[] {
  const found = new Set<string>();
  for (const m of src.matchAll(/path="([a-z0-9_.]+)"/g)) found.add(m[1]);
  for (const m of src.matchAll(/path=\{`([^`]+)`\}/g)) found.add(m[1]);
  for (const m of src.matchAll(/setParam\(\s*"([a-z0-9_.]+)"/g)) found.add(m[1]);
  for (const m of src.matchAll(/"(crop\.[a-z0-9_]+)":/g)) found.add(m[1]);
  for (const m of src.matchAll(/[a-z]+: "(tone_curve\.[a-z0-9_]+)"/g)) found.add(m[1]);
  for (const m of src.matchAll(/(?:hue|sat|lum|range): "([a-z0-9_.]+)"/g)) found.add(m[1]);
  return [...found];
}

function expandDynamic(paths: string[], bands: string[]): string[] {
  const out = new Set<string>();
  for (const p of paths) {
    if (p.includes("${") && p.startsWith("hsl.")) {
      for (const band of bands) {
        out.add(p.replace(/\$\{[^}]+\}/, band));
      }
    } else {
      out.add(p);
    }
  }
  return [...out];
}

function mixerBands(src: string): string[] {
  const block = src.match(/const mixerColors = \[([\s\S]*?)\] as const/);
  if (!block) return [];
  return [...block[1].matchAll(/id: "([a-z]+)"/g)].map((m) => m[1]);
}

function hslMixerBands(specs: SpecDump[]): string[] {
  return [
    ...new Set(
      specs
        .filter((s) => /^hsl\.[a-z]+\.(hue|sat|lum)$/.test(s.path))
        .map((s) => s.path.split(".")[1]),
    ),
  ];
}

describe("contract A — param registry", () => {
  it("committed registry.json fixture exists", () => {
    expect(existsSync(FIXTURE), "run UPDATE_FIXTURES=1 cargo test -p meratech-core --lib").toBe(
      true,
    );
  });

  it("extractor expands dynamic hsl ParamRow templates", () => {
    const src = '<ParamRow path={`hsl.${activeColor}.hue`} />';
    expect(extractReferencedPaths(src)).toEqual(["hsl.${activeColor}.hue"]);
  });

  it("extractor catches a ParamRow typo pattern", () => {
    const src = `<ParamRow path="exposure.stops" />\nsetParam("tone_curve.contrast", 1)`;
    expect(extractReferencedPaths(src).sort()).toEqual(
      ["exposure.stops", "tone_curve.contrast"].sort(),
    );
  });

  it("every ParamRow / setParam path exists in the registry", () => {
    const specs = JSON.parse(readFileSync(FIXTURE, "utf8")) as SpecDump[];
    const registry = new Set(specs.map((s) => s.path));
    const hslBands = hslMixerBands(specs);

    const files = [
      ...walkSvelte(COMPONENTS),
      join(ROOT, "src/crop/cropMath.ts"),
    ];
    const raw: string[] = [];
    for (const file of files) raw.push(...extractReferencedPaths(readFileSync(file, "utf8")));
    const referenced = expandDynamic(raw, hslBands).filter((p) => p.includes("."));

    const missing = referenced.filter((p) => !registry.has(p));
    expect(missing, `unknown param paths: ${missing.join(", ")}`).toEqual([]);
  });

  it("every registry path is referenced or listed in KNOWN_UNEXPOSED", () => {
    const specs = JSON.parse(readFileSync(FIXTURE, "utf8")) as SpecDump[];
    const hslBands = hslMixerBands(specs);
    const files = [
      ...walkSvelte(COMPONENTS),
      join(ROOT, "src/crop/cropMath.ts"),
    ];
    const raw: string[] = [];
    for (const file of files) raw.push(...extractReferencedPaths(readFileSync(file, "utf8")));
    const referenced = new Set(expandDynamic(raw, hslBands));
    const allowed = new Set(KNOWN_UNEXPOSED.map((x) => x.path));

    const orphaned = specs.map((s) => s.path).filter((p) => !referenced.has(p) && !allowed.has(p));
    expect(orphaned, `unexposed registry paths: ${orphaned.join(", ")}`).toEqual([]);

    const stale = KNOWN_UNEXPOSED.filter((x) => referenced.has(x.path)).map((x) => x.path);
    expect(stale, `KNOWN_UNEXPOSED entries that are now wired: ${stale.join(", ")}`).toEqual([]);

    const staleAllow = KNOWN_UNEXPOSED.filter((x) => !specs.some((s) => s.path === x.path)).map(
      (x) => x.path,
    );
    expect(
      staleAllow,
      `KNOWN_UNEXPOSED paths missing from registry: ${staleAllow.join(", ")}`,
    ).toEqual([]);

    for (const entry of KNOWN_UNEXPOSED) {
      expect(entry.reason.length).toBeGreaterThan(8);
    }
  });

  it("ColorSettings mixer bands match the registry hsl.* set", () => {
    const specs = JSON.parse(readFileSync(FIXTURE, "utf8")) as SpecDump[];
    const fromReg = hslMixerBands(specs).sort();
    const ui = mixerBands(
      readFileSync(join(COMPONENTS, "edit-panel/ColorSettings.svelte"), "utf8"),
    );
    expect([...ui].sort()).toEqual(fromReg);
    expect(ui).toHaveLength(fromReg.length);
    for (const band of fromReg) {
      for (const param of ["hue", "sat", "lum"]) {
        expect(specs.some((s) => s.path === `hsl.${band}.${param}`)).toBe(true);
      }
    }
  });

  it("every ui.group maps to a known panel that exists on disk", () => {
    const specs = JSON.parse(readFileSync(FIXTURE, "utf8")) as SpecDump[];
    const groups = [...new Set(specs.map((s) => s.ui.group))];
    const unknown = groups.filter((g) => !GROUP_PANELS[g]);
    expect(unknown, `unmapped ui.group: ${unknown.join(", ")}`).toEqual([]);
    const panelDir = join(COMPONENTS, "edit-panel");
    for (const files of Object.values(GROUP_PANELS)) {
      for (const file of files) {
        expect(existsSync(join(panelDir, file)), file).toBe(true);
      }
    }
  });

  it("meraraw-ui mock registry matches the engine fixture when present", () => {
    const mock = join(homedir(), "Desktop/meraraw-ui/mock/registry.ts");
    if (!existsSync(mock)) return;
    const specs = JSON.parse(readFileSync(FIXTURE, "utf8")) as SpecDump[];
    const engine = new Set(specs.map((s) => s.path));
    const src = readFileSync(mock, "utf8");
    const mockPaths = new Set<string>();
    for (const m of src.matchAll(/\bf\(\s*"([a-z0-9_.]+)"/g)) mockPaths.add(m[1]);
    for (const m of src.matchAll(/\bcurve\(\s*"([a-z0-9_.]+)"/g)) mockPaths.add(m[1]);
    const bandsBlock = src.match(/const HSL_BANDS:[^=]*=\s*\[([\s\S]*?)\];/);
    const bands = bandsBlock
      ? [...bandsBlock[1].matchAll(/\["([a-z]+)"/g)].map((m) => m[1])
      : [];
    for (const band of bands) {
      for (const param of ["hue", "sat", "lum"]) {
        mockPaths.add(`hsl.${band}.${param}`);
      }
    }
    expect(mockPaths.size).toBeGreaterThan(0);
    const missing = [...engine].filter((p) => !mockPaths.has(p));
    const extra = [...mockPaths].filter((p) => !engine.has(p));
    expect(missing, `mock missing engine paths: ${missing.join(", ")}`).toEqual([]);
    expect(extra, `mock extra paths: ${extra.join(", ")}`).toEqual([]);
  });
});
