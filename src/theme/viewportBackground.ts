// App chrome background — solid colors, liquid glass, or full transparency
// so the desktop shows through. Photo + controls stay opaque.

import { Effect, EffectState, getCurrentWindow } from "@tauri-apps/api/window";

export type ViewportBg =
  | "white"
  | "light-gray"
  | "black"
  | "jet-black"
  | "liquid-glass"
  | "transparent";

/** Solid chrome colors available in both Modern and Faithful. */
export const SOLID_VIEWPORT_BGS = [
  "white",
  "light-gray",
  "black",
  "jet-black",
] as const satisfies readonly ViewportBg[];

export type SolidViewportBg = (typeof SOLID_VIEWPORT_BGS)[number];

export function isSolidViewportBg(id: ViewportBg): id is SolidViewportBg {
  return (SOLID_VIEWPORT_BGS as readonly string[]).includes(id);
}

export const VIEWPORT_BG_OPTIONS: {
  id: ViewportBg;
  label: string;
  swatch: string;
  hint: string;
}[] = [
  { id: "white", label: "White", swatch: "#ffffff", hint: "Bright chrome" },
  { id: "light-gray", label: "Light gray", swatch: "#c8c8c8", hint: "Soft neutral chrome" },
  { id: "black", label: "Black", swatch: "#1a1a1a", hint: "Classic dark chrome" },
  { id: "jet-black", label: "Jet black", swatch: "#000000", hint: "True black chrome" },
  {
    id: "liquid-glass",
    label: "Liquid glass",
    swatch: "linear-gradient(135deg,#8ec5ff66,#ffffff22)",
    hint: "Frosted glass — desktop shows through",
  },
  {
    id: "transparent",
    label: "Transparent",
    swatch: "repeating-conic-gradient(#666 0% 25%, #333 0% 50%) 0 0/12px 12px",
    hint: "Clear chrome — only photo & controls stay solid",
  },
];

/** Faithful shell: solid colors only (no glass / video). */
export const FAITHFUL_VIEWPORT_BG_OPTIONS = VIEWPORT_BG_OPTIONS.filter((o) =>
  isSolidViewportBg(o.id),
);

const STORAGE_KEY = "meraraw.viewportBg";
const DEFAULT_BG: ViewportBg = "black";

const SOLID_WINDOW_COLORS: Record<
  Exclude<ViewportBg, "liquid-glass" | "transparent">,
  [number, number, number, number]
> = {
  white: [240, 240, 240, 255],
  "light-gray": [200, 200, 200, 255],
  black: [39, 39, 39, 255],
  "jet-black": [0, 0, 0, 255],
};

export function readStoredViewportBg(): ViewportBg {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (VIEWPORT_BG_OPTIONS.some((o) => o.id === v)) return v as ViewportBg;
  } catch {
    /* ignore */
  }
  return DEFAULT_BG;
}

export function persistViewportBg(id: ViewportBg) {
  try {
    localStorage.setItem(STORAGE_KEY, id);
  } catch {
    /* ignore */
  }
}

/** Apply CSS data attribute on the root + app shell. */
export function applyViewportBgAttr(id: ViewportBg) {
  document.documentElement.dataset.viewportBg = id;
  const app = document.querySelector(".app");
  if (app instanceof HTMLElement) app.dataset.viewportBg = id;
}

/** Sync native window color / vibrancy with the selected chrome style. */
export async function syncWindowBackdrop(id: ViewportBg): Promise<void> {
  try {
    const win = getCurrentWindow();
    if (id === "liquid-glass") {
      await win.setBackgroundColor([0, 0, 0, 0]);
      await win.setEffects({
        effects: [Effect.HudWindow, Effect.UnderWindowBackground, Effect.Sidebar],
        state: EffectState.Active,
        radius: 18,
      });
      return;
    }
    if (id === "transparent") {
      await win.clearEffects();
      await win.setBackgroundColor([0, 0, 0, 0]);
      return;
    }
    await win.clearEffects();
    await win.setBackgroundColor(SOLID_WINDOW_COLORS[id]);
  } catch {
    // Browser preview / unsupported platform — CSS-only fallback is fine.
  }
}
