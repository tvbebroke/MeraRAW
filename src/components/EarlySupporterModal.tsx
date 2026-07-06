import { useCallback, useEffect, useState } from "react";
import { trackSupporterEvent } from "../analytics/supporterAnalytics";
import {
  getSupporterStatus,
  openEarlySupporterPage,
  restorePurchases,
  startEarlySupporterCheckout,
  type SupporterStatus,
} from "../services/purchaseService";

const PRICE = "$20";

function formatInvokeError(err: unknown): string {
  if (typeof err === "object" && err !== null && "Internal" in err) {
    return String((err as { Internal: string }).Internal);
  }
  return String(err);
}

interface EarlySupporterModalProps {
  open: boolean;
  onClose: () => void;
  /** When true, user already purchased — show badge only. */
  isSupporter?: boolean;
}

export function EarlySupporterModal({
  open,
  onClose,
  isSupporter = false,
}: EarlySupporterModalProps) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [status, setStatus] = useState<SupporterStatus | null>(null);

  useEffect(() => {
    if (!open) return;
    trackSupporterEvent("supporter_page_viewed", { surface: "modal" });
    void getSupporterStatus().then(setStatus);
  }, [open]);

  const supporter = isSupporter || status?.isEarlySupporter;

  const onPurchase = useCallback(async () => {
    setError("");
    setBusy(true);
    try {
      if (email.trim() && password) {
        await startEarlySupporterCheckout(email.trim(), password);
      } else {
        openEarlySupporterPage("modal_fallback");
      }
    } catch (err) {
      setError(formatInvokeError(err));
    } finally {
      setBusy(false);
    }
  }, [email, password]);

  const onRestore = useCallback(async () => {
    setError("");
    if (!email.trim() || !password) {
      setError("Enter the email and password from your meratech.co account.");
      return;
    }
    setBusy(true);
    try {
      const next = await restorePurchases(email.trim(), password);
      setStatus(next);
      if (!next.isEarlySupporter) {
        setError("No Early Supporter purchase found for this account yet.");
      }
    } catch (err) {
      setError(formatInvokeError(err));
    } finally {
      setBusy(false);
    }
  }, [email, password]);

  if (!open) return null;

  return (
    <div
      className="modal-backdrop"
      role="presentation"
      onClick={onClose}
    >
      <div
        className="modal es-modal"
        role="dialog"
        aria-labelledby="es-modal-title"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        <button type="button" className="modal-close" aria-label="Close" onClick={onClose}>
          ×
        </button>
        <span className="eyebrow sm">Support development</span>
        <h2 id="es-modal-title">Become an Early Supporter</h2>
        <p className="muted sm es-modal-lede">
          MeraRAW is in active development. The free version stays fully functional — open, edit,
          and export without limits. Purchase a lifetime license to support future improvements.
        </p>

        {supporter ? (
          <div className="es-modal-supporter">
            <span className="es-badge-app">Early Supporter</span>
            <p className="muted sm">Thank you — lifetime access is active on your account.</p>
            <div className="modal-actions">
              <button type="button" className="primary" onClick={onClose}>
                Done
              </button>
            </div>
          </div>
        ) : (
          <>
            <ul className="es-benefits-app">
              <li>Lifetime access to Early Supporter premium features</li>
              <li>Fund ongoing development and improvements</li>
              <li>Lowest price we&apos;ll ever offer — {PRICE}</li>
            </ul>
            <p className="es-pricing-notice-app">
              Early supporter pricing is temporary and may increase as the product grows and new
              features are added.
            </p>
            <div className="license-form es-modal-form">
              <label className="muted xs" htmlFor="es-email">
                Account email (optional — for in-app checkout)
              </label>
              <input
                id="es-email"
                type="email"
                autoComplete="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="Same as meratech.co"
              />
              <input
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="Password"
              />
            </div>
            {error && <p className="license-error">{error}</p>}
            <div className="modal-actions es-modal-actions">
              <button type="button" className="primary" disabled={busy} onClick={() => void onPurchase()}>
                {busy ? "Opening checkout…" : `Get Lifetime Access — ${PRICE}`}
              </button>
              <button type="button" className="ghost" disabled={busy} onClick={onClose}>
                Continue Using Free Version
              </button>
            </div>
            <button
              type="button"
              className="ghost es-restore-link"
              disabled={busy}
              onClick={() => void onRestore()}
            >
              Restore purchase
            </button>
          </>
        )}
      </div>
    </div>
  );
}
