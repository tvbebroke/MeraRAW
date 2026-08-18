import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import type { EditDocMirror } from "./types";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../..");
const FIXTURE = join(ROOT, "src-tauri/core/tests/fixtures/doc-full.json");

function isParamValue(v: unknown): boolean {
  if (typeof v === "boolean") return true;
  if (typeof v === "number") return true;
  if (typeof v === "string") return true;
  if (Array.isArray(v)) {
    return v.every(
      (pt) =>
        Array.isArray(pt) &&
        pt.length === 2 &&
        typeof pt[0] === "number" &&
        typeof pt[1] === "number",
    );
  }
  if (v && typeof v === "object") {
    const o = v as Record<string, unknown>;
    return typeof o.h === "number" && typeof o.s === "number" && typeof o.l === "number";
  }
  return false;
}

function assertEditDocMirror(raw: unknown): asserts raw is EditDocMirror {
  expect(raw).toBeTypeOf("object");
  const doc = raw as EditDocMirror;
  expect(doc.schema_version).toBe(1);
  expect(typeof doc.doc_id).toBe("string");
  expect(doc.source_ref?.path).toBeTruthy();
  expect(Array.isArray(doc.masks)).toBe(true);
  expect(Array.isArray(doc.retouch)).toBe(true);
  expect(doc.masks?.length).toBeGreaterThan(0);
  expect(doc.retouch?.length).toBeGreaterThan(0);
  const modules = doc.modules ?? {};
  const values = Object.values(modules).flatMap((m) => Object.values(m));
  expect(values.some((v) => typeof v === "boolean"), "Bool variant").toBe(true);
  expect(values.some((v) => typeof v === "number"), "F32 variant").toBe(true);
  expect(values.some((v) => typeof v === "string"), "Enum variant").toBe(true);
  expect(values.some((v) => Array.isArray(v)), "Curve variant").toBe(true);
  expect(
    values.some((v) => v && typeof v === "object" && !Array.isArray(v) && "h" in (v as object)),
    "Color variant",
  ).toBe(true);
  expect(values.every(isParamValue)).toBe(true);
}

describe("contract D — doc schema", () => {
  it("SCHEMA_VERSION in types matches core/src/doc.rs", () => {
    const rust = readFileSync(join(ROOT, "src-tauri/core/src/doc.rs"), "utf8");
    const ver = rust.match(/pub const SCHEMA_VERSION: u32 = (\d+)/)?.[1];
    expect(ver).toBe("1");
  });

  it("parses the committed full-doc fixture as EditDocMirror", () => {
    const raw = JSON.parse(readFileSync(FIXTURE, "utf8"));
    assertEditDocMirror(raw);
    expect(raw.meta.profile_file).toBeTruthy();
    expect(raw.meta.lut_file).toBeTruthy();
    expect(raw.meta.demosaic).toBe("rcd");
    expect(raw.masks[0].modules.exposure.stops).toBeTypeOf("number");
  });
});
