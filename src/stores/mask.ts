import { atom } from "nanostores";
import { setMaskOverlay } from "../ipc/commands";
import { selectedMask, type ViewportTool, viewportTool } from "./app";
import type { MaskMirror } from "../ipc/types";

/** Add / subtract when refining an existing mask (Lightroom-style). */
export type MaskRefineMode = "add" | "subtract";

export const maskRefineMode = atom<MaskRefineMode>("add");
export const brushHardness = atom(0.6);
export const brushFlow = atom(1.0);
export const maskOverlayVisible = atom(true);
export const maskPanelExpanded = atom(true);

/** True while dragging mask handles, painting, or editing scoped sliders. */
export const maskAdjusting = atom(false);

/** Segmented mask ids waiting for on-device inference. */
export const maskPendingIds = atom<Set<string>>(new Set());

export function markMaskPending(id: string): void {
  const next = new Set(maskPendingIds.get());
  next.add(id);
  maskPendingIds.set(next);
}

export function clearMaskPending(id: string): void {
  const next = new Set(maskPendingIds.get());
  next.delete(id);
  maskPendingIds.set(next);
}

function sourceHasGeometry(source: MaskMirror["source"]): boolean {
  if (!source) return false;
  if (source.type === "radial" || source.type === "linear") return true;
  if (source.type === "composite") {
    const comps = source.components as { source?: { type?: string } }[] | undefined;
    return comps?.some((c) => c.source?.type === "radial" || c.source?.type === "linear") ?? false;
  }
  return false;
}

export function maskHasGeometry(m: MaskMirror | null | undefined): boolean {
  if (!m) return false;
  if (m.kind === "radial" || m.kind === "linear") return true;
  return sourceHasGeometry(m.source);
}

export function syncViewportToolForMask(m: MaskMirror | null | undefined): void {
  if (!m) {
    viewportTool.set("pan");
    return;
  }
  let tool: ViewportTool = "pan";
  if (m.kind === "brush") tool = "brush";
  else if (maskHasGeometry(m)) tool = "mask-geo";
  viewportTool.set(tool);
}

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
