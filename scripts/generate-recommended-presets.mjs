#!/usr/bin/env node
/**
 * Generate MeraRAW's original "Recommended" develop presets.
 *
 * These are NOT Adobe Lightroom files and are not scraped from the internet.
 * They mirror common Lightroom-style starter categories (portrait, landscape,
 * B&W, film, etc.) as original PartialDoc / PresetFile JSON so the panel is
 * useful out of the box.
 *
 * Licensed third-party .xmp packs you already own still go through:
 *   npm run import-presets
 *
 * Usage: node scripts/generate-recommended-presets.mjs [--replace]
 */

import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const OUT = path.join(ROOT, "presets", "bundled");

/** @type {{ id: string, label: string, tags: string[], modules: Record<string, Record<string, number>> }[]} */
const PRESETS = [
  {
    id: "Recommended_Natural",
    label: "Recommended · Natural",
    tags: ["Natural", "Clean", "Recommended"],
    modules: {
      exposure: { stops: 0.1 },
      tone_curve: { contrast: 8, highlights: -8, shadows: 12, lights: 4, darks: -4 },
      color_grade: { perceptual_sat: 8, global_chroma: 2 },
      detail: { sharpen_amount: 35, noise_luma: 10, noise_chroma: 25 },
    },
  },
  {
    id: "Recommended_Bright_Airy",
    label: "Recommended · Bright & Airy",
    tags: ["Bright & Airy", "Pastel", "Recommended"],
    modules: {
      exposure: { stops: 0.45 },
      tone_curve: { contrast: -5, highlights: -25, shadows: 35, lights: 15, darks: 10 },
      color_grade: { perceptual_sat: -5, global_chroma: -8 },
      white_balance: { temp: 5600, tint: 4 },
    },
  },
  {
    id: "Recommended_Portrait_Soft",
    label: "Recommended · Portrait Soft",
    tags: ["Natural", "Dreamy", "Recommended"],
    modules: {
      exposure: { stops: 0.2 },
      tone_curve: { contrast: 5, highlights: -18, shadows: 22, lights: 8, darks: 5 },
      color_grade: { perceptual_sat: 5, global_chroma: -4 },
      hsl: {
        "orange.sat": 8,
        "orange.lum": 6,
        "red.sat": 4,
        "yellow.sat": -4,
      },
      detail: { sharpen_amount: 25, noise_luma: 15, noise_chroma: 30 },
    },
  },
  {
    id: "Recommended_Portrait_Warm",
    label: "Recommended · Portrait Warm",
    tags: ["Warm", "Natural", "Recommended"],
    modules: {
      exposure: { stops: 0.15 },
      white_balance: { temp: 5900, tint: 8 },
      tone_curve: { contrast: 10, highlights: -12, shadows: 18 },
      color_grade: {
        perceptual_sat: 10,
        midtones_hue: 35,
        midtones_sat: 8,
        highlights_hue: 40,
        highlights_sat: 6,
      },
      hsl: { "orange.sat": 12, "orange.lum": 4, "red.sat": 6 },
    },
  },
  {
    id: "Recommended_Landscape_Vivid",
    label: "Recommended · Landscape Vivid",
    tags: ["Vibrant", "Earthy", "Recommended"],
    modules: {
      exposure: { stops: 0.1 },
      tone_curve: { contrast: 22, highlights: -20, shadows: 15, lights: -5, darks: -10 },
      color_grade: { perceptual_sat: 28, global_chroma: 12 },
      hsl: {
        "blue.sat": 18,
        "aqua.sat": 12,
        "green.sat": 10,
        "green.hue": -8,
        "yellow.sat": 8,
      },
      detail: { sharpen_amount: 55, noise_luma: 5, noise_chroma: 20 },
    },
  },
  {
    id: "Recommended_Landscape_Moody",
    label: "Recommended · Landscape Moody",
    tags: ["Moody", "Dark", "Cool", "Recommended"],
    modules: {
      exposure: { stops: -0.25 },
      tone_curve: { contrast: 28, highlights: -30, shadows: -5, lights: -12, darks: -18 },
      color_grade: {
        perceptual_sat: -8,
        global_chroma: -5,
        shadows_hue: 210,
        shadows_sat: 14,
        midtones_hue: 200,
        midtones_sat: 6,
      },
      white_balance: { temp: 5000, tint: -2 },
    },
  },
  {
    id: "Recommended_Golden_Hour",
    label: "Recommended · Golden Hour",
    tags: ["Warm", "Dreamy", "Recommended"],
    modules: {
      exposure: { stops: 0.2 },
      white_balance: { temp: 6200, tint: 12 },
      tone_curve: { contrast: 12, highlights: -22, shadows: 20 },
      color_grade: {
        perceptual_sat: 12,
        highlights_hue: 45,
        highlights_sat: 18,
        midtones_hue: 35,
        midtones_sat: 10,
        shadows_hue: 25,
        shadows_sat: 6,
      },
    },
  },
  {
    id: "Recommended_Cool_Clean",
    label: "Recommended · Cool Clean",
    tags: ["Cool", "Clean", "Recommended"],
    modules: {
      exposure: { stops: 0.1 },
      white_balance: { temp: 4800, tint: -4 },
      tone_curve: { contrast: 12, highlights: -10, shadows: 15 },
      color_grade: {
        perceptual_sat: 5,
        shadows_hue: 220,
        shadows_sat: 8,
        midtones_hue: 205,
        midtones_sat: 4,
      },
    },
  },
  {
    id: "Recommended_High_Contrast",
    label: "Recommended · High Contrast",
    tags: ["High Contrast", "Editorial", "Recommended"],
    modules: {
      exposure: { stops: 0.05 },
      tone_curve: { contrast: 40, highlights: -25, shadows: -10, lights: -15, darks: -25 },
      color_grade: { perceptual_sat: 8, global_chroma: 5 },
      detail: { sharpen_amount: 60 },
    },
  },
  {
    id: "Recommended_Matte_Film",
    label: "Recommended · Matte Film",
    tags: ["Matte", "Film", "Muted", "Recommended"],
    modules: {
      exposure: { stops: 0.05 },
      tone_curve: { contrast: -8, highlights: -15, shadows: 25, lights: 8, darks: 18 },
      color_grade: { perceptual_sat: -12, global_chroma: -18 },
      effects: { grain_amount: 25 },
    },
  },
  {
    id: "Recommended_Teal_Orange",
    label: "Recommended · Teal & Orange",
    tags: ["Teal & Orange", "Cinematic", "Recommended"],
    modules: {
      exposure: { stops: 0.05 },
      tone_curve: { contrast: 18, highlights: -15, shadows: 10 },
      color_grade: {
        perceptual_sat: 10,
        shadows_hue: 195,
        shadows_sat: 22,
        midtones_hue: 180,
        midtones_sat: 8,
        highlights_hue: 40,
        highlights_sat: 20,
      },
      hsl: { "orange.sat": 15, "aqua.sat": 12, "blue.sat": 10 },
    },
  },
  {
    id: "Recommended_BW_Classic",
    label: "Recommended · B&W Classic",
    tags: ["Black & White", "Natural", "Recommended"],
    modules: {
      color_grade: { global_chroma: -100, perceptual_sat: 0 },
      tone_curve: { contrast: 20, highlights: -10, shadows: 15, lights: 5, darks: -8 },
      detail: { sharpen_amount: 45 },
    },
  },
  {
    id: "Recommended_BW_Punch",
    label: "Recommended · B&W Punch",
    tags: ["Black & White", "High Contrast", "Recommended"],
    modules: {
      color_grade: { global_chroma: -100 },
      tone_curve: { contrast: 45, highlights: -30, shadows: -15, lights: -10, darks: -30 },
      detail: { sharpen_amount: 70 },
    },
  },
  {
    id: "Recommended_Vintage_Fade",
    label: "Recommended · Vintage Fade",
    tags: ["Vintage", "Matte", "Warm", "Recommended"],
    modules: {
      exposure: { stops: 0.1 },
      white_balance: { temp: 5800, tint: 10 },
      tone_curve: { contrast: -12, highlights: -20, shadows: 30, darks: 22 },
      color_grade: {
        perceptual_sat: -15,
        global_chroma: -20,
        highlights_hue: 40,
        highlights_sat: 10,
        shadows_hue: 30,
        shadows_sat: 8,
      },
      effects: { grain_amount: 35, vignette_amount: -15 },
    },
  },
  {
    id: "Recommended_Night_City",
    label: "Recommended · Night City",
    tags: ["Dark", "Cool", "Cinematic", "Recommended"],
    modules: {
      exposure: { stops: -0.15 },
      white_balance: { temp: 4200, tint: 5 },
      tone_curve: { contrast: 25, highlights: -35, shadows: 20, lights: -20 },
      color_grade: {
        perceptual_sat: 15,
        shadows_hue: 230,
        shadows_sat: 18,
        midtones_hue: 210,
        midtones_sat: 10,
        highlights_hue: 50,
        highlights_sat: 12,
      },
      detail: { noise_luma: 35, noise_chroma: 40, sharpen_amount: 40 },
    },
  },
  {
    id: "Recommended_Food_Punch",
    label: "Recommended · Food Punch",
    tags: ["Vibrant", "Warm", "Recommended"],
    modules: {
      exposure: { stops: 0.15 },
      white_balance: { temp: 5600, tint: 6 },
      tone_curve: { contrast: 18, highlights: -12, shadows: 12 },
      color_grade: { perceptual_sat: 35, global_chroma: 15 },
      hsl: {
        "orange.sat": 20,
        "red.sat": 15,
        "yellow.sat": 18,
        "green.sat": 8,
      },
      detail: { sharpen_amount: 50 },
    },
  },
  {
    id: "Recommended_Flat_Neutral",
    label: "Recommended · Flat Neutral (charts)",
    tags: ["Clean", "Natural", "Recommended"],
    modules: {
      exposure: { stops: 0 },
      tone_curve: { contrast: -20, highlights: 10, shadows: 15, lights: 5, darks: 10 },
      color_grade: { perceptual_sat: 0, global_chroma: 0 },
      white_balance: { temp: 5500, tint: 0 },
    },
  },
  {
    id: "Recommended_Shadow_Lift",
    label: "Recommended · Shadow Lift",
    tags: ["Bright & Airy", "Clean", "Recommended"],
    modules: {
      exposure: { stops: 0.2 },
      tone_curve: { contrast: 5, highlights: -5, shadows: 45, darks: 20 },
      color_grade: { perceptual_sat: 5 },
    },
  },
  {
    id: "Recommended_Soft_Print",
    label: "Recommended · Soft Print",
    tags: ["Muted", "Dreamy", "Recommended"],
    modules: {
      exposure: { stops: 0.1 },
      tone_curve: { contrast: -5, highlights: -20, shadows: 25, lights: 10 },
      color_grade: { perceptual_sat: -8, global_chroma: -10 },
      detail: { sharpen_amount: 20 },
    },
  },
  {
    id: "Recommended_Cinematic_Cool",
    label: "Recommended · Cinematic Cool",
    tags: ["Cinematic", "Cool", "Teal & Orange", "Recommended"],
    modules: {
      exposure: { stops: -0.1 },
      tone_curve: { contrast: 22, highlights: -28, shadows: 8, lights: -15, darks: -12 },
      color_grade: {
        perceptual_sat: -5,
        shadows_hue: 200,
        shadows_sat: 25,
        midtones_hue: 190,
        midtones_sat: 10,
        highlights_hue: 35,
        highlights_sat: 12,
        highlights_lum: -5,
      },
      effects: { vignette_amount: -20 },
    },
  },
];

function main() {
  const replace = process.argv.includes("--replace");
  fs.mkdirSync(OUT, { recursive: true });

  if (replace) {
    for (const f of fs.readdirSync(OUT)) {
      if (f.startsWith("Recommended_") && f.endsWith(".json")) {
        fs.unlinkSync(path.join(OUT, f));
      }
    }
  }

  let n = 0;
  for (const p of PRESETS) {
    const file = {
      label: p.label,
      tags: p.tags,
      modules: p.modules,
    };
    const dest = path.join(OUT, `${p.id}.json`);
    fs.writeFileSync(dest, JSON.stringify(file, null, 2) + "\n");
    n++;
  }
  console.log(`Wrote ${n} recommended presets → ${OUT}`);
  console.log("Tip: npm run tag-presets  # refresh tags on any imported packs");
}

main();
