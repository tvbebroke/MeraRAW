// Engine/app state shared across the Svelte UI (successor to the old
// zustand uiStore — only what the new shell actually consumes).
import { atom } from "nanostores";
import type { ImageMeta } from "../ipc/types";

export const engineReady = atom(false);
export const gpuAdapter = atom<string | null>(null);
export const statusMessage = atom("");

export const imageOpen = atom(false);
export const lastOpenedPath = atom<string | null>(null);
export const imageMeta = atom<ImageMeta | null>(null);
export const imageDims = atom<{ w: number; h: number } | null>(null);
export type DecodeState = "idle" | "preview" | "ready" | "error";
export const decodeState = atom<DecodeState>("idle");

/** Latest engine frame version (bumped by frame-ready / image-ready). */
export const frameVersion = atom(0);
export const zoomLabel = atom("fit");

/** Display look: 0 Neutral, 1 Camera, 2 Filmic/AgX, 4 Original. */
export const displayLook = atom(1);
/** Before/after: render the un-edited base when true. */
export const previewBypass = atom(false);

/** Mask scoping for panel edits: params target mask.<id>.<path> when set. */
export const selectedMask = atom<string | null>(null);

/** Viewport tool (pan / white-balance eyedropper / brush / crop). */
export type ViewportTool = "pan" | "wb" | "brush" | "crop";
export const viewportTool = atom<ViewportTool>("pan");
export const brushRadius = atom(0.05);
export const cropActive = atom(false);

/** One-shot viewport commands from the chrome bar. */
export type ViewCmd = "fit" | "oneToOne" | "zoomIn" | "zoomOut" | null;
export const viewCmd = atom<ViewCmd>(null);
export const viewCmdNonce = atom(0);
export function sendViewCmd(cmd: Exclude<ViewCmd, null>): void {
  viewCmd.set(cmd);
  viewCmdNonce.set(viewCmdNonce.get() + 1);
}

/** Current browse folder (catalog-backed). */
export const currentFolder = atom<string | null>(null);
