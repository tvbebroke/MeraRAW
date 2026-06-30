#!/usr/bin/env node
/**
 * Convert Lightroom .xmp / .lrtemplate / .zip packs → MeraRAW preset JSON.
 *
 * Usage:
 *   node scripts/import-lightroom-presets.mjs [sourceDir ...]
 */

import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { execFileSync } from "node:child_process";

const ROOT = path.resolve(import.meta.dirname, "..");
const OUT_DIR = path.join(ROOT, "presets", "bundled");
const TMP_DIR = path.join(ROOT, ".preset-import-tmp");

const DEFAULT_SOURCES = [
  path.join(ROOT, "NAKID PRESETS"),
  path.join(ROOT, "Cuba Gallery Lightroom Presets - Pack 8"),
  path.join(ROOT, "450+ Lightroom Presets and Photoshop Actions - [CrackzSoft]"),
  path.join(ROOT, "preset-sources"),
];

const XMP_MAP = [
  ["exposure", "stops", "crs:Exposure2012"],
  ["white_balance", "temp", "crs:Temperature"],
  ["white_balance", "tint", "crs:Tint"],
  ["tone_curve", "contrast", "crs:Contrast2012"],
  ["tone_curve", "highlights", "crs:Highlights2012"],
  ["tone_curve", "shadows", "crs:Shadows2012"],
  ["tone_curve", "lights", "crs:Whites2012"],
  ["tone_curve", "darks", "crs:Blacks2012"],
  ["color_grade", "perceptual_sat", "crs:Vibrance"],
  ["color_grade", "global_chroma", "crs:Saturation"],
  ["color_grade", "shadows_hue", "crs:ColorGradeShadowHue"],
  ["color_grade", "shadows_sat", "crs:ColorGradeShadowSat"],
  ["color_grade", "shadows_lum", "crs:ColorGradeShadowLum"],
  ["color_grade", "midtones_hue", "crs:ColorGradeMidtoneHue"],
  ["color_grade", "midtones_sat", "crs:ColorGradeMidtoneSat"],
  ["color_grade", "midtones_lum", "crs:ColorGradeMidtoneLum"],
  ["color_grade", "highlights_hue", "crs:ColorGradeHighlightHue"],
  ["color_grade", "highlights_sat", "crs:ColorGradeHighlightSat"],
  ["color_grade", "highlights_lum", "crs:ColorGradeHighlightLum"],
  ["detail", "sharpen_amount", "crs:Sharpness"],
  ["detail", "noise_luma", "crs:LuminanceSmoothing"],
  ["detail", "noise_chroma", "crs:ColorNoiseReduction"],
];

const HSL_BANDS = [
  ["red", "Red"],
  ["orange", "Orange"],
  ["yellow", "Yellow"],
  ["green", "Green"],
  ["aqua", "Aqua"],
  ["blue", "Blue"],
  ["purple", "Purple"],
  ["magenta", "Magenta"],
];

const STRIP_RE =
  /\b(nakid|cuba gallery|crackzsoft|lightroom|photoshop actions|presets pack|presets|pack \d+|pack\d+|\[crackzsoft\]|scarlett|aya|modsun|photonify|portrait presets|landscape presets|matte presets|newborn presets|light leaks)\b/gi;

const SKIP_DIR_RE =
  /^(metadata|export|watermark|filename|filter preset|brush|camera raw settings|locales?|help|photoshop actions)$/i;

const SKIP_FILE_RE = /\.(txt|pdf|url|html|exe|dmg|zip)$/i;

function parseXmpValue(raw) {
  const t = raw.trim();
  if (!t || /^false$/i.test(t)) return null;
  if (/^true$/i.test(t)) return 1;
  const n = parseFloat(t.replace(/^\+/, ""));
  return Number.isFinite(n) ? n : null;
}

function parseXmpAttr(text, attr) {
  const needle = `${attr}="`;
  const start = text.indexOf(needle);
  if (start < 0) return null;
  const rest = text.slice(start + needle.length);
  const end = rest.indexOf('"');
  if (end < 0) return null;
  return parseXmpValue(rest.slice(0, end));
}

