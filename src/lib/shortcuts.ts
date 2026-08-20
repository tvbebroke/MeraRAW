import { push, router } from "svelte-spa-router";
import { leftRailCollapsed, isZenMode, imageBrowserCollapsed, photoDetailsCollapsed, commandPaletteOpen } from "../stores/editor";
import { isSettingsOpen, isExportOpen, isBugReportOpen, classicLook, isShortcutsOpen } from "../stores/ui";
import { applyEditFocus, showAiPanel } from "./editor/focus";
import { activePhoto, libraryItems, openPhoto, gridKey } from "../stores/browse";
import { imageMeta } from "../stores/app";
import { editorRoute, isEditorRoute, isLibraryRoute, libraryRoute, workspace } from "../stores/workspace";
import { copyGrade, pasteGrade } from "./grade";

// ─── Types ──────────────────────────────────────────────────────────────────

export interface ShortcutDef {
  /** Display label for the shortcut hint (e.g. "⌘E") */
  label: string;
  /** Human-readable description */
  description: string;
  /** The handler function — return `true` to signal the event was consumed */
  handler: () => boolean | void;
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/** Returns true when the active element is an editable field */
function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.isContentEditable ||
    ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)
  );
}

/** Safely push a route with an optional View Transition */
function safePush(path: string) {
  if (typeof document !== "undefined" && (document as any).startViewTransition) {
    (document as any).startViewTransition(() => push(path));
  } else {
    push(path);
  }
}

function isOnRoute(path: string): boolean {
  return router.location === path;
}

function isEditVideo(): boolean {
  return isEditorRoute() && imageMeta.get()?.kind === "video";
}

function prevPhoto() {
  const list = libraryItems.get();
  if (!list.length) return;
  const current = activePhoto.get();
  const idx = current ? list.findIndex((p) => gridKey(p) === gridKey(current)) : -1;
  const next = Math.max(0, idx - 1);
  if (next !== idx) void openPhoto(list[next]);
}

function nextPhoto() {
  const list = libraryItems.get();
  if (!list.length) return;
  const current = activePhoto.get();
  const idx = current ? list.findIndex((p) => gridKey(p) === gridKey(current)) : -1;
  const next = Math.min(list.length - 1, idx === -1 ? 0 : idx + 1);
  if (next !== idx) void openPhoto(list[next]);
}

