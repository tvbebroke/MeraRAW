/** Analytics for Early Supporter funnel — routes through opt-in telemetry. */

import { capture } from "./telemetry";

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
  // `capture` handles consent, dev logging, and prop whitelisting. Props not
  // listed in telemetry.ts's ALLOWED_PROPS are dropped (warned about in dev).
  capture(event, props);
}
