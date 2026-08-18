import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { DEBUG_COMMANDS, UNCALLED_OK } from "./contractAllowlist";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../..");

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, name.name);
    if (name.isDirectory()) out.push(...walk(p));
    else if (/\.(ts|svelte)$/.test(name.name)) out.push(p);
  }
  return out;
}

export function extractInvokes(src: string): string[] {
  return [...src.matchAll(/invoke(?:<[^>]*>)?\(\s*["']([a-z0-9_]+)["']/g)].map(
    (m) => m[1],
  );
}

export function extractWrappers(src: string): { fn: string; command: string }[] {
  const out: { fn: string; command: string }[] = [];
  for (const part of src.split(/export function /).slice(1)) {
    const fn = part.match(/^(\w+)/)?.[1];
    const command = part.match(
      /invoke(?:<[^>]*>)?\(\s*["']([a-z0-9_]+)["']/,
    )?.[1];
    if (fn && command) out.push({ fn, command });
  }
  return out;
}

function extractHandler(src: string): string[] {
  const block = src.match(/generate_handler!\[([\s\S]*?)\]\)/);
  if (!block) return [];
  return [...block[1].matchAll(/(?:commands|license|assistant)::(\w+)/g)].map(
    (m) => m[1],
  );
}

function extractTomlAllow(src: string, identifier: string): string[] {
  const parts = src.split("[[permission]]");
  const block = parts.find((p) => p.includes(`identifier = "${identifier}"`));
  if (!block) return [];
  const allow = block.match(/commands\.allow = \[([\s\S]*?)\]/);
  if (!allow) return [];
  return [...allow[1].matchAll(/"([a-z0-9_]+)"/g)].map((m) => m[1]);
}

describe("contract B — IPC commands", () => {
  const handler = extractHandler(
    readFileSync(join(ROOT, "src-tauri/src/main.rs"), "utf8"),
  );
  const toml = readFileSync(
    join(ROOT, "src-tauri/permissions/default.toml"),
    "utf8",
  );
  const releaseAcl = extractTomlAllow(toml, "meraraw-ipc");
  const debugAcl = extractTomlAllow(toml, "allow-dev-probes");
  const srcFiles = walk(join(ROOT, "src"));
  const wrapperFile = join(ROOT, "src/ipc/commands.ts");
  const wrapperSrc = readFileSync(wrapperFile, "utf8");
  const wrappers = extractWrappers(wrapperSrc);
  const wrapperInvokes = new Set(wrappers.map((w) => w.command));
  const productInvokes = new Set<string>();
  for (const f of srcFiles) {
    if (f.endsWith("/ipc/commands.ts")) continue;
    if (f.endsWith(".test.ts")) continue;
    const src = readFileSync(f, "utf8");
    for (const n of extractInvokes(src)) productInvokes.add(n);
    for (const { fn, command } of wrappers) {
      if (new RegExp(`\\b${fn}\\s*\\(`).test(src)) productInvokes.add(command);
    }
  }

  it("capability identifier is meraraw-ipc, never bare default", () => {
    expect(toml).toContain('identifier = "meraraw-ipc"');
    expect(toml).not.toMatch(/identifier\s*=\s*"default"/);
    const cap = JSON.parse(
      readFileSync(join(ROOT, "src-tauri/capabilities/main.json"), "utf8"),
    ) as { identifier: string; permissions: string[] };
    expect(cap.identifier).toBe("main-capability");
    expect(cap.identifier).not.toBe("default");
    expect(cap.permissions).toContain("meraraw-ipc");
  });

  it("every TS invoke name exists in generate_handler", () => {
    const all = new Set([...wrapperInvokes, ...productInvokes]);
    const missing = [...all].filter((n) => !handler.includes(n));
    expect(missing, `TS invoke with no Rust handler: ${missing.join(", ")}`).toEqual(
      [],
    );
  });

  it("every Rust command has a commands.ts wrapper", () => {
    const missing = handler.filter((n) => !wrapperInvokes.has(n));
    expect(missing, `Rust command with no TS wrapper: ${missing.join(", ")}`).toEqual(
      [],
    );
  });

  it("every Rust command is called outside commands.ts or listed in UNCALLED_OK", () => {
    const allowed = new Set(UNCALLED_OK.map((x) => x.command));
    const missing = handler.filter(
      (n) => !productInvokes.has(n) && !allowed.has(n),
    );
    expect(
      missing,
      `Rust command with no product caller: ${missing.join(", ")}`,
    ).toEqual([]);
  });

  it("every non-debug Rust command is in the release ACL", () => {
    const debug = new Set<string>(DEBUG_COMMANDS);
    const missing = handler.filter(
      (n) => !debug.has(n) && !releaseAcl.includes(n),
    );
    expect(missing, `production command missing from meraraw-ipc: ${missing.join(", ")}`).toEqual(
      [],
    );
  });

  it("extractor reads invoke names", () => {
    expect(extractInvokes(`invoke("open_image"); invoke<void>("undo")`)).toEqual([
      "open_image",
      "undo",
    ]);
  });

  it("debug probes are in capabilities-dev only", () => {
    const leaked = DEBUG_COMMANDS.filter((n) => releaseAcl.includes(n));
    expect(leaked, `debug probe in release ACL: ${leaked.join(", ")}`).toEqual([]);
    const missing = DEBUG_COMMANDS.filter((n) => !debugAcl.includes(n));
    expect(missing, `debug probe missing from allow-dev-probes: ${missing.join(", ")}`).toEqual(
      [],
    );
  });

  it("UNCALLED_OK entries still exist on the handler and are not stale", () => {
    for (const entry of UNCALLED_OK) {
      expect(handler, entry.command).toContain(entry.command);
      expect(entry.reason.length).toBeGreaterThan(8);
    }
    const stale = UNCALLED_OK.filter((e) => productInvokes.has(e.command)).map(
      (e) => e.command,
    );
    expect(stale, `UNCALLED_OK now has a product caller: ${stale.join(", ")}`).toEqual(
      [],
    );
  });
});
