import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PURCHASE_URL } from "../config";

type GateState = "checking" | "locked" | "unlocked";

/** Tauri AppError serializes as `{ Internal: "message" }`, not `{ message }`. */
function formatInvokeError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err && typeof err === "object") {
    const o = err as Record<string, unknown>;
    if (typeof o.message === "string") return o.message;
    const kind = Object.keys(o)[0];
    if (kind && typeof o[kind] === "string") return o[kind] as string;
  }
  return String(err);
}

export function LicenseGate({ children }: { children: React.ReactNode }) {
  const [state, setState] = useState<GateState>("checking");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [userId, setUserId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const check = await invoke<{ licensed: boolean; userId?: string }>("license_check_local");
        if (cancelled) return;
        if (check.licensed) {
          setUserId(check.userId ?? null);
          setState("unlocked");
        } else {
          setState("locked");
        }
      } catch {
        // No Tauri backend (e.g. browser preview during dev) — unlock so the
        // UI can be worked on outside the desktop shell. Never in prod builds.
        if (!cancelled) setState(import.meta.env.DEV ? "unlocked" : "locked");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleSignIn(e: React.FormEvent) {
    e.preventDefault();
    setError("");
    setBusy(true);
    try {
      const userId = await invoke<string>("license_sign_in_and_activate", {
        email: email.trim(),
        password,
      });
      setUserId(userId || null);
      setState("unlocked");
    } catch (err) {
      setError(formatInvokeError(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleSignOut() {
    setBusy(true);
    try {
      await invoke("license_clear_token");
      setUserId(null);
      setState("locked");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  if (state === "checking") {
    return (
      <div className="license-screen">
        <div className="license-card">
          <p className="muted">Checking license…</p>
        </div>
      </div>
    );
  }

  if (state === "unlocked") {
    return (
      <>
        {children}
        <button
          type="button"
          className="license-signout"
          onClick={handleSignOut}
          disabled={busy}
          title={userId ? `Licensed · ${userId}` : "Licensed"}
        >
          Sign out
        </button>
      </>
    );
  }

  return (
    <div className="license-screen">
      <div className="license-card">
        <div className="license-brand">
          <img src="/logo.png" alt="" className="license-logo" aria-hidden="true" />
          MeraRAW
        </div>
        <h1>Sign in to activate beta</h1>
        <p className="muted">
          Use your free{" "}
          <button
            type="button"
            className="license-link"
            onClick={() => void invoke("open_external_url", { url: PURCHASE_URL })}
          >
            meratech.co
          </button>{" "}
          account (sign up there first if you haven&apos;t). Activation checks online once,
          then works offline.
        </p>

        <form className="license-form" onSubmit={handleSignIn}>
          <label>
            <span className="muted sm">Email</span>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              autoComplete="email"
              required
            />
          </label>
          <label>
            <span className="muted sm">Password</span>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="current-password"
              required
            />
          </label>
          {error ? (
            <div className="license-error">
              {error}
              {error === "Load failed" ? (
                <p className="muted sm" style={{ marginTop: 8 }}>
                  This usually means an older build is installed. Quit the app, install the
                  latest DMG from{" "}
                  <code style={{ fontSize: 11 }}>release/MeraRAW Beta 0.1.0.dmg</code> after{" "}
                  <code style={{ fontSize: 11 }}>git pull && ./scripts/release-beta.sh</code>, or
                  run <code style={{ fontSize: 11 }}>npm run tauri dev</code> from the repo.
                </p>
              ) : null}
            </div>
          ) : null}
          <button type="submit" className="license-submit" disabled={busy}>
            {busy ? "Activating…" : "Sign in & activate"}
          </button>
        </form>

        <p className="muted sm license-foot">
          No account yet?{" "}
          <button
            type="button"
            className="license-link"
            onClick={() => void invoke("open_external_url", { url: PURCHASE_URL })}
          >
            Sign up free on meratech.co
          </button>
          <br />
          <span style={{ opacity: 0.6 }}>Build 0.1.0 · native sign-in</span>
        </p>
      </div>
    </div>
  );
}
