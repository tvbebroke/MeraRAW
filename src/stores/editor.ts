// UI-only editor state (tool, section disclosure, layout). Engine state
// lives in stores/app.ts + stores/doc.ts; params flow via lib/engine/params.
import { atom, map } from "nanostores";

export type Tool = "edit" | "crop" | "mask" | "ai" | "presets" | "chat";
export const activeTool = atom<Tool>("edit");

export type SectionId =
  | "profile"
  | "light"
  | "color"
  | "detail"
  | "effects"
  | "optics"
  | "demosaic"
  | "lut"
  | "cropAspect"
  | "cropTransform"
  | "maskList"
  | "aiSelect"
  | "presetList";
export const openSections = map<Record<SectionId, boolean>>({
  profile: true,
  light: true,
  color: true,
  detail: true,
  effects: true,
  optics: false,
  demosaic: false,
  lut: false,
  cropAspect: true,
  cropTransform: true,
  maskList: true,
  aiSelect: true,
  presetList: true,
});
export const toggleSection = (id: SectionId) =>
  openSections.setKey(id, !openSections.get()[id]);

/** Tone-curve channel selector (matches engine paths). */
export type CurveChannel = "luma" | "red" | "green" | "blue";
export const curveChannel = atom<CurveChannel>("luma");

/** Split-toning display mode (matches engine paths; UI-only selector). */
export type SplitToningMode = "perceptual" | "classic" | "light";
export const splitToningMode = atom<SplitToningMode>("perceptual");

export const leftRailCollapsed = atom<boolean>(false);
export const isZenMode = atom<boolean>(false);
export const imageBrowserCollapsed = atom<boolean>(false);
export const photoDetailsCollapsed = atom<boolean>(false);

/** Re-export browse atoms so older intern imports keep compiling. */
export {
  folder,
  photos,
  activePhoto,
} from "./browse";
