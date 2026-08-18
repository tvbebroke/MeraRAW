import { setMaskOverlay } from "../../ipc/commands";
import {
  cropActive,
  selectedMask,
  selectedRetouch,
  viewportTool,
} from "../../stores/app";
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
    viewportTool.set("brush");
    const mid = selectedMask.get();
    void setMaskOverlay(mid);
  } else if (id === "ai") {
    selectedMask.set(null);
    void setMaskOverlay(null);
    viewportTool.set(selectedRetouch.get() ? "brush" : "pan");
  } else {
    selectedRetouch.set(null);
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
  rightPanelMode.set("edit");
  if (focus === "crop") applyTool("crop");
  else if (focus === "mask") applyTool("mask");
  else if (focus === "retouch") applyTool("ai");
  else if (focus === "presets") applyTool("presets");
  else applyTool("edit");

  if (!focus) return;
  const id = FOCUS_TO_SECTION[focus];
  openSections.setKey(id, true);
  queueMicrotask(() => {
    document
      .querySelector(`[data-section="${id}"]`)
      ?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  });
}

export function showAiPanel() {
  rightPanelMode.set("ai");
  applyTool("edit");
  window.dispatchEvent(new CustomEvent("meraraw:focus-agent"));
}
