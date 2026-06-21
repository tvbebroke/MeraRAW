import { createClient, type Session, type SupabaseClient } from "@supabase/supabase-js";
import { invoke } from "@tauri-apps/api/core";
import { PURCHASE_URL, SUPABASE_ANON_KEY, SUPABASE_URL } from "../config";

let client: SupabaseClient | null = null;

export function getSupabase(): SupabaseClient {
  if (!client) {
    client = createClient(SUPABASE_URL, SUPABASE_ANON_KEY);
  }
  return client;
}

export async function activateLicense(session: Session): Promise<void> {
  const res = await fetch(`${SUPABASE_URL}/functions/v1/verify-license`, {
    method: "POST",
    headers: { Authorization: `Bearer ${session.access_token}` },
  });
  const result = (await res.json()) as { token?: string; error?: string };
  if (!res.ok || !result.token) {
    throw new Error(result.error || "License verification failed");
  }

  const valid = await invoke<boolean>("license_verify_token_locally", {
    token: result.token,
  });
  if (!valid) {
    throw new Error("Server token failed local signature check");
  }

  await invoke("license_save_token", { token: result.token });
}

export async function signIn(email: string, password: string): Promise<Session> {
  const supabase = getSupabase();
  const { data, error } = await supabase.auth.signInWithPassword({ email, password });
  if (error) throw error;
  if (!data.session) throw new Error("No session returned");
  return data.session;
}

export async function signOutAndClearLicense(): Promise<void> {
  const supabase = getSupabase();
  await supabase.auth.signOut();
  await invoke("license_clear_token");
}

export { PURCHASE_URL };
