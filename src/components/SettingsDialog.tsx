import { useEffect, useState } from "react";
import { getSupporterStatus, openEarlySupporterPage, type SupporterStatus } from "../services/purchaseService";
import { ViewportBgSettings } from "./ViewportBgPicker";

interface SettingsDialogProps {
  open: boolean;
  onClose: () => void;
  onSupportDevelopment: () => void;
}

export function SettingsDialog({
  open,
  onClose,
  onSupportDevelopment,
}: SettingsDialogProps) {
  const [status, setStatus] = useState<SupporterStatus | null>(null);

  useEffect(() => {
    if (!open) return;
    void getSupporterStatus().then(setStatus);
  }, [open]);

  if (!open) return null;

  return (
    <div className="modal-backdrop" role="presentation" onClick={onClose}>
      <div
        className="modal settings-modal"
        role="dialog"
        aria-labelledby="settings-title"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        <button type="button" className="modal-close" aria-label="Close" onClick={onClose}>
          ×
        </button>
        <h2 id="settings-title">Settings</h2>

        <section className="settings-section">
          <h3 className="settings-h">Account</h3>
          {status?.isEarlySupporter ? (
            <p className="muted sm">
              <span className="es-badge-app">Early Supporter</span> Lifetime license active.
            </p>
          ) : (
            <p className="muted sm">Free beta — all core editing features included.</p>
          )}
        </section>

        <ViewportBgSettings />

        <section className="settings-section">
          <h3 className="settings-h">Support</h3>
          <p className="muted sm">
            MeraRAW is independently built. Early Supporter licenses help fund development.
          </p>
          {!status?.isEarlySupporter && (
            <div className="modal-actions settings-actions">
              <button type="button" className="primary" onClick={onSupportDevelopment}>
                Support Development
              </button>
              <button
                type="button"
                className="ghost"
                onClick={() => openEarlySupporterPage("settings")}
              >
                View on meratech.co
              </button>
            </div>
          )}
        </section>

        <section className="settings-section">
          <h3 className="settings-h">About</h3>
          <p className="muted sm">
            MeraRAW — GPU-powered RAW editor with local AI grading.
          </p>
          <p className="muted xs">Version 0.1.4 (beta)</p>
        </section>

        <div className="modal-actions">
          <button type="button" className="ghost" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
