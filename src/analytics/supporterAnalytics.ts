/** Analytics for Early Supporter funnel — console + optional gtag. */

export type SupporterEvent =
  | "supporter_page_viewed"
  | "supporter_purchase_started"
  | "supporter_purchase_completed"
  | "supporter_purchase_failed"
  | "supporter_restore_started";

export function trackSupporterEvent(
  event: SupporterEvent,
  props?: Record<string, string | number | boolean>,
): void {
  const payload = { event, ts: Date.now(), ...props };
  if (import.meta.env.DEV) {
    console.debug("[meraraw-analytics]", payload);
  }
  const gtag = (window as unknown as { gtag?: (...args: unknown[]) => void }).gtag;
  if (typeof gtag === "function") {
    gtag("event", event, props ?? {});
  }
}
