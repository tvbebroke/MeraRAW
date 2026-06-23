/** Supabase + purchase site config — anon key is public (same as meratech.co). */
export const SUPABASE_URL =
  import.meta.env.VITE_SUPABASE_URL ?? "https://kaanlfnxoyrjrgqrxcuz.supabase.co";

export const SUPABASE_ANON_KEY =
  import.meta.env.VITE_SUPABASE_ANON_KEY ??
  "sb_publishable_7HICr8pQlJLALuYJzMrhjQ_mzaQt_4U";

export const PURCHASE_URL =
  import.meta.env.VITE_PURCHASE_URL ?? "https://www.meratech.co/#download";

export function supabaseConfigured(): boolean {
  return Boolean(SUPABASE_URL && SUPABASE_ANON_KEY && !SUPABASE_ANON_KEY.includes("YOUR_"));
}