function xmpToPartialDoc(text) {
  if (!text.includes("crs:")) return null;
  const modules = {};
  const set = (mod, param, val) => {
    if (val === null || val === undefined) return;
    if (!modules[mod]) modules[mod] = {};
    modules[mod][param] = val;
  };

  for (const [mod, param, attr] of XMP_MAP) {
    set(mod, param, parseXmpAttr(text, attr));
  }

  for (const [band, label] of HSL_BANDS) {
    set("hsl", `${band}.hue`, parseXmpAttr(text, `crs:HueAdjustment${label}`));
    set("hsl", `${band}.sat`, parseXmpAttr(text, `crs:SaturationAdjustment${label}`));
    set("hsl", `${band}.lum`, parseXmpAttr(text, `crs:LuminanceAdjustment${label}`));
  }

  if (parseXmpAttr(text, "crs:ConvertToGrayscale") === 1) {
    set("color_grade", "global_chroma", -100);
    set("color_grade", "perceptual_sat", -100);
  }

  if (Object.keys(modules).length === 0) return null;
  return { modules };
}

function lrNum(text, key) {
  const re = new RegExp(`\\b${key}\\s*=\\s*([+-]?\\d+\\.?\\d*)`);
  const m = text.match(re);
  return m ? parseFloat(m[1]) : null;
}

function lrFirst(text, keys) {
  for (const k of keys) {
    const v = lrNum(text, k);
    if (v !== null) return v;
  }
  return null;
}

/** Classic .lrtemplate (Lua table) → PartialDoc */
function lrtemplateToPartialDoc(text) {
  if (!text.includes("settings =")) return null;

  const modules = {};
  const set = (mod, param, val) => {
    if (val === null || val === undefined || !Number.isFinite(val) || val === 0) return;
    if (!modules[mod]) modules[mod] = {};
    modules[mod][param] = val;
  };
  // Allow zero for exposure and WB
  const setAny = (mod, param, val) => {
    if (val === null || val === undefined || !Number.isFinite(val)) return;
    if (!modules[mod]) modules[mod] = {};
    modules[mod][param] = val;
  };

  setAny(
    "exposure",
    "stops",
    lrFirst(text, ["Exposure2012", "Exposure"]),
  );
  setAny("white_balance", "temp", lrNum(text, "Temperature"));
  setAny("white_balance", "tint", lrNum(text, "Tint"));
  set("tone_curve", "contrast", lrFirst(text, ["Contrast2012", "Contrast"]));
  set("tone_curve", "highlights", lrFirst(text, ["Highlights2012", "HighlightRecovery"]));
  set(
    "tone_curve",
    "shadows",
    lrFirst(text, ["Shadows2012", "Shadows", "FillLight"]),
  );
  set("tone_curve", "lights", lrFirst(text, ["Whites2012", "Brightness"]));
  set("tone_curve", "darks", lrFirst(text, ["Blacks2012"]));
  set("color_grade", "perceptual_sat", lrNum(text, "Vibrance"));
  set("color_grade", "global_chroma", lrNum(text, "Saturation"));
  set("color_grade", "shadows_hue", lrNum(text, "SplitToningShadowHue"));
  set("color_grade", "shadows_sat", lrNum(text, "SplitToningShadowSaturation"));
  set("color_grade", "highlights_hue", lrNum(text, "SplitToningHighlightHue"));
  set("color_grade", "highlights_sat", lrNum(text, "SplitToningHighlightSaturation"));
  set("detail", "sharpen_amount", lrNum(text, "Sharpness"));
  set("detail", "noise_luma", lrNum(text, "LuminanceSmoothing"));
  set("detail", "noise_chroma", lrNum(text, "ColorNoiseReduction"));

  for (const [band, label] of HSL_BANDS) {
    set("hsl", `${band}.hue`, lrNum(text, `HueAdjustment${label}`));
    set("hsl", `${band}.sat`, lrNum(text, `SaturationAdjustment${label}`));
    set("hsl", `${band}.lum`, lrNum(text, `LuminanceAdjustment${label}`));
  }

  if (/ConvertToGrayscale\s*=\s*true/i.test(text)) {
    if (!modules.color_grade) modules.color_grade = {};
    modules.color_grade.global_chroma = -100;
    modules.color_grade.perceptual_sat = -100;
  }

  if (Object.keys(modules).length === 0) return null;
  return { modules };
}

function cleanLabel(s) {
  return s
    .replace(/\.(xmp|lrtemplate)$/i, "")
    .replace(STRIP_RE, " ")
    .replace(/^\d+\s*[-–—]\s*/, "")
    .replace(/[_]+/g, " ")
    .replace(/\s+/g, " ")
    .replace(/^[\s\-–—]+|[\s\-–—]+$/g, "")
    .trim();
}

function internalName(text) {
  const m = text.match(/internalName\s*=\s*"([^"]+)"/);
  return m ? cleanLabel(m[1]) : null;
}

