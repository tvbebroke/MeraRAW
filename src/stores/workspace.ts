// Photo vs video are two products on one engine. Default is photo — original MeraRAW.
import { atom } from "nanostores";
import { push, router } from "svelte-spa-router";
import { cropActive, imageMeta, imageOpen, viewportTool } from "./app";
import { isVideoPath } from "../lib/media";

export type WorkspaceId = "photo" | "video";

/** Video editor is built but not offered yet. Flip this to re-enable /clips and /grade. */
export const VIDEO_WORKSPACE_ENABLED = false;

const WS_KEY = "meraraw.workspace";
const CHOSEN_KEY = "meraraw.workspaceChosen";

function readWorkspace(): WorkspaceId {
  if (!VIDEO_WORKSPACE_ENABLED) return "photo";
  try {
    const v = localStorage.getItem(WS_KEY);
    if (v === "video" || v === "photo") return v;
  } catch {
    /* ignore */
  }
  return "photo";
}

function readChosen(): boolean {
  try {
    return localStorage.getItem(CHOSEN_KEY) === "true";
  } catch {
    return false;
  }
}

export const workspace = atom<WorkspaceId>(readWorkspace());
export const workspaceChosen = atom<boolean>(readChosen());

function persist(mode: WorkspaceId, chosen: boolean) {
  try {
    localStorage.setItem(WS_KEY, mode);
    if (chosen) localStorage.setItem(CHOSEN_KEY, "true");
  } catch {
    /* ignore */
  }
}

export function libraryRoute(mode: WorkspaceId = workspace.get()): string {
  return mode === "video" ? "/clips" : "/library";
}

export function editorRoute(mode: WorkspaceId = workspace.get()): string {
  return mode === "video" ? "/grade" : "/edit";
}

export function isVideoRoute(loc: string = router.location): boolean {
  return loc === "/clips" || loc === "/grade";
}

export function isLibraryRoute(loc: string = router.location): boolean {
  return loc === "/library" || loc === "/clips";
}

export function isEditorRoute(loc: string = router.location): boolean {
  return loc === "/edit" || loc === "/grade" || loc === "/";
}

function go(mode: WorkspaceId) {
  const open = imageOpen.get();
  const kind = imageMeta.get()?.kind;
  const path = imageMeta.get()?.path;
  const isVid = kind === "video" || isVideoPath(path);
  if (mode === "video") {
    push(open && isVid ? "/grade" : "/clips");
  } else {
    push(open && !isVid ? "/edit" : "/library");
  }
}

/** User picked a workspace (chooser or title bar). */
export function setWorkspace(mode: WorkspaceId, opts?: { navigate?: boolean }): void {
  if (mode === "video" && !VIDEO_WORKSPACE_ENABLED) return;
  workspace.set(mode);
  workspaceChosen.set(true);
  persist(mode, true);
  if (mode === "video") {
    cropActive.set(false);
    viewportTool.set("pan");
  }
  if (opts?.navigate !== false) go(mode);
}

/** Route is the source of truth when the user lands on /grade or /library. */
export function adoptWorkspaceFromRoute(loc: string): void {
  if (!VIDEO_WORKSPACE_ENABLED && (loc === "/clips" || loc === "/grade")) {
    workspace.set("photo");
    persist("photo", workspaceChosen.get());
    push("/library");
    return;
  }
  if (loc === "/clips" || loc === "/grade") {
    if (workspace.get() !== "video") {
      workspace.set("video");
      persist("video", workspaceChosen.get());
    }
  } else if (loc === "/library" || loc === "/edit") {
    if (workspace.get() !== "photo") {
      workspace.set("photo");
      persist("photo", workspaceChosen.get());
    }
  }
}

/** Opening a file selects the matching product without discarding the other. */
export function workspaceForOpenPath(path: string, kind?: string | null): WorkspaceId {
  if (!VIDEO_WORKSPACE_ENABLED) return "photo";
  if (kind === "video" || isVideoPath(path)) return "video";
  return "photo";
}

export function syncWorkspaceToOpenFile(path: string, kind?: string | null): string {
  const mode = workspaceForOpenPath(path, kind);
  workspace.set(mode);
  persist(mode, workspaceChosen.get());
  return editorRoute(mode);
}
