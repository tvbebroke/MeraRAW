import { useEffect } from "react";
import { useUiStore } from "../state/uiStore";
import { registerAllCommandHandlers } from "./commands";
import { getAppKeyboardContext } from "./context";
import { dispatchKeyEvent, initKeyboardDispatcher, setScopeProvider } from "./dispatcher";
import { allKeyBindings, buildBindingIndex } from "./loadKeymap";
import type { KeymapScope } from "./types";

let initialized = false;

function ensureInit() {
  if (initialized) return;
  initialized = true;
  registerAllCommandHandlers();
  initKeyboardDispatcher(buildBindingIndex(allKeyBindings()));
  setScopeProvider(() => {
    const ui = useUiStore.getState();
    const app = getAppKeyboardContext();
    const scopes: KeymapScope[] = [];

    if (app?.getMode() === "develop") {
      if (ui.tool === "brush") scopes.push("tool:brush");
      if (ui.cropActive || ui.rightRailTab === "crop") scopes.push("tool:crop");
      if (ui.rightRailTab === "remove") scopes.push("tool:spot");
    }

    if (app?.getMode() === "library") scopes.push("module:library");
    if (app?.getMode() === "develop") scopes.push("module:develop");
    scopes.push("global");
    return scopes;
  });
}

/** Mount once at app root — single global keydown dispatcher. */
export function useKeyboardShortcuts(enabled = true) {
  useEffect(() => {
    if (!enabled) return;
    ensureInit();

    function onKeyDown(e: KeyboardEvent) {
      const ui = useUiStore.getState();
      if (ui.cropActive && getAppKeyboardContext()?.getMode() === "develop") {
        if (e.key === "Enter") {
          ui.exitCropTool(true);
          e.preventDefault();
          return;
        }
        if (e.key === "Escape") {
          ui.exitCropTool(false);
          e.preventDefault();
          return;
        }
        if (e.key === "h" || e.key === "H") {
          ui.toggleCropOverlayVisible();
          e.preventDefault();
          return;
        }
      }
      void dispatchKeyEvent(e);
    }

    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [enabled]);
}

export { allKeyBindings, buildBindingIndex };
