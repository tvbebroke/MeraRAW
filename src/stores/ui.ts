import { atom } from "nanostores";
import { isZenMode } from "./editor";

export const isSettingsOpen = atom<boolean>(false);
export const isExportOpen = atom<boolean>(false);
export const isBugReportOpen = atom<boolean>(false);
export const isShortcutsOpen = atom<boolean>(false);

const CLASSIC_LOOK_KEY = "classic-look";

function readStoredClassicLook(): boolean {
  try {
    return localStorage.getItem(CLASSIC_LOOK_KEY) === "true";
  } catch {
    return false;
  }
}

function persistClassicLook(val: boolean): void {
  try {
    localStorage.setItem(CLASSIC_LOOK_KEY, String(val));
  } catch {
    /* ignore */
  }
}

export const classicLook = atom<boolean>(readStoredClassicLook());

const THEME_PREFERENCE_KEY = "theme-preference";

export type ThemePreference = "dark" | "light" | "system";

function readStoredThemePreference(): ThemePreference {
  try {
    const v = localStorage.getItem(THEME_PREFERENCE_KEY);
    if (v === "dark" || v === "light" || v === "system") return v;
  } catch {
    /* ignore */
  }
  return "dark";
}

function systemPrefersLight(): boolean {
  try {
    return window.matchMedia("(prefers-color-scheme: light)").matches;
  } catch {
    return false;
  }
}

/** What the user picked. Dark is the default — see `lightMode` for why. */
export const themePreference = atom<ThemePreference>(readStoredThemePreference());

/**
 * Resolved light/dark, after folding in the OS setting.
 *
 * Dark by default and deliberately so: a light surround biases how you read
 * exposure and white balance, which is why serious raw editors ship dark.
 * Even in light mode the image canvas keeps its dark surround (see
 * `.light-look` in app.css) so colour judgement stays trustworthy.
 */
export const lightMode = atom<boolean>(
  readStoredThemePreference() === "light" ||
    (readStoredThemePreference() === "system" && systemPrefersLight()),
);

function resolveLightMode(pref: ThemePreference): boolean {
  return pref === "light" || (pref === "system" && systemPrefersLight());
}

export const setThemePreference = (pref: ThemePreference) => {
  themePreference.set(pref);
  lightMode.set(resolveLightMode(pref));
  try {
    localStorage.setItem(THEME_PREFERENCE_KEY, pref);
  } catch {
    /* ignore */
  }
};

// Follow the OS in real time, but only while "system" is selected.
try {
  window
    .matchMedia("(prefers-color-scheme: light)")
    .addEventListener("change", () => {
      if (themePreference.get() === "system") {
        lightMode.set(systemPrefersLight());
      }
    });
} catch {
  /* matchMedia unavailable — stay on the stored preference */
}

export const themeTransitionActive = atom<boolean>(false);
export const themeFlashActive = atom<boolean>(false);
export const themeTransitionTarget = atom<"classic" | "standard">("classic");

export const setClassicLook = (val: boolean) => {
  classicLook.set(val);
  persistClassicLook(val);
  // Classic look has no Zen mode, so leaving it on would hide the whole rail.
  if (val) isZenMode.set(false);
};

const FLASH_AT_MS = 900;
const FLASH_HOLD_MS = 100;
const TRANSITION_MS = 2000;

let flashTimer: ReturnType<typeof setTimeout> | null = null;
let flashEndTimer: ReturnType<typeof setTimeout> | null = null;
let transitionEndTimer: ReturnType<typeof setTimeout> | null = null;

/** Swap the shell mid-animation so the change is masked by the shutter flash. */
export const triggerThemeTransition = (val: boolean) => {
  if (flashTimer) clearTimeout(flashTimer);
  if (flashEndTimer) clearTimeout(flashEndTimer);
  if (transitionEndTimer) clearTimeout(transitionEndTimer);

  themeFlashActive.set(false);
  themeTransitionTarget.set(val ? "classic" : "standard");
  themeTransitionActive.set(true);

  flashTimer = setTimeout(() => {
    themeFlashActive.set(true);
    setClassicLook(val);

    flashEndTimer = setTimeout(() => {
      themeFlashActive.set(false);
      flashEndTimer = null;
    }, FLASH_HOLD_MS);
    flashTimer = null;
  }, FLASH_AT_MS);

  transitionEndTimer = setTimeout(() => {
    themeTransitionActive.set(false);
    transitionEndTimer = null;
  }, TRANSITION_MS);
};
