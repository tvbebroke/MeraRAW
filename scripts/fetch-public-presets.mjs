#!/usr/bin/env node
/**
 * Fetch curated free/open photo presets into preset-sources/public/.
 *
 * Only downloads packs listed in presets/public-catalog.json (SPDX licenses).
 * Does NOT scrape Adobe Marketplace, cracked packs, or arbitrary web hosts.
 *
 * Usage:
 *   node scripts/fetch-public-presets.mjs
 *   node scripts/fetch-public-presets.mjs --id peva3-film-xmp
 *   node scripts/fetch-public-presets.mjs --include-darktable
 *   node scripts/fetch-public-presets.mjs --list
 *   node scripts/fetch-public-presets.mjs --discover   # print GitHub MIT hits for manual review
 */

import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";

const ROOT = path.resolve(import.meta.dirname, "..");
const CATALOG_PATH = path.join(ROOT, "presets", "public-catalog.json");
const OUT_ROOT = path.join(ROOT, "preset-sources", "public");
const ATTR_PATH = path.join(OUT_ROOT, "ATTRIBUTION.md");

function parseArgs(argv) {
  const out = {
    ids: [],
    list: false,
    discover: false,
    includeDarktable: false,
    importAfter: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--list") out.list = true;
    else if (a === "--discover") out.discover = true;
    else if (a === "--include-darktable") out.includeDarktable = true;
    else if (a === "--import") out.importAfter = true;
    else if (a === "--id") out.ids.push(argv[++i]);
    else if (a.startsWith("--id=")) out.ids.push(a.slice(5));
  }
  return out;
}

function loadCatalog() {
  return JSON.parse(fs.readFileSync(CATALOG_PATH, "utf8"));
}

function matchGlob(rel, glob) {
  // Minimal **/*.ext / *.ext matcher
  const g = glob.replace(/\\/g, "/");
  const r = rel.replace(/\\/g, "/");
  if (g.startsWith("**/")) {
    const suf = g.slice(3);
    if (suf.startsWith("*.")) {
      return r.toLowerCase().endsWith(suf.slice(1).toLowerCase());
    }
    return r.endsWith(suf) || r.includes("/" + suf);
  }
  if (g.startsWith("*.")) return r.toLowerCase().endsWith(g.slice(1).toLowerCase());
  return r === g || r.endsWith("/" + g);
}

function walkFiles(dir, acc = []) {
  if (!fs.existsSync(dir)) return acc;
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, ent.name);
    if (ent.isDirectory()) {
      if (ent.name === ".git") continue;
      walkFiles(p, acc);
    } else {
      acc.push(p);
    }
  }
  return acc;
}

function shallowClone(repo, ref, dest) {
  fs.rmSync(dest, { recursive: true, force: true });
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  console.log(`  git clone --depth 1 ${repo}@${ref}`);
  execFileSync(
    "git",
    ["clone", "--depth", "1", "--branch", ref, `https://github.com/${repo}.git`, dest],
    { stdio: "inherit" },
  );
  fs.rmSync(path.join(dest, ".git"), { recursive: true, force: true });
}

function filterCopy(srcRoot, destRoot, globs) {
  fs.rmSync(destRoot, { recursive: true, force: true });
  fs.mkdirSync(destRoot, { recursive: true });
  const files = walkFiles(srcRoot);
  let n = 0;
  for (const abs of files) {
    const rel = path.relative(srcRoot, abs);
    if (!globs.some((g) => matchGlob(rel, g))) continue;
    const out = path.join(destRoot, rel);
    fs.mkdirSync(path.dirname(out), { recursive: true });
    fs.copyFileSync(abs, out);
    n++;
  }
  return n;
}

