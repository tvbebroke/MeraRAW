/** Supabase + purchase site config — set via .env at build/dev time. */
export const SUPABASE_URL =
  import.meta.env.VITE_SUPABASE_URL ?? "https://kaanlfnxoyrjrgqrxcuz.supabase.co";

export const SUPABASE_ANON_KEY = import.meta.env.VITE_SUPABASE_ANON_KEY ?? "";

export const PURCHASE_URL =
  import.meta.env.VITE_PURCHASE_URL ?? "https://www.meratech.co/#download";

export function supabaseConfigured(): boolean {
  return Boolean(SUPABASE_URL && SUPABASE_ANON_KEY && !SUPABASE_ANON_KEY.includes("YOUR_"));
}
