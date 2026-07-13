/** License helpers — all network I/O goes through Rust commands (no webview fetch). */
import { invoke } from "@tauri-apps/api/core";
import { PURCHASE_URL } from "../config";

export async function signInAndActivate(
  email: string,
  password: string,
): Promise<string> {
  return invoke<string>("license_sign_in_and_activate", { email, password });
}

export async function clearLicense(): Promise<void> {
  await invoke("license_clear_token");
}

export async function checkLocalLicense(): Promise<{
  licensed: boolean;
  userId?: string;
}> {
  return invoke("license_check_local");
}

export { PURCHASE_URL };
