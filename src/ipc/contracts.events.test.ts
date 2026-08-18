import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { EVENTS } from "./events";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../..");

export function extractRustEventNames(src: string): string[] {
  return [...src.matchAll(/pub const [A-Z0-9_]+: &str = "([^"]+)";/g)].map(
    (m) => m[1],
  );
}

describe("contract C — event names", () => {
  const rust = extractRustEventNames(
    readFileSync(join(ROOT, "src-tauri/src/events.rs"), "utf8"),
  );
  const ts = Object.values(EVENTS);

  it("every Rust event const has a matching TS EVENTS entry", () => {
    const missing = rust.filter((n) => !ts.includes(n as (typeof ts)[number]));
    expect(missing, `Rust event missing from events.ts: ${missing.join(", ")}`).toEqual(
      [],
    );
  });

  it("every TS EVENTS value has a matching Rust const", () => {
    const missing = ts.filter((n) => !rust.includes(n));
    expect(missing, `TS event missing from events.rs: ${missing.join(", ")}`).toEqual(
      [],
    );
  });

  it("extractor reads a const", () => {
    expect(extractRustEventNames('pub const FOO: &str = "foo-bar";')).toEqual([
      "foo-bar",
    ]);
  });
});