function describePreset(filePath, sourceRoot, text) {
  const fromMeta = text ? internalName(text) : null;
  if (fromMeta) return fromMeta;

  const rel = path.relative(sourceRoot, filePath);
  const parts = rel.split(path.sep).filter(Boolean);
  const fileBase = cleanLabel(parts.pop() ?? path.basename(filePath));
  const folders = parts.map(cleanLabel).filter(Boolean);
  const folderHint = folders.length ? cleanLabel(folders[folders.length - 1]) : "";
  let label = fileBase;
  if (folderHint && !fileBase.toLowerCase().includes(folderHint.toLowerCase())) {
    label = `${folderHint} - ${fileBase}`;
  }
  return cleanLabel(label) || fileBase;
}

function safePresetId(label) {
  const base = label
    .normalize("NFKD")
    .replace(/[^\w\s-]/g, "")
    .replace(/\s+/g, "_")
    .replace(/_+/g, "_")
    .replace(/^_|_$/g, "")
    .slice(0, 80);
  return base || "preset";
}

function extractZip(zipPath, destDir) {
  fs.mkdirSync(destDir, { recursive: true });
  try {
    execFileSync("unzip", ["-q", "-o", zipPath, "-d", destDir], { stdio: "pipe" });
    return true;
  } catch {
    return false;
  }
}

function collectPresetFiles(sourceRoot, out = []) {
  let entries;
  try {
    entries = fs.readdirSync(sourceRoot, { withFileTypes: true });
  } catch (e) {
    console.warn(`  ⚠ skip (unreadable): ${sourceRoot} — ${e.message}`);
    return out;
  }
  for (const ent of entries) {
    const full = path.join(sourceRoot, ent.name);
    if (ent.isDirectory()) {
      if (SKIP_DIR_RE.test(ent.name)) continue;
      collectPresetFiles(full, out);
    } else if (/\.zip$/i.test(ent.name)) {
      const dest = path.join(TMP_DIR, path.basename(ent.name, ".zip"));
      if (extractZip(full, dest)) collectPresetFiles(dest, out);
    } else if (/\.(xmp|lrtemplate)$/i.test(ent.name)) {
      out.push(full);
    } else if (SKIP_FILE_RE.test(ent.name)) {
      /* ignore junk */
    }
  }
  return out;
}

function importSource(sourceRoot, usedNames) {
  const files = collectPresetFiles(sourceRoot);
  let imported = 0;
  let skipped = 0;

  for (const file of files) {
    let text;
    try {
      text = fs.readFileSync(file, "utf8");
    } catch {
      skipped++;
      continue;
    }

    const partial = file.toLowerCase().endsWith(".xmp")
      ? xmpToPartialDoc(text)
      : lrtemplateToPartialDoc(text);
    if (!partial) {
      skipped++;
      continue;
    }

    const label = describePreset(file, sourceRoot, text);
    let id = safePresetId(label);
    let n = 2;
    while (usedNames.has(id)) {
      id = `${safePresetId(label)}_${n++}`;
    }
    usedNames.add(id);

    fs.writeFileSync(
      path.join(OUT_DIR, `${id}.json`),
      JSON.stringify(partial, null, 2) + "\n",
    );
    imported++;
  }
  return { imported, skipped, scanned: files.length };
}

function clearBundledPresets() {
  for (const f of fs.readdirSync(OUT_DIR)) {
    if (f.endsWith(".json")) fs.unlinkSync(path.join(OUT_DIR, f));
  }
}

function main() {
  const sources = process.argv.slice(2).length
    ? process.argv.slice(2).map((p) => path.resolve(p))
    : DEFAULT_SOURCES.filter((p) => fs.existsSync(p));

  fs.mkdirSync(OUT_DIR, { recursive: true });
  fs.rmSync(TMP_DIR, { recursive: true, force: true });
  fs.mkdirSync(TMP_DIR, { recursive: true });
  clearBundledPresets();

  const usedNames = new Set();
  console.log(`MeraRAW preset import → ${OUT_DIR}\n`);
  let total = 0;
  let totalSkipped = 0;

  for (const src of sources) {
    if (!fs.existsSync(src)) {
      console.log(`— missing: ${src}`);
      continue;
    }
    console.log(`— ${src}`);
    const { imported, skipped, scanned } = importSource(src, usedNames);
    console.log(`  scanned ${scanned}, imported ${imported}, skipped ${skipped}`);
    total += imported;
    totalSkipped += skipped;
  }

  fs.rmSync(TMP_DIR, { recursive: true, force: true });

  console.log(`\nDone: ${total} presets in presets/bundled/`);
  if (totalSkipped > 0) {
    console.log(
      `Note: ${totalSkipped} files skipped (local-only gradients/masks, or no mappable global sliders).`,
    );
  }
  if (total === 0) process.exit(1);
}

main();