function writeAttribution(sources) {
  const lines = [
    "# Public preset attribution",
    "",
    "Fetched by `npm run fetch-public-presets` from `presets/public-catalog.json`.",
    "",
  ];
  for (const s of sources) {
    lines.push(`## ${s.name}`);
    lines.push(`- License: ${s.license}`);
    lines.push(`- Source: ${s.homepage}`);
    if (s.notes) lines.push(`- Notes: ${s.notes}`);
    lines.push("");
  }
  fs.mkdirSync(OUT_ROOT, { recursive: true });
  fs.writeFileSync(ATTR_PATH, lines.join("\n"));
}

async function discoverGithub() {
  const q =
    "https://api.github.com/search/repositories?q=lightroom+presets+license:mit&sort=stars&per_page=30";
  const res = await fetch(q, {
    headers: { "User-Agent": "MeraRAW-public-preset-discover", Accept: "application/vnd.github+json" },
  });
  if (!res.ok) throw new Error(`GitHub search failed: ${res.status}`);
  const data = await res.json();
  console.log("MIT GitHub hits (review before adding to public-catalog.json):\n");
  for (const item of data.items || []) {
    console.log(
      `- ${item.full_name} ★${item.stargazers_count} — ${item.html_url}\n  ${item.description || ""}`,
    );
  }
  console.log(
    "\nOnly add packs whose authors clearly license redistribution. Skip converters, scrapes, and commercial dumps.",
  );
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const catalog = loadCatalog();

  if (args.list) {
    for (const s of catalog.sources) {
      const on = s.enabled === false ? "off" : "on";
      console.log(`${s.id.padEnd(28)} ${on.padEnd(4)} ${s.license.padEnd(12)} ${s.format}  ${s.name}`);
    }
    return;
  }

  if (args.discover) {
    await discoverGithub();
    return;
  }

  let sources = catalog.sources.filter((s) => {
    if (args.ids.length) return args.ids.includes(s.id);
    if (s.enabled === false && !(args.includeDarktable && s.format === "dtstyle")) return false;
    if (s.format === "dtstyle" && !args.includeDarktable) return false;
    return true;
  });

  if (!sources.length) {
    console.error("No sources selected. Use --list or --id <id>.");
    process.exit(1);
  }

  fs.mkdirSync(OUT_ROOT, { recursive: true });
  const tmp = path.join(ROOT, ".preset-fetch-tmp");
  fs.rmSync(tmp, { recursive: true, force: true });
  fs.mkdirSync(tmp, { recursive: true });

  const fetched = [];
  for (const s of sources) {
    console.log(`\n→ ${s.id} (${s.license})`);
    if (s.kind !== "github") {
      console.warn("  skip: unsupported kind", s.kind);
      continue;
    }
    const cloneDir = path.join(tmp, s.id);
    try {
      shallowClone(s.repo, s.ref || "main", cloneDir);
    } catch (e) {
      // retry default branch if ref missing
      console.warn(`  clone @${s.ref} failed, trying without --branch…`);
      fs.rmSync(cloneDir, { recursive: true, force: true });
      execFileSync(
        "git",
        ["clone", "--depth", "1", `https://github.com/${s.repo}.git`, cloneDir],
        { stdio: "inherit" },
      );
      fs.rmSync(path.join(cloneDir, ".git"), { recursive: true, force: true });
    }
    const dest = path.join(OUT_ROOT, s.id);
    const n = filterCopy(cloneDir, dest, s.include_globs || ["**/*.xmp"]);
    console.log(`  kept ${n} files → ${dest}`);
    if (n > 0) fetched.push(s);
  }

  fs.rmSync(tmp, { recursive: true, force: true });
  writeAttribution(fetched);
  console.log(`\nDone. Sources under ${OUT_ROOT}`);
  console.log("Next: npm run import-presets && npm run tag-presets");

  if (args.importAfter) {
    execFileSync("node", [path.join(ROOT, "scripts/import-lightroom-presets.mjs"), OUT_ROOT], {
      stdio: "inherit",
    });
    execFileSync("node", [path.join(ROOT, "scripts/tag-presets.mjs"), path.join(ROOT, "presets/bundled")], {
      stdio: "inherit",
    });
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
