import { useEffect } from "react";
import { useUiStore } from "../state/uiStore";
import { registerAllCommandHandlers } from "./commands";
import { getAppKeyboardContext } from "./context";
import {
  dispatchKeyEvent,
  initKeyboardDispatcher,
  invokeCommand,
  setScopeProvider,
} from "./dispatcher";
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

function isMacPlatform(): boolean {
  return typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
}

/** Mount once at app root — single global keydown dispatcher. */
export function useKeyboardShortcuts(enabled = true) {
  useEffect(() => {
    if (!enabled) return;
    ensureInit();

    function onKeyDown(e: KeyboardEvent) {
      const ui = useUiStore.getState();
      const tag = (e.target as HTMLElement | null)?.tagName;
      const typing =
        tag === "INPUT" ||
        tag === "TEXTAREA" ||
        (e.target as HTMLElement | null)?.isContentEditable;
      if (ui.cropActive && getAppKeyboardContext()?.getMode() === "develop" && !typing) {
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
        // Spec: L toggles lights-out while cropping
        if ((e.key === "l" || e.key === "L") && !e.metaKey && !e.ctrlKey && !e.altKey) {
          ui.toggleCropLightsOut();
          e.preventDefault();
          return;
        }
        // Arrow nudge: 1px (Shift = 10px); Ctrl/Cmd+arrows nudge angle 0.1°
        if (
          e.key === "ArrowLeft" ||
          e.key === "ArrowRight" ||
          e.key === "ArrowUp" ||
          e.key === "ArrowDown"
        ) {
          const id =
            e.metaKey || e.ctrlKey
              ? "develop-module---tools:nudge-crop-angle"
              : "develop-module---tools:nudge-crop";
          const chord = [
            e.metaKey || e.ctrlKey ? (isMacPlatform() ? "Cmd" : "Ctrl") : null,
            e.shiftKey ? "Shift" : null,
            e.key.replace("Arrow", ""),
          ]
            .filter(Boolean)
            .join("+");
          void invokeCommand(id, chord);
          e.preventDefault();
          return;
        }
        if (e.key === "0" && !e.metaKey && !e.ctrlKey && !e.altKey) {
          void invokeCommand("develop-module---tools:reset-crop-angle", "0");
          e.preventDefault();
          return;
        }
        // Ctrl/Cmd+Alt+C/V — copy/paste crop
        if ((e.metaKey || e.ctrlKey) && e.altKey && (e.key === "c" || e.key === "C")) {
          void invokeCommand("develop-module---tools:copy-crop", "Cmd+Alt+C");
          e.preventDefault();
          return;
        }
        if ((e.metaKey || e.ctrlKey) && e.altKey && (e.key === "v" || e.key === "V")) {
          void invokeCommand("develop-module---tools:paste-crop", "Cmd+Alt+V");
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
