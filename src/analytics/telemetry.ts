/**
 * Product telemetry (PostHog) — opt-in, anonymous, desktop-shaped.
 *
 * Rules this module enforces, in order of importance:
 *  1. Nothing is loaded or sent until the user explicitly opts in. The
 *     posthog-js bundle is dynamically imported on grant, so a user who
 *     declines never even parses it.
 *  2. Event properties are whitelisted, not filtered. A RAW editor's natural
 *     props are file paths and filenames — i.e. people's names, client names,
 *     and folder structure. Those must never leave the machine, so anything
 *     not explicitly declared safe is dropped.
 *  3. No autocapture, no pageviews, no session replay. This is a desktop app;
 *     DOM-level capture is noise, and `$current_url` is a `tauri://` path that
 *     can carry the open file's name.
 *
 * We use the `no-external` bundle: the app's CSP is `script-src 'self'`
 * (src-tauri/tauri.conf.json), so posthog's default behaviour of injecting
 * CDN <script> tags at runtime would silently fail. That bundle plus
 * `disable_external_dependency_loading` guarantees zero runtime script fetches.
 */

import { atom } from "nanostores";
// Types must come from the same entrypoint we import at runtime — the
// no-external bundle ships its own structurally-distinct copy of them.
import type { PostHog } from "posthog-js/dist/module.no-external";
import { POSTHOG_HOST, POSTHOG_KEY, telemetryConfigured } from "../config";

export type ConsentState = "unset" | "granted" | "denied";

const CONSENT_KEY = "telemetry-consent";

function readStoredConsent(): ConsentState {
  try {
    const raw = localStorage.getItem(CONSENT_KEY);
    return raw === "granted" || raw === "denied" ? raw : "unset";
  } catch {
    return "unset";
  }
}

function persistConsent(val: ConsentState): void {
  try {
    localStorage.setItem(CONSENT_KEY, val);
  } catch {
    /* ignore — private mode / storage disabled */
  }
}

/** "unset" until the user answers the first-run prompt. */
export const telemetryConsent = atom<ConsentState>(readStoredConsent());

/** True only while the first-run prompt should be on screen. */
export const needsConsentPrompt = atom<boolean>(
  telemetryConfigured() && readStoredConsent() === "unset",
);

let client: PostHog | null = null;
let loading: Promise<PostHog | null> | null = null;

/** Events queued between "user clicked Allow" and the bundle finishing load. */
const pending: Array<[string, Record<string, unknown>]> = [];

async function load(): Promise<PostHog | null> {
  if (client) return client;
  if (loading) return loading;

  loading = (async () => {
    try {
      // `dist/module.no-external` = everything bundled at build time, no
      // runtime CDN fetches. Plain `posthog-js` would try to load extensions.
      const mod = await import("posthog-js/dist/module.no-external");
      const posthog = mod.default ?? (mod as unknown as PostHog);

      posthog.init(POSTHOG_KEY, {
        api_host: POSTHOG_HOST,
        defaults: "2025-05-24",

        // CSP: script-src 'self' — no remote code, ever.
        disable_external_dependency_loading: true,

        // Desktop app: none of the web-shaped capture makes sense here.
        autocapture: false,
        capture_pageview: false,
        capture_pageleave: false,
        capture_performance: false,
        disable_session_recording: true,
        disable_surveys: true,

        // Anonymous unless we ever explicitly identify (we don't today).
        person_profiles: "identified_only",
        persistence: "localStorage",

        // `$current_url` under Tauri can embed the open document's path.
        property_denylist: [
          "$current_url",
          "$pathname",
          "$initial_current_url",
          "$initial_pathname",
          "$referrer",
          "$referring_domain",
          "$host",
          "$initial_referrer",
          "$initial_referring_domain",
        ],
      });

      client = posthog;
      for (const [event, props] of pending.splice(0)) {
        posthog.capture(event, props);
      }
      return posthog;
    } catch (err) {
      // Telemetry must never break the editor.
      if (import.meta.env.DEV) console.warn("[telemetry] init failed", err);
      loading = null;
      return null;
    }
  })();

  return loading;
}

/**
 * Whitelist: only these prop values are ever transmitted, and only as
 * primitives. Strings are length-capped so a stray path can't ride along.
 *
 * Deliberately absent: anything derived from a filename, directory, EXIF
 * body/lens serial, or user-authored text (preset names, AI grader prompts).
 */
const ALLOWED_PROPS = new Set([
  "algo",           // demosaic algorithm id, e.g. "rcd"
  "amount",         // denoise strength bucket
  "camera_model",   // e.g. "ILCE-7M3" — model, never a serial
  "color_space",    // export target space
  "count",          // batch size
  "duration_ms",
  "format",         // export format
  "gpu_adapter",    // e.g. "Apple M1 Pro"
  "look",           // display look index
  "outcome",        // "ok" | "cancelled" | "error"
  "panel",          // which edit panel
  "preset_source",  // "builtin" | "user" — never the preset name
  "reason",         // short enum-ish error reason, never a message
  "source",         // where an action was triggered from
  "tool",           // viewport tool id
  "app_version",
]);

const MAX_STRING = 64;

function sanitize(props?: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  if (!props) return out;
  for (const [key, val] of Object.entries(props)) {
    if (!ALLOWED_PROPS.has(key)) {
      if (import.meta.env.DEV) {
        console.warn(`[telemetry] dropped non-whitelisted prop "${key}"`);
      }
      continue;
    }
    if (typeof val === "string") {
      out[key] = val.slice(0, MAX_STRING);
    } else if (typeof val === "number" || typeof val === "boolean") {
      out[key] = val;
    }
    // objects/arrays/null are dropped — no nested payloads.
  }
  return out;
}

/**
 * Record a product event. No-op unless the user has opted in.
 * Safe to call from anywhere, including before the bundle has loaded.
 */
export function capture(event: string, props?: Record<string, unknown>): void {
  const clean = sanitize(props);

  if (import.meta.env.DEV) {
    console.debug("[telemetry]", event, clean);
  }

  if (telemetryConsent.get() !== "granted" || !telemetryConfigured()) return;

  if (client) {
    client.capture(event, clean);
  } else {
    if (pending.length < 50) pending.push([event, clean]);
    void load();
  }
}

/** Grant or revoke consent. Revoking stops sending and clears local state. */
export function setTelemetryConsent(granted: boolean): void {
  const next: ConsentState = granted ? "granted" : "denied";
  telemetryConsent.set(next);
  persistConsent(next);
  needsConsentPrompt.set(false);

  if (granted) {
    void load();
  } else if (client) {
    // opt_out_capturing also stops any queued/retrying requests.
    try {
      client.opt_out_capturing();
      client.reset();
    } catch {
      /* ignore */
    }
    client = null;
    loading = null;
    pending.length = 0;
  }
}

/** Call once at app start: resumes telemetry for a user who already opted in. */
export function initTelemetry(): void {
  if (!telemetryConfigured()) {
    needsConsentPrompt.set(false);
    return;
  }
  if (telemetryConsent.get() === "granted") void load();
}
