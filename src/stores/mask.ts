import { atom } from "nanostores";
import { setMaskOverlay } from "../ipc/commands";
import { selectedMask, type ViewportTool, viewportTool } from "./app";
import { doc } from "./doc";
import type { MaskMirror } from "../ipc/types";

/** Add / subtract / intersect when refining an existing mask (Lightroom-style). */
export type MaskRefineMode = "add" | "subtract" | "intersect";

export const maskRefineMode = atom<MaskRefineMode>("add");
export const brushHardness = atom(0.6);
export const brushFlow = atom(1.0);
export const maskOverlayVisible = atom(true);
/** 0 red · 1 white · 2 black · 3 color-on-B&W */
export type MaskOverlayMode = 0 | 1 | 2 | 3;
export const maskOverlayMode = atom<MaskOverlayMode>(0);
/** Tint strength 0..1 */
export const maskOverlayStrength = atom(0.55);
export const maskPanelExpanded = atom(true);

/** True while dragging mask handles, painting, or editing scoped sliders. */
export const maskAdjusting = atom(false);

/** Segmented mask ids waiting for on-device inference. */
export const maskPendingIds = atom<Set<string>>(new Set());

/** Last segmentation error per mask id. */
export const maskErrors = atom<Map<string, string>>(new Map());

/** Click-to-select object mode: next viewport tap creates an object mask. */
export const objectPickActive = atom(false);

/** Color-range eyedropper: next tap samples into the selected parametric mask. */
export const colorPickActive = atom(false);

export function clearMaskError(id: string): void {
  const next = new Map(maskErrors.get());
  next.delete(id);
  maskErrors.set(next);
}

export function setMaskError(id: string, message: string): void {
  const next = new Map(maskErrors.get());
  next.set(id, message);
  maskErrors.set(next);
}

export function markMaskPending(id: string): void {
  const next = new Set(maskPendingIds.get());
  next.add(id);
  maskPendingIds.set(next);
  clearMaskError(id);
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

export function maskIsEnabled(m: MaskMirror | null | undefined): boolean {
  return m?.enabled !== false;
}

/** Apply overlay visibility: hidden while adjusting or when mask is disabled. */
export function syncMaskOverlay(): void {
  const id = selectedMask.get();
  const m = id ? doc.get()?.masks?.find((x) => x.id === id) : null;
  const show =
    maskOverlayVisible.get() &&
    !maskAdjusting.get() &&
    !!id &&
    maskIsEnabled(m);
  void setMaskOverlay(show ? id : null, {
    strength: maskOverlayStrength.get(),
    mode: maskOverlayMode.get(),
  });
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

/** Click off / Escape: hide overlay and handles; mask edits stay in the render. */
export function deselectMask(): void {
  if (!selectedMask.get()) return;
  selectedMask.set(null);
  maskAdjusting.set(false);
  syncViewportToolForMask(null);
  void setMaskOverlay(null);
}

export function maskDisplayName(kind: string, index: number, name?: string | null): string {
  if (name && name.trim()) return name.trim();
  const labels: Record<string, string> = {
    subject: "Subject",
    sky: "Sky",
    background: "Background",
    object: "Object",
    people: "People",
    skin: "Skin",
    hair: "Hair",
    brush: "Brush",
    linear: "Linear Gradient",
    radial: "Radial Gradient",
    parametric: "Range",
  };
  return labels[kind] ?? `Mask ${index + 1}`;
}
