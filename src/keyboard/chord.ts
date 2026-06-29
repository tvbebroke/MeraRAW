/** Cross-platform chord parsing and keyboard-event normalization. */

const EN_DASH = "\u2013";

export function isMacOs(): boolean {
  return /Mac|iPhone|iPad/i.test(navigator.platform);
}

function normalizeToken(token: string): string {
  const t = token.trim();
  if (t === "Option" || t === "Opt") return "Alt";
  if (t === "Return") return "Enter";
  if (t === "Spacebar") return "Space";
  if (t === "Side arrows") return "ArrowLeft"; // expanded separately
  if (t === "Esc") return "Escape";
  if (t === "Back quote" || t === "`") return "`";
  if (t.length === 1 && /[a-z]/i.test(t)) return t.toUpperCase();
  return t;
}

/** Expand `1–5`, `Ctrl+0–9`, etc. into discrete strings. */
export function expandNumericRanges(raw: string): string[] {
  const dash = raw.includes(EN_DASH) ? EN_DASH : raw.includes("-") ? "-" : null;
  if (!dash) return [raw];

  const idx = raw.lastIndexOf(dash);
  const left = raw.slice(0, idx);
  const right = raw.slice(idx + dash.length);
  const startMatch = left.match(/(\d+)$/);
  const endMatch = right.match(/^(\d+)(.*)$/);
  if (!startMatch || !endMatch) return [raw];

  const prefix = left.slice(0, startMatch.index);
  const start = parseInt(startMatch[1], 10);
  const end = parseInt(endMatch[1], 10);
  const suffix = endMatch[2] ?? "";
  if (Number.isNaN(start) || Number.isNaN(end)) return [raw];

  const out: string[] = [];
  const step = start <= end ? 1 : -1;
  for (let n = start; step > 0 ? n <= end : n >= end; n += step) {
    out.push(`${prefix}${n}${suffix}`);
  }
  return out;
}

function splitAlternatives(raw: string): string[] {
  return raw
    .split(/\s+\/\s+|\s+or\s+/i)
    .map((s) => s.trim())
    .filter(Boolean);
}

function isPointerBinding(raw: string, note: string): boolean {
  if (/click|drag|Double-click/i.test(raw)) return true;
  if (/Click-modifier|Click action|Drag action/i.test(note)) return true;
  return false;
}

/** Parse one OS key string into zero or more normalized chords. */
export function parseKeyString(raw: string, note = ""): string[] {
  if (!raw || raw === "n/a") return [];
  if (isPointerBinding(raw, note)) return [];

  const alternatives = splitAlternatives(raw);
  const chords: string[] = [];
  let carryMods = "";

  for (const alt of alternatives) {
    let part = alt;
    if (part.startsWith("+")) {
      part = `${carryMods}${part.slice(1)}`;
    }

    const rangeExpanded = expandNumericRanges(part);
    for (const expanded of rangeExpanded) {
      const sideArrowMatch = expanded.match(/^(.*)Alt\+Side arrows$/i);
      if (sideArrowMatch) {
        const base = sideArrowMatch[1].replace(/\+$/, "");
        for (const arrow of ["Left", "Right"]) {
          chords.push(normalizeChord(`${base}${base ? "+" : ""}Alt+${arrow}`));
        }
        continue;
      }

      const tokens = expanded.split("+").map(normalizeToken);
      if (tokens.length === 0) continue;
      carryMods = tokens.slice(0, -1).join("+");
      carryMods = carryMods ? `${carryMods}+` : "";
      chords.push(normalizeChord(tokens.join("+")));
    }
  }

  return [...new Set(chords)];
}

export function normalizeChord(chord: string): string {
  const tokens = chord.split("+").map(normalizeToken);
  const key = tokens.pop() ?? "";
  const mods = tokens.sort((a, b) => modOrder(a) - modOrder(b));
  return [...mods, key].join("+");
}

function modOrder(mod: string): number {
  switch (mod) {
    case "Ctrl":
      return 0;
    case "Cmd":
      return 1;
    case "Alt":
      return 2;
    case "Shift":
      return 3;
    default:
      return 4;
  }
}

export function eventToChord(e: KeyboardEvent): string {
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.metaKey) mods.push("Cmd");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");

  let key = e.key;
  if (key === " ") key = "Space";
  if (key === "ArrowLeft") key = "Left";
  if (key === "ArrowRight") key = "Right";
  if (key === "ArrowUp") key = "Up";
  if (key === "ArrowDown") key = "Down";
  if (key === "=" && e.shiftKey) key = "+";
  if (key.length === 1 && /[a-z]/i.test(key)) key = key.toUpperCase();

  mods.sort((a, b) => modOrder(a) - modOrder(b));
  return normalizeChord([...mods, key].join("+"));
}

export function chordMatchesEvent(chord: string, e: KeyboardEvent): boolean {
  return eventToChord(e) === normalizeChord(chord);
}

export function isTypingTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  if (el.isContentEditable) return true;
  return false;
}