// ─── Global Shortcut Handler ────────────────────────────────────────────────
//
// NOTE: Undo/Redo (⌘Z / ⌘⇧Z) are intentionally NOT handled here. This app has
// real engine-backed undo/redo wired in App.svelte (via ipc/commands.ts
// undo()/redo() + stores/doc.ts reconcile()) — that listener already owns
// those combos. Duplicating a placeholder here would just shadow it with a
// no-op, so we let those two key combos fall through untouched.
export function handleGlobalShortcut(e: KeyboardEvent): void {
  const meta = e.metaKey || e.ctrlKey;
  const key = e.key.toLowerCase();

  if (meta && key === "k" && !e.shiftKey) {
    e.preventDefault();
    commandPaletteOpen.set(!commandPaletteOpen.get());
    return;
  }

  // Never intercept when the user is typing in an input
  if (isEditableTarget(e.target)) return;

  const shift = e.shiftKey;

  // ── Meta combos ───────────────────────────────────────────
  if (meta) {
    // ⌘, — Settings
    if (key === ",") {
      e.preventDefault();
      isSettingsOpen.set(true);
      return;
    }

    // ⌘E — Export
    if (key === "e" && !shift) {
      e.preventDefault();
      isExportOpen.set(true);
      return;
    }

    // ⌘Z / ⌘⇧Z — Undo/Redo: handled by App.svelte's real engine-backed
    // listener. Don't consume or preventDefault here.
    if (key === "z") {
      return;
    }

    // ⌘F — Focus search bar (Library)
    if (key === "f" && !shift) {
      if (isLibraryRoute()) {
        e.preventDefault();
        const searchInput = document.querySelector<HTMLInputElement>(".search-input");
        searchInput?.focus();
      }
      return;
    }

    // ⌘⇧C / ⌘⇧V — Copy / Paste Grade (not system clipboard)
    if (shift && key === "c") {
      e.preventDefault();
      void copyGrade();
      return;
    }
    if (shift && key === "v") {
      e.preventDefault();
      void pasteGrade();
      return;
    }

    // Don't intercept other ⌘ combos (copy, paste, etc.)
    return;
  }

  // ── Escape ────────────────────────────────────────────────
  if (key === "escape") {
    // Close modals in priority order
    if (isExportOpen.get()) { isExportOpen.set(false); return; }
    if (isSettingsOpen.get()) { isSettingsOpen.set(false); return; }
    if (isBugReportOpen.get()) { isBugReportOpen.set(false); return; }
    if (isShortcutsOpen.get()) { isShortcutsOpen.set(false); return; }
    if (commandPaletteOpen.get()) { commandPaletteOpen.set(false); return; }
    return;
  }

  // ── Single-key shortcuts ──────────────────────────────────

  // G — Library
  if (key === "g") {
    safePush(libraryRoute());
    return;
  }

  // D — Edit / Grade
  if (key === "d") {
    safePush(editorRoute());
    return;
  }

  // F — Fullscreen
  if (key === "f") {
    if (!document.fullscreenElement) {
      document.documentElement.requestFullscreen().catch(() => {});
    } else {
      document.exitFullscreen().catch(() => {});
    }
    return;
  }

  // L — Toggle sidebar
  if (key === "l") {
    leftRailCollapsed.set(!leftRailCollapsed.get());
    return;
  }

  // I — mark In on video; details panel on Library
  if (key === "i") {
    if (isEditVideo()) {
      window.dispatchEvent(new CustomEvent("meraraw:mark-in"));
      return;
    }
    if (isLibraryRoute()) {
      photoDetailsCollapsed.set(!photoDetailsCollapsed.get());
    }
    return;
  }

  if (key === "o" && isEditVideo()) {
    window.dispatchEvent(new CustomEvent("meraraw:mark-out"));
    return;
  }

  if (key === " " && isEditVideo()) {
    e.preventDefault();
    window.dispatchEvent(new CustomEvent("meraraw:video-play"));
    return;
  }

  if (key === "j" && isEditVideo()) {
    window.dispatchEvent(new CustomEvent("meraraw:video-reverse"));
    return;
  }
  if (key === "k" && isEditVideo()) {
    window.dispatchEvent(new CustomEvent("meraraw:video-pause"));
    return;
  }

  if (key === "[" ) {
    window.dispatchEvent(new CustomEvent("meraraw:look-prev"));
    return;
  }
  if (key === "]") {
    window.dispatchEvent(new CustomEvent("meraraw:look-next"));
    return;
  }

  // B — Toggle filmstrip
  if (key === "b") {
    imageBrowserCollapsed.set(!imageBrowserCollapsed.get());
    return;
  }

  // T — Toggle Zen mode (non-classic only)
  if (key === "t") {
    if (!classicLook.get()) {
      isZenMode.set(!isZenMode.get());
    }
    return;
  }

  // Z — Toggle zoom
  if (key === "z") {
    // Dispatch a custom event that the MainViewport listens to
    window.dispatchEvent(new CustomEvent("meraraw:toggle-zoom"));
    return;
  }

  // \ — Toggle compare
  if (key === "\\") {
    window.dispatchEvent(new CustomEvent("meraraw:toggle-compare"));
    return;
  }

  // 1–5 — Develop sections · 6 — AI agent
  if (key === "1") {
    applyEditFocus("light");
    return;
  }
  if (key === "2") {
    if (workspace.get() === "video") return;
    applyEditFocus("crop");
    return;
  }
  if (key === "3") {
    if (workspace.get() === "video") return;
    applyEditFocus("mask");
    return;
  }
  if (key === "4") {
    if (workspace.get() === "video") return;
    applyEditFocus("retouch");
    return;
  }
  if (key === "5") {
    applyEditFocus("presets");
    return;
  }
  if (key === "6") {
    showAiPanel();
    return;
  }

  // ArrowLeft / ArrowRight — frame-step on video, otherwise navigate photos
  if (key === "arrowleft") {
    if (isEditVideo()) {
      e.preventDefault();
      window.dispatchEvent(new CustomEvent("meraraw:video-step", { detail: -1 }));
      return;
    }
    prevPhoto();
    return;
  }
  if (key === "arrowright") {
    if (isEditVideo()) {
      e.preventDefault();
      window.dispatchEvent(new CustomEvent("meraraw:video-step", { detail: 1 }));
      return;
    }
    nextPhoto();
    return;
  }

  // Enter — Open selected photo in editor (Library)
  if (key === "enter") {
    if (isLibraryRoute() && activePhoto.get()) {
      safePush(editorRoute());
    }
    return;
  }
}

// ─── Shortcut Hint Labels (for UI tooltips & context menus) ─────────────────

const isMac =
  typeof navigator !== "undefined" && /mac/i.test(navigator.platform);
const MOD = isMac ? "⌘" : "Ctrl+";

export const shortcutLabels = {
  settings: `${MOD},`,
  export: `${MOD}E`,
  undo: `${MOD}Z`,
  redo: `${MOD}⇧Z`,
  search: `${MOD}F`,
  library: "G",
  edit: "D",
  fullscreen: "F",
  sidebar: "L",
  details: "I",
  filmstrip: "B",
  zenMode: "T",
  zoom: "Z",
  compare: "\\",
  lookPrev: "[",
  lookNext: "]",
  videoPlay: "Space",
  markIn: "I",
  markOut: "O",
  copyGrade: `${MOD}⇧C`,
  pasteGrade: `${MOD}⇧V`,
  toolEdit: "1",
  toolCrop: "2",
  toolMask: "3",
  toolAi: "4",
  toolPresets: "5",
  toolChat: "6",
  prevPhoto: "←",
  nextPhoto: "→",
  openPhoto: "Enter",
  escape: "Esc",
} as const;
