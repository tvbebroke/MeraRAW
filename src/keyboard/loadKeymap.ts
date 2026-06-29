import keymapFile from "../../keybinds/keymap.json";
import { isMacOs, parseKeyString } from "./chord";
import type { KeymapFile, KeymapScope, ResolvedBinding } from "./types";

const file = keymapFile as KeymapFile;

export function getKeymapMeta() {
  return { source: file.source, note: file.note, scopes: file.scopes };
}

export function loadBindings(): ResolvedBinding[] {
  const os = isMacOs() ? "macos" : "windows";
  const out: ResolvedBinding[] = [];

  for (const raw of file.bindings) {
    const keyStr = raw.keys[os];
    const altKeyStr = os === "macos" ? raw.keys.windows : raw.keys.macos;
    const chords = [
      ...parseKeyString(keyStr, raw.note),
      ...(keyStr === "n/a" ? parseKeyString(altKeyStr, raw.note) : []),
    ];
    const inputKind =
      chords.length === 0 &&
      (raw.note.includes("Click") || /click|drag/i.test(keyStr ?? ""))
        ? "pointer"
        : "key";

    const scopes = raw.scope.split(",").map((s) => s.trim()) as KeymapScope[];

    for (const chord of chords) {
      for (const scope of scopes) {
        out.push({
          id: raw.id,
          section: raw.section,
          action: raw.action,
          chord,
          scope,
          relevance: raw.relevance,
          description: raw.description,
          note: raw.note,
          inputKind,
        });
      }
    }
  }

  return out;
}

export type BindingIndex = Map<string, ResolvedBinding[]>;

/** Index: `${scope}|${chord}` → bindings (first wins at dispatch). */
export function buildBindingIndex(bindings: ResolvedBinding[]): BindingIndex {
  const index: BindingIndex = new Map();
  for (const b of bindings) {
    if (b.inputKind !== "key") continue;
    const k = `${b.scope}|${b.chord}`;
    const list = index.get(k) ?? [];
    list.push(b);
    index.set(k, list);
  }
  return index;
}

export function allKeyBindings(): ResolvedBinding[] {
  return loadBindings().filter((b) => b.inputKind === "key");
}
