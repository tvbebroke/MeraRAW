// Registry-driven param bridge: every slider edit flows through the
// registry → ops → doc-mirror reconcile loop (same contract as the old
// React useParam hook, reimplemented framework-free for Svelte).
import { atom } from "nanostores";
import { getRegistry, setParam } from "../../ipc/commands";
import type { EditDocMirror, ImageMeta, ParamSpec } from "../../ipc/types";
import { docParam, reconcile } from "../../stores/doc";
import { selectedMask } from "../../stores/app";
import { beginMaskAdjust, endMaskAdjust } from "../../stores/mask";

export const registry = atom<ParamSpec[]>([]);

let registryLoaded = false;
export async function loadRegistry(): Promise<void> {
  if (registryLoaded) return;
  registryLoaded = true;
  try {
    registry.set(await getRegistry());
  } catch {
    registryLoaded = false;
  }
}

export function findSpec(specs: ParamSpec[], path: string): ParamSpec | undefined {
  return specs.find((s) => s.path === path);
}

/** Effective value shown in the UI: mask override / doc value / as-shot / default. */
export function effectiveValue(
  d: EditDocMirror | null,
  spec: ParamSpec,
  meta: ImageMeta | null,
  maskId: string | null,
): number {
  const dot = spec.path.indexOf(".");
  const module = spec.path.slice(0, dot);
  const param = spec.path.slice(dot + 1);
  const specDefault = typeof spec.default === "number" ? spec.default : 0;
  const asShot =
    !maskId && module === "white_balance" && param === "temp"
      ? meta?.estimatedCct ?? undefined
      : undefined;
  const fallback = asShot ?? specDefault;
  if (maskId) {
    const v = d?.masks?.find((m) => m.id === maskId)?.modules?.[module]?.[param];
    return typeof v === "number" ? v : fallback;
  }
  return docParam(d, module, param) ?? fallback;
}

/** Default the UI resets to (mask edits reset to 0/spec default, not as-shot). */
export function defaultValue(spec: ParamSpec, meta: ImageMeta | null, maskId: string | null): number {
  return effectiveValue(null, spec, meta, maskId);
}

interface SendState {
  seq: number;
  inFlight: boolean;
  pendingLive: number | null;
}
const sendStates = new Map<string, SendState>();

function stateFor(path: string): SendState {
  let s = sendStates.get(path);
  if (!s) {
    s = { seq: 0, inFlight: false, pendingLive: null };
    sendStates.set(path, s);
  }
  return s;
}

function targetPath(path: string): string {
  const mask = selectedMask.get();
  return mask ? `mask.${mask}.${path}` : path;
}

function send(path: string, v: number, live: boolean): Promise<void> {
  const s = stateFor(path);
  const seq = ++s.seq;
  return setParam(targetPath(path), v, live)
    .then((delta) => {
      if (seq !== s.seq) return;
      reconcile(delta);
    })
    .catch(() => {});
}

/**
 * In-flight coalescing for drags: send the newest value immediately; while
 * one is outstanding, keep only the latest and fire it the moment the
 * previous resolves. Never drops the final value; rate self-limits to
 * whatever the engine keeps up with.
 */
function pumpLive(path: string): void {
  const s = stateFor(path);
  if (s.inFlight || s.pendingLive === null) return;
  const v = s.pendingLive;
  s.pendingLive = null;
  s.inFlight = true;
  void send(path, v, true).finally(() => {
    s.inFlight = false;
    pumpLive(path);
  });
}

/** Live (mid-drag) update — coalesced into one undo step engine-side. */
export function setParamLive(path: string, v: number): void {
  if (selectedMask.get()) beginMaskAdjust();
  const s = stateFor(path);
  s.pendingLive = v;
  pumpLive(path);
}

/** Final committed value (drag release / typed entry / reset). */
export function commitParam(path: string, v: number): Promise<void> {
  const s = stateFor(path);
  s.pendingLive = null;
  if (selectedMask.get()) beginMaskAdjust();
  return send(path, v, false).finally(() => {
    if (selectedMask.get()) endMaskAdjust();
  });
}
