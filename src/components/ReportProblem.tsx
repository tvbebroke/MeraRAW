// Bottom-left "Report a problem / talk to the creator". Opens a small form;
// on send, the backend hands a pre-filled mailto: to the OS mail client so the
// message lands in the creator's inbox. No server, no third-party service.
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export function ReportProblem() {
  const [open, setOpen] = useState(false);
  const [msg, setMsg] = useState("");
  const [from, setFrom] = useState("");
  const [busy, setBusy] = useState(false);
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function close() {
    setOpen(false);
    setError(null);
  }

  async function send() {
    if (!msg.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      await invoke("report_problem", {
        message: msg.trim(),
        from: from.trim() || null,
      });
      setSent(true);
      setMsg("");
    } catch (e) {
      setError(
        typeof e === "object" && e !== null && "message" in e
          ? String((e as { message: unknown }).message)
          : String(e),
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <button
        className="report-fab"
        title="Report a problem or talk to the creator"
        onClick={() => {
          setOpen(true);
          setSent(false);
          setError(null);
        }}
      >
        💬 Report a problem
      </button>

      {open && (
        <div className="modal-backdrop" onClick={close}>
          <div className="modal report-modal" onClick={(e) => e.stopPropagation()}>
            <h2>Talk to the creator</h2>
            {sent ? (
              <>
                <p className="muted sm">
                  Thanks! Your mail app should have opened with the message
                  ready — just hit send and it lands in my inbox.
                </p>
                <div className="modal-actions">
                  <button className="primary" onClick={close}>
                    Done
                  </button>
                </div>
              </>
            ) : (
              <>
                <p className="muted sm">
                  Hit a bug or have feedback? Send it straight to me. This opens
                  your mail app with the message ready to send.
                </p>
                <p className="report-note sm">
                  MeraRAW is an early beta from a solo dev who loves color
                  grading — the full version is coming soon, and your notes
                  genuinely help shape it. 🙏
                </p>
                <textarea
                  className="report-text"
                  placeholder="What happened? Or just say hi 👋"
                  value={msg}
                  autoFocus
                  onChange={(e) => setMsg(e.target.value)}
                />
                <input
                  type="text"
                  className="report-from"
                  placeholder="Your email (optional, so I can reply)"
                  value={from}
                  onChange={(e) => setFrom(e.target.value)}
                />
                {error && <div className="report-error">{error}</div>}
                <div className="modal-actions">
                  <button onClick={close}>Cancel</button>
                  <button
                    className="primary"
                    disabled={busy || !msg.trim()}
                    onClick={() => void send()}
                  >
                    {busy ? "Opening…" : "Send"}
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </>
  );
}
