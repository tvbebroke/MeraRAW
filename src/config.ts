/** Supabase + purchase site config — anon key is public (same as meratech.co). */

const DEFAULT_SUPABASE_URL = "https://kaanlfnxoyrjrgqrxcuz.supabase.co";
const DEFAULT_SUPABASE_ANON_KEY =
  "sb_publishable_7HICr8pQlJLALuYJzMrhjQ_mzaQt_4U";

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

export const SUPABASE_ANON_KEY =
  import.meta.env.VITE_SUPABASE_ANON_KEY ?? DEFAULT_SUPABASE_ANON_KEY;

export const PURCHASE_URL =
  import.meta.env.VITE_PURCHASE_URL ?? "https://www.meratech.co/#download";

export const EARLY_SUPPORTER_URL =
  import.meta.env.VITE_EARLY_SUPPORTER_URL ?? "https://www.meratech.co/early-supporter.html";

export const EARLY_SUPPORTER_PRICE = "$20";

export function supabaseConfigured(): boolean {
  return Boolean(SUPABASE_URL && SUPABASE_ANON_KEY && !SUPABASE_ANON_KEY.includes("YOUR_"));
}
