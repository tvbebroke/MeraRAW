import { eventToChord, isTypingTarget } from "./chord";
import type { BindingIndex } from "./loadKeymap";
import type {
  CommandHandler,
  DispatchResult,
  KeymapScope,
  ResolvedBinding,
} from "./types";

const handlers = new Map<string, CommandHandler>();
const debug = import.meta.env.DEV;

let scopeProvider: () => KeymapScope[] = () => ["global"];
let index: BindingIndex = new Map();

export function initKeyboardDispatcher(bindingIndex: BindingIndex) {
  index = bindingIndex;
}

export function setScopeProvider(fn: () => KeymapScope[]) {
  scopeProvider = fn;
}

export function registerCommandHandler(id: string, handler: CommandHandler) {
  handlers.set(id, handler);
}

export function registerCommandHandlers(map: Record<string, CommandHandler>) {
  for (const [id, handler] of Object.entries(map)) {
    handlers.set(id, handler);
  }
}

export function unregisterCommandHandler(id: string) {
  handlers.delete(id);
}

/** Invoke a registered handler by id with a synthetic chord (crop arrow nudges, etc.). */
export async function invokeCommand(id: string, chord = ""): Promise<boolean> {
  const handler = handlers.get(id);
  if (!handler) return false;
  const binding: ResolvedBinding = {
    id,
    section: "",
    action: id,
    chord,
    scope: "module:develop",
    relevance: "core",
    description: "",
    note: "",
    inputKind: "key",
  };
  await handler(binding);
  return true;
}

function resolveBinding(
  chord: string,
  scopes: KeymapScope[],
): { binding: ResolvedBinding; scope: KeymapScope } | null {
  for (const scope of scopes) {
    const list = index.get(`${scope}|${chord}`);
    if (list?.length) return { binding: list[0], scope };
  }
  return null;
}

export async function dispatchKeyEvent(e: KeyboardEvent): Promise<DispatchResult> {
  if (isTypingTarget(e.target)) {
    return { handled: false, reason: "blocked-input" };
  }

  const chord = eventToChord(e);
  const scopes = scopeProvider();
  const hit = resolveBinding(chord, scopes);

  if (!hit) {
    if (debug) {
      console.debug("[keybind] miss", { chord, scopes });
    }
    return { handled: false, reason: "no-match" };
  }

  const handler = handlers.get(hit.binding.id);
  if (!handler) {
    if (debug) {
      console.debug("[keybind] unmapped", {
        id: hit.binding.id,
        action: hit.binding.action,
        chord,
        scope: hit.scope,
      });
    }
    return { handled: false, binding: hit.binding, scope: hit.scope, reason: "no-handler" };
  }

  e.preventDefault();
  e.stopPropagation();

  if (debug) {
    console.debug("[keybind] fired", {
      id: hit.binding.id,
      action: hit.binding.action,
      scope: hit.scope,
      chord,
    });
  }

  await handler(hit.binding);
  return { handled: true, binding: hit.binding, scope: hit.scope };
}

export function bindingsForScopes(
  scopes: KeymapScope[],
  all: ResolvedBinding[],
): ResolvedBinding[] {
  const scopeSet = new Set<KeymapScope>(scopes);
  const seen = new Set<string>();
  const out: ResolvedBinding[] = [];
  for (const scope of scopes) {
    for (const b of all) {
      if (b.scope !== scope || !scopeSet.has(b.scope)) continue;
      const key = `${b.scope}|${b.chord}|${b.id}`;
      if (seen.has(key)) continue;
      seen.add(key);
      out.push(b);
    }
  }
  // Also include global fallbacks not shadowed
  for (const b of all) {
    if (b.scope !== "global") continue;
    const key = `${b.scope}|${b.chord}|${b.id}`;
    if (seen.has(key)) continue;
    const shadowed = scopes.some(
      (s) => s !== "global" && all.some((x) => x.scope === s && x.chord === b.chord),
    );
    if (shadowed) continue;
    seen.add(key);
    out.push(b);
  }
  return out.sort((a, b) => a.section.localeCompare(b.section) || a.action.localeCompare(b.action));
}
