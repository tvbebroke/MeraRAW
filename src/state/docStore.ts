// Synced MIRROR of the canonical EditDoc (lives in Rust core). Never master.
// Reconciled from DocDelta on every accepted op / undo / redo / restore.
import { create } from "zustand";
import type { DocDelta, EditDocMirror } from "../ipc/types";

interface DocState {
  doc: EditDocMirror | null;
  docVersion: number;
  undoDepth: number;
  redoDepth: number;
  lastLabel: string | null;
  history: string[];
  reconcile: (delta: DocDelta) => void;
  setDoc: (doc: EditDocMirror | null) => void;
  setHistory: (labels: string[]) => void;
  clear: () => void;
}

export const useDocStore = create<DocState>((set) => ({
  doc: null,
  docVersion: 0,
  undoDepth: 0,
  redoDepth: 0,
  lastLabel: null,
  history: [],
  reconcile: (delta) =>
    set((s) => ({
      doc: delta.doc,
      docVersion: s.docVersion + 1,
      undoDepth: delta.undoDepth,
      redoDepth: delta.redoDepth,
      lastLabel: delta.label,
    })),
  setDoc: (doc) => set((s) => ({ doc, docVersion: s.docVersion + 1 })),
  setHistory: (labels) => set({ history: labels }),
  clear: () =>
    set({
      doc: null,
      docVersion: 0,
      undoDepth: 0,
      redoDepth: 0,
      lastLabel: null,
      history: [],
    }),
}));

/** Effective param value: doc override or undefined (= registry default). */
export function docParam(
  doc: EditDocMirror | null,
  module: string,
  param: string,
): number | undefined {
  const v = doc?.modules?.[module]?.[param];
  return typeof v === "number" ? v : undefined;
}
