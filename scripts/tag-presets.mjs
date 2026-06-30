#!/usr/bin/env node
/**
 * Assign category tags + display labels to preset JSON files.
 *
 * Usage:
 *   npm run tag-presets              # bundled + user presets
 *   node scripts/tag-presets.mjs <dir...>
 */

import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const BUNDLED = path.join(ROOT, "presets", "bundled");

export const ALL_TAGS = [
  "Natural",
  "Bright & Airy",
  "Moody",
  "Cinematic",
  "Film",
  "Vintage",
  "Matte",
  "Warm",
  "Cool",
  "Vibrant",
  "Muted",
  "Pastel",
  "High Contrast",
  "Black & White",
  "Dark",
  "Dreamy",
  "Editorial",
  "Clean",
  "Earthy",
  "Teal & Orange",
];

function userPresetsDir() {
  if (process.platform === "darwin") {
    return path.join(os.homedir(), "Library", "Application Support", "MeraRAW", "presets");
  }
  if (process.platform === "win32") {
    return path.join(process.env.APPDATA ?? os.homedir(), "MeraRAW", "presets");
  }
  return path.join(os.homedir(), ".local", "share", "MeraRAW", "presets");
}

function humanizeId(id) {
  return id
    .replace(/_/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

function presetLabel(id, raw) {
  if (raw.label) return raw.label;
  if (/[\s-']/.test(id)) return id;
  return humanizeId(id);
}

function num(v) {
  return typeof v === "number" && Number.isFinite(v) ? v : null;
}

function tagFromName(id) {
  const n = id.toLowerCase().replace(/_/g, " ");
  const tags = new Set();
  const has = (...words) => words.some((w) => n.includes(w));

  if (has("bw", "b&w", "black white", "monochrome", "mono ", " mono", "noir", "tri-x", "hp5"))
    tags.add("Black & White");
  if (has("cinematic", "cine negative", "cine ")) tags.add("Cinematic");
  if (has("vintage", "retro", "polaroid", "sepia", "antique", "expired film"))
    tags.add("Vintage");
  if (has("matte")) tags.add("Matte");
  if (
    has(
      "film",
      "fuji",
      "kodak",
      "kodachrome",
      "portra",
      "velvia",
      "polaroid",
      "cross process",
      "bleach bypass",
      "cine negative",
      "expired",
      "slide ",
    )
  )
    tags.add("Film");
  if (has("portrait", "magazine skin", "wedding")) tags.add("Editorial");
  if (has("landscape", "mountain", "nature", "seascape", "tropical", "lush green"))
    tags.add("Natural");
  if (has("pastel", "soft", "lifestyle soft", "light & airy", "high key"))
    tags.add("Pastel");
  if (has("warm", "gold", "golden", "sunset", "autumn", "desert", "portra soft"))
    tags.add("Warm");
  if (has("cool", "blue hour", "cold ", "cyan", "ice", "seascape blue", "aqua"))
    tags.add("Cool");
  if (has("teal and orange", "teal orange", "teal and red")) tags.add("Teal & Orange");
  if (has("moody", "dramatic", "dark &", "low key", "noir", "forest")) tags.add("Moody");
  if (has("dark") && !has("light")) tags.add("Dark");
  if (has("dream", "fog", "foggy", "mist", "haze", "soften")) tags.add("Dreamy");
  if (has("vibrant", "pop", "punch", "vivid", "fluo", "neon", "velvia", "travel vibrant"))
    tags.add("Vibrant");
  if (has("muted", "fade", "faded", "desaturat", "washed", "urban desaturated", "low sat"))
    tags.add("Muted");
  if (has("bright", "airy", "light & airy", "high key", "dehaze clear", "crisp"))
    tags.add("Bright & Airy");
  if (has("editorial", "clean editorial", "magazine", "commercial", "neutral pro"))
    tags.add("Editorial");
  if (has("clean", "neutral", "auto tone")) tags.add("Clean");
  if (has("desert", "earth tone", "earthy", "earth tones", "rusty", "utah red"))
    tags.add("Earthy");
  if (has("high contrast", "punch up", "street contrast", "mono high contrast"))
    tags.add("High Contrast");
  if (has("architecture")) tags.add("Editorial");
  if (has("street")) tags.add("Cinematic");
  if (has("night", "neon")) tags.add("Dark");
  if (has("wedding light", "wedding pastel", "wedding timeless")) tags.add("Pastel");
  if (has("wedding warm")) tags.add("Warm");
  if (has("basic -")) tags.add("Natural");

  return tags;
}

function tagFromModules(modules) {
  const tags = new Set();
  const exp = num(modules.exposure?.stops) ?? 0;
  const contrast = num(modules.tone_curve?.contrast) ?? 0;
  const highlights = num(modules.tone_curve?.highlights) ?? 0;
  const shadows = num(modules.tone_curve?.shadows) ?? 0;
  const sat = num(modules.color_grade?.global_chroma) ?? 0;
  const vib = num(modules.color_grade?.perceptual_sat) ?? 0;
  const shHue = num(modules.color_grade?.shadows_hue);
  const shSat = num(modules.color_grade?.shadows_sat) ?? 0;
  const hiHue = num(modules.color_grade?.highlights_hue);
  const hiSat = num(modules.color_grade?.highlights_sat) ?? 0;
  const temp = num(modules.white_balance?.temp);

  if (sat <= -90 && vib <= -90) tags.add("Black & White");
  if (exp >= 0.28) tags.add("Bright & Airy");
  if (exp <= -0.35) tags.add("Dark");
  if (contrast >= 28) tags.add("High Contrast");
  if (Math.abs(contrast) <= 12 && sat <= -15 && vib <= 15) tags.add("Matte");
  if (sat <= -18 && vib <= 12 && sat > -90) tags.add("Muted");
  if (vib >= 30 || sat >= 22) tags.add("Vibrant");
  if (temp !== null && temp >= 5800) tags.add("Warm");
  if (temp !== null && temp <= 4800) tags.add("Cool");
  if (
    shHue !== null &&
    shHue >= 165 &&
    shHue <= 245 &&
    shSat >= 8 &&
    hiHue !== null &&
    hiHue >= 20 &&
    hiHue <= 75 &&
    hiSat >= 8
  ) {
    tags.add("Teal & Orange");
  }
  if (Math.abs(contrast) < 18 && exp > -0.25 && exp < 0.35 && sat > -45 && sat < 15)
    tags.add("Pastel");
  if (contrast < 18 && highlights > 10 && sat < 5) tags.add("Dreamy");
  if (shadows > 15 && exp < 0.1 && contrast > 10) tags.add("Moody");

  const paramCount = Object.values(modules).reduce(
    (acc, mod) => acc + Object.keys(mod).length,
    0,
  );
  if (paramCount <= 4 && Math.abs(exp) < 0.2 && Math.abs(contrast) < 12) {
    tags.add("Clean");
    tags.add("Natural");
  }

  if (hiHue !== null && hiHue >= 25 && hiHue <= 55 && hiSat >= 10 && !tags.has("Teal & Orange"))
    tags.add("Warm");
  if (shHue !== null && shHue >= 200 && shHue <= 250 && shSat >= 10) tags.add("Cool");

  return tags;
}

function inferTags(id, modules) {
  const tags = new Set([...tagFromName(id), ...tagFromModules(modules)]);

  const n = id.toLowerCase();
  if (/^portrait(_|$|-)/.test(n) && tags.size <= 2) {
    tags.add("Editorial");
    tags.add("Natural");
  }
  if (/^landscape(_|$|-)/.test(n) && tags.size <= 2) {
    tags.add("Natural");
    tags.add("Vibrant");
  }
  if (/^cinematic(_|$|-)/.test(n)) tags.add("Cinematic");
  if (/^wedding/.test(n) && !tags.has("Warm") && !tags.has("Pastel")) tags.add("Editorial");
  if (tags.size === 0) tags.add("Natural");

  return ALL_TAGS.filter((t) => tags.has(t));
}

function tagDirectory(dir) {
  if (!fs.existsSync(dir)) {
    console.log(`Skip (missing): ${dir}`);
    return { updated: 0, tagCounts: Object.fromEntries(ALL_TAGS.map((t) => [t, 0])) };
  }

  const files = fs.readdirSync(dir).filter((f) => f.endsWith(".json"));
  const tagCounts = Object.fromEntries(ALL_TAGS.map((t) => [t, 0]));

  for (const file of files) {
    const id = file.slice(0, -5);
    const filePath = path.join(dir, file);
    const raw = JSON.parse(fs.readFileSync(filePath, "utf8"));
    const modules = raw.modules ?? raw;
    const tags = inferTags(id, modules);
    const label = presetLabel(id, raw);
    fs.writeFileSync(filePath, JSON.stringify({ label, tags, modules }, null, 2) + "\n");
    for (const t of tags) tagCounts[t] = (tagCounts[t] ?? 0) + 1;
  }

  return { updated: files.length, tagCounts };
}

function main() {
  const dirs =
    process.argv.length > 2
      ? process.argv.slice(2).map((d) => path.resolve(d))
      : [BUNDLED, userPresetsDir()];

  let total = 0;
  const combined = Object.fromEntries(ALL_TAGS.map((t) => [t, 0]));

  for (const dir of dirs) {
    const { updated, tagCounts } = tagDirectory(dir);
    total += updated;
    console.log(`\n${dir}`);
    console.log(`  Tagged ${updated} presets`);
    for (const t of ALL_TAGS) {
      if (tagCounts[t]) console.log(`    ${t}: ${tagCounts[t]}`);
      combined[t] += tagCounts[t] ?? 0;
    }
  }

  console.log(`\nTotal: ${total} presets tagged`);
}

main();
