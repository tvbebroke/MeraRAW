// Shared controller for the psychedelic background <video>.

export type PsyMediaState = {
  playing: boolean;
  muted: boolean;
  volume: number; // 0–1
};

const VOLUME_KEY = "meraraw.psyVolume";
const MUTED_KEY = "meraraw.psyMuted";

function readVolume(): number {
  try {
    const v = Number(localStorage.getItem(VOLUME_KEY));
    if (Number.isFinite(v)) return Math.min(1, Math.max(0, v));
  } catch {
    /* ignore */
  }
  return 0.4;
}

function readMuted(): boolean {
  try {
    const v = localStorage.getItem(MUTED_KEY);
    if (v === "0" || v === "false") return false;
    if (v === "1" || v === "true") return true;
  } catch {
    /* ignore */
  }
  // Default muted — unmuted autoplay was freezing the app.
  return true;
}

let video: HTMLVideoElement | null = null;
let state: PsyMediaState = {
  playing: false,
  muted: readMuted(),
  volume: readVolume(),
};
const listeners = new Set<() => void>();

function emit() {
  for (const fn of listeners) fn();
}

function setState(patch: Partial<PsyMediaState>) {
  state = { ...state, ...patch };
  emit();
}

export function getPsyMediaState(): PsyMediaState {
  return state;
}

export function subscribePsyMedia(fn: () => void): () => void {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}

export function bindPsyVideo(el: HTMLVideoElement | null) {
  if (video && video !== el) {
    video.pause();
  }
  video = el;
  if (!el) {
    setState({ playing: false });
    return;
  }
  el.loop = true;
  el.playsInline = true;
  el.muted = state.muted;
  el.volume = state.volume;
  // Never autoplay — user hits Play. Avoids freeze / heavy decode on switch-in.
  el.pause();
  setState({ playing: false });
}

export async function psyTogglePlay(): Promise<void> {
  const el = video;
  if (!el) return;
  if (!el.paused) {
    el.pause();
    setState({ playing: false });
    return;
  }
  try {
    await el.play();
    setState({ playing: true });
  } catch {
    setState({ playing: false });
  }
}

export function psyRewind(): void {
  const el = video;
  if (!el) return;
  el.currentTime = 0;
}

export function psySetMuted(muted: boolean): void {
  const el = video;
  if (el) el.muted = muted;
  try {
    localStorage.setItem(MUTED_KEY, muted ? "1" : "0");
  } catch {
    /* ignore */
  }
  setState({ muted });
}

export function psyToggleMute(): void {
  psySetMuted(!state.muted);
}

export function psySetVolume(volume: number): void {
  const v = Math.min(1, Math.max(0, volume));
  const muted = v === 0;
  const storedVol = v > 0 ? v : state.volume || 0.4;
  const el = video;
  if (el) {
    el.volume = storedVol;
    el.muted = muted;
  }
  try {
    localStorage.setItem(VOLUME_KEY, String(storedVol));
    localStorage.setItem(MUTED_KEY, muted ? "1" : "0");
  } catch {
    /* ignore */
  }
  setState({ volume: storedVol, muted });
}

/** Sync React state from video element events. */
export function psyOnPlay(): void {
  setState({ playing: true });
}
export function psyOnPause(): void {
  setState({ playing: false });
}
