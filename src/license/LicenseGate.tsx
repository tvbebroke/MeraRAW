import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { activateLicense, PURCHASE_URL, signIn, signOutAndClearLicense } from "./auth";
import { supabaseConfigured } from "../config";

type GateState = "checking" | "locked" | "unlocked";

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
      if (!supabaseConfigured()) {
        throw new Error("Supabase is not configured. Set VITE_SUPABASE_URL and VITE_SUPABASE_ANON_KEY.");
      }
      const session = await signIn(email.trim(), password);
      await activateLicense(session);
      setUserId(session.user.id);
      setState("unlocked");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleSignOut() {
    setBusy(true);
    try {
      await signOutAndClearLicense();
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
          <a href={PURCHASE_URL} target="_blank" rel="noreferrer">
            meratech.co
          </a>{" "}
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
          {error ? <div className="license-error">{error}</div> : null}
          <button type="submit" className="license-submit" disabled={busy}>
            {busy ? "Activating…" : "Sign in & activate"}
          </button>
        </form>

        <p className="muted sm license-foot">
          No account yet?{" "}
          <a href={PURCHASE_URL} target="_blank" rel="noreferrer">
            Sign up free on meratech.co
          </a>
        </p>
      </div>
    </div>
  );
}
