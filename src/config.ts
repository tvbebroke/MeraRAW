/** Supabase + purchase site config. All auth I/O goes through Rust commands. */

const DEFAULT_SUPABASE_URL = "https://kaanlfnxoyrjrgqrxcuz.supabase.co";

function supabaseHostAllowed(host: string): boolean {
  return host.toLowerCase() === "kaanlfnxoyrjrgqrxcuz.supabase.co";
}

function resolveSupabaseUrl(raw: string | undefined): string {
  const candidate = (raw ?? DEFAULT_SUPABASE_URL).trim();
  try {
    const u = new URL(candidate);
    if (u.protocol !== "https:") return DEFAULT_SUPABASE_URL;
    if (!u.hostname || !supabaseHostAllowed(u.hostname)) return DEFAULT_SUPABASE_URL;
    return `https://${u.hostname}`;
  } catch {
    return DEFAULT_SUPABASE_URL;
  }
}

export const SUPABASE_URL = resolveSupabaseUrl(import.meta.env.VITE_SUPABASE_URL);

export const EARLY_SUPPORTER_URL =
  import.meta.env.VITE_EARLY_SUPPORTER_URL ?? "https://www.meratech.co/early-supporter.html";

/**
 * PostHog product telemetry. The project API key is a write-only ingest key
 * and is safe to ship in the binary (same class as the Supabase anon key).
 * Absent key = telemetry disabled entirely, including the consent prompt.
 *
 * Changing POSTHOG_HOST also requires updating `connect-src` in
 * src-tauri/tauri.conf.json, or requests are blocked by CSP.
 */
export const POSTHOG_KEY = import.meta.env.VITE_POSTHOG_KEY ?? "";

export const POSTHOG_HOST =
  import.meta.env.VITE_POSTHOG_HOST ?? "https://us.i.posthog.com";

export function telemetryConfigured(): boolean {
  return POSTHOG_KEY.startsWith("phc_");
}
