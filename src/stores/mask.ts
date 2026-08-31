import { atom } from "nanostores";
import { setMaskOverlay } from "../ipc/commands";
import { selectedMask } from "./app";

/** Add / subtract when refining an existing mask (Lightroom-style). */
export type MaskRefineMode = "add" | "subtract";

export const maskRefineMode = atom<MaskRefineMode>("add");
export const brushHardness = atom(0.6);
export const brushFlow = atom(1.0);
export const maskOverlayVisible = atom(true);
export const maskPanelExpanded = atom(true);

/** True while dragging mask handles or painting a mask brush stroke. */
export const maskAdjusting = atom(false);

/** Which tool row is open in the masking panel. */
export type MaskToolGroup = "ai" | "manual" | "range" | null;
export const maskToolGroup = atom<MaskToolGroup>(null);

/** Apply overlay visibility: hidden while adjusting so edits are visible. */
export function syncMaskOverlay(): void {
  const id = selectedMask.get();
  const show = maskOverlayVisible.get() && !maskAdjusting.get() && !!id;
  void setMaskOverlay(show ? id : null);
}

export function beginMaskAdjust(): void {
  if (!maskAdjusting.get()) {
    maskAdjusting.set(true);
    syncMaskOverlay();
  }
}

export function endMaskAdjust(): void {
  if (maskAdjusting.get()) {
    maskAdjusting.set(false);
    syncMaskOverlay();
  }
}

export function maskDisplayName(kind: string, index: number): string {
  const labels: Record<string, string> = {
    subject: "Subject",
    sky: "Sky",
    background: "Background",
    object: "Object",
    brush: "Brush",
    linear: "Linear Gradient",
    radial: "Radial Gradient",
    parametric: "Range",
  };
  return labels[kind] ?? `Mask ${index + 1}`;
}
