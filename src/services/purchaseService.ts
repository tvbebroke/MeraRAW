import { invoke } from "@tauri-apps/api/core";
import { EARLY_SUPPORTER_URL, SUPABASE_URL } from "../config";
import { trackSupporterEvent } from "../analytics/supporterAnalytics";

export interface SupporterStatus {
  licensed: boolean;
  isEarlySupporter: boolean;
  userId: string | null;
  reason: string | null;
}

/** Read local license + early-supporter flag from saved JWT. */
export async function getSupporterStatus(): Promise<SupporterStatus> {
  try {
    return await invoke<SupporterStatus>("license_supporter_status");
  } catch {
    return {
      licensed: false,
      isEarlySupporter: false,
      userId: null,
      reason: "unavailable",
    };
  }
}

/** Open meratech.co checkout in the system browser (user signs in on web). */
export function openEarlySupporterPage(surface?: string): void {
  trackSupporterEvent("supporter_page_viewed", { surface: surface ?? "app" });
  void invoke("open_external_url", { url: EARLY_SUPPORTER_URL }).catch(() => {
    window.open(EARLY_SUPPORTER_URL, "_blank", "noopener,noreferrer");
  });
}

/** Start in-app checkout when signed in — falls back to web page. */
export async function startEarlySupporterCheckout(
  email: string,
  password: string,
): Promise<void> {
  trackSupporterEvent("supporter_purchase_started", { price_usd: 20 });
  try {
    const url = await invoke<string>("license_start_checkout", { email, password });
    await invoke("open_external_url", { url });
  } catch (err) {
    trackSupporterEvent("supporter_purchase_failed", {
      reason: String(err),
    });
    throw err;
  }
}

/** Re-verify license after web purchase (sign in + refresh JWT). */
export async function restorePurchases(
  email: string,
  password: string,
): Promise<SupporterStatus> {
  trackSupporterEvent("supporter_restore_started");
  await invoke<string>("license_sign_in_and_activate", { email, password });
  const status = await getSupporterStatus();
  if (status.isEarlySupporter) {
    trackSupporterEvent("supporter_purchase_completed", { restored: true });
  }
  return status;
}

export function checkoutConfigured(): boolean {
  return Boolean(SUPABASE_URL);
}
