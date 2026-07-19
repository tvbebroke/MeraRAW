import { atom } from "nanostores";
import { isZenMode } from "./editor";

export const isSettingsOpen = atom<boolean>(false);
export const isExportOpen = atom<boolean>(false);

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
