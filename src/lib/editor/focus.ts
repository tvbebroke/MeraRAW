import { setMaskOverlay } from "../../ipc/commands";
import {
  cropActive,
  selectedMask,
  selectedRetouch,
  viewportTool,
} from "../../stores/app";
import { doc } from "../../stores/doc";
import { syncMaskOverlay } from "../../stores/mask";
import {
  activeTool,
  editFocus,
  openSections,
  rightPanelMode,
  type EditFocus,
  type SectionId,
  type Tool,
} from "../../stores/editor";

export function applyTool(id: Tool) {
  activeTool.set(id);
  cropActive.set(id === "crop");
  if (id === "crop") {
    selectedRetouch.set(null);
    viewportTool.set("crop");
    void setMaskOverlay(null);
  } else if (id === "mask") {
    selectedRetouch.set(null);
    const mid = selectedMask.get();
    const m = mid ? doc.get()?.masks?.find((x) => x.id === mid) : null;
    viewportTool.set(m?.kind === "brush" ? "brush" : "pan");
    syncMaskOverlay();
  } else if (id === "ai") {
    selectedMask.set(null);
    void setMaskOverlay(null);
    viewportTool.set(selectedRetouch.get() ? "brush" : "pan");
  } else {
    selectedRetouch.set(null);
    selectedMask.set(null);
    viewportTool.set("pan");
    void setMaskOverlay(null);
  }
}

const FOCUS_TO_SECTION: Record<Exclude<EditFocus, null>, SectionId> = {
  light: "light",
  color: "color",
  curve: "curve",
  detail: "detail",
  grading: "grading",
  crop: "crop",
  mask: "mask",
  retouch: "retouch",
  camera: "camera",
  presets: "presets",
};

export function applyEditFocus(focus: EditFocus) {
  editFocus.set(focus);
  if (focus === "mask") {
    showMaskPanel();
  } else if (focus === "crop") {
    rightPanelMode.set("edit");
    applyTool("crop");
  } else if (focus === "retouch") {
    rightPanelMode.set("edit");
    applyTool("ai");
  } else if (focus === "presets") {
    rightPanelMode.set("edit");
    applyTool("presets");
  } else {
    rightPanelMode.set("edit");
    applyTool("edit");
  }

  if (!focus) return;
  const id = FOCUS_TO_SECTION[focus];
  openSections.setKey(id, true);
  if (focus !== "mask") {
    queueMicrotask(() => {
      document
        .querySelector(`[data-section="${id}"]`)
        ?.scrollIntoView({ behavior: "smooth", block: "nearest" });
    });
  }
}

export function showAiPanel() {
  rightPanelMode.set("ai");
  applyTool("edit");
  window.dispatchEvent(new CustomEvent("meraraw:focus-agent"));
}

/** Lightroom-style masking rail: tools + scoped adjustments. */
export function showMaskPanel() {
  rightPanelMode.set("mask");
  applyTool("mask");
}
