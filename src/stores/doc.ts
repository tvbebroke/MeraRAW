// Synced MIRROR of the canonical EditDoc (lives in Rust core). Never master.
// Reconciled from DocDelta on every accepted op / undo / redo / restore.
// (nanostores port of the old zustand docStore — same reconcile guard.)
import { atom } from "nanostores";
import type { DocDelta, EditDocMirror } from "../ipc/types";

export const doc = atom<EditDocMirror | null>(null);
export const docVersion = atom(0);
export const undoDepth = atom(0);
export const redoDepth = atom(0);
export const lastLabel = atom<string | null>(null);
export const historyLabels = atom<string[]>([]);

export function reconcile(delta: DocDelta): void {
  // Ignore out-of-order setParam responses (live throttle vs commit).
  // Undo increases redoDepth, so it still passes this guard.
  if (
    doc.get() !== null &&
    delta.undoDepth < undoDepth.get() &&
    delta.redoDepth <= redoDepth.get()
  ) {
    return;
  }
  doc.set(delta.doc);
  docVersion.set(docVersion.get() + 1);
  undoDepth.set(delta.undoDepth);
  redoDepth.set(delta.redoDepth);
  lastLabel.set(delta.label);
}

export function setDoc(d: EditDocMirror | null): void {
  doc.set(d);
  docVersion.set(docVersion.get() + 1);
}

export function clearDoc(): void {
  doc.set(null);
  docVersion.set(0);
  undoDepth.set(0);
  redoDepth.set(0);
  lastLabel.set(null);
  historyLabels.set([]);
}

/** Effective param value: doc override or undefined (= registry default). */
export function docParam(
  d: EditDocMirror | null,
  module: string,
  param: string,
): number | undefined {
  const v = d?.modules?.[module]?.[param];
  return typeof v === "number" ? v : undefined;
}
