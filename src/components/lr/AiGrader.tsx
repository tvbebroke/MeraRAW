// AI Color Grader — the assistant as a native LR panel (the one
// intentional departure from Lightroom). Chips + streaming activity +
// streaming text + Accept/Revert via snapshot. Everything still lands as
// normal guard-walled, undoable ops; this panel only displays + offers a
// snapshot-based one-click revert of the whole AI batch.
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  getDoc,
  restoreSnapshot,
  setPreviewBypass,
  snapshot,
} from "../../ipc/commands";
import { useDocStore } from "../../state/docStore";
import { Icon } from "./widgets";

const CHIPS = [
  "Warmer",
  "Cooler",
  "Teal & orange",
  "Lift shadows",
  "Filmic contrast",
  "Fix skin",
  "Moody",
  "Bright & airy",
];

interface Activity {
  kind: "tool" | "text";
  text: string;
}

const SNAP = "ai-pre";

export function AiGrader() {
  const [available, setAvailable] = useState<boolean | null>(null);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [activity, setActivity] = useState<Activity[]>([]);
  const [reply, setReply] = useState<string | null>(null);
  const [pending, setPending] = useState(false); // a finished run awaiting accept/revert
  const [comparing, setComparing] = useState(false);
  const logRef = useRef<HTMLDivElement>(null);
  const reconcile = useDocStore((s) => s.reconcile);

  useEffect(() => {
    invoke<boolean>("assistant_available")
      .then(setAvailable)
      .catch(() => setAvailable(false));
    const un = listen<{ kind: string; label: string }>(
      "assistant-progress",
      (e) => {
        const { kind, label } = e.payload;
        if (kind === "tool") setActivity((a) => [...a, { kind: "tool", text: label }]);
        else if (kind === "text" && label.trim())
          setActivity((a) => [...a, { kind: "text", text: label }]);
      },
    );
    return () => {
      un.then((f) => f());
    };
  }, []);

  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [activity, reply]);

  async function run(message: string, mode: "edit" | "auto" | "explain") {
    if (busy) return;
    setBusy(true);
    setActivity([]);
    setReply(null);
    setPending(false);
    // snapshot BEFORE the run so we can one-click revert the whole batch
    if (mode !== "explain") await snapshot(SNAP).catch(() => {});
    try {
      const text = await invoke<string>("assistant_send", { message, mode });
      setReply(text);
      if (mode !== "explain") {
        const doc = await getDoc();
        if (doc) useDocStore.getState().setDoc(doc);
        setPending(true);
      }
    } catch (e) {
      const msg =
        typeof e === "object" && e !== null && "message" in e
          ? String((e as { message: unknown }).message)
          : String(e);
      setReply(
        /rate limit|429/i.test(msg)
          ? "⏳ Rate limited — your plan allows 30k input tokens/min. Wait a minute, or add credits."
          : `⚠ ${msg}`,
      );
    } finally {
      setBusy(false);
    }
  }

  async function revert() {
    try {
      reconcile(await restoreSnapshot(SNAP));
    } catch {
      /* no snapshot */
    }
    setPending(false);
    setComparing(false);
    await setPreviewBypass(false).catch(() => {});
  }
  function accept() {
    setPending(false);
    setComparing(false);
    void setPreviewBypass(false).catch(() => {});
  }
  async function toggleCompare() {
    const next = !comparing;
    setComparing(next);
    // "before" = the un-edited base via the engine bypass
    await setPreviewBypass(next).catch(() => {});
  }

  // Not in the beta yet. Keep it warm but clearly "coming soon".
  if (available === false) {
    return (
      <div className="ai-soon">
        <div className="ai-soon-title">
          <Icon.Sparkles size={13} /> Not available yet
        </div>
        <div className="muted sm">
          The AI Color Grader isn&apos;t available in the beta yet — I&apos;m
          still figuring this one out. Coming soon.
        </div>
      </div>
    );
  }

  return (
    <div className="ai-grader">
      <div className="ai-chips">
        {CHIPS.map((c) => (
          <button
            key={c}
            className="ai-chip"
            disabled={busy}
            onClick={() => void run(c, "edit")}
          >
            {c}
          </button>
        ))}
      </div>
      <div className="ai-input-row">
        <input
          className="ai-input"
          placeholder="Describe the look…"
          value={input}
          disabled={busy}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && input.trim()) {
              void run(input.trim(), "edit");
              setInput("");
            }
          }}
        />
        <button
          className="ai-go"
          disabled={busy || !input.trim()}
          title="Apply"
          onClick={() => {
            void run(input.trim(), "edit");
            setInput("");
          }}
        >
          <Icon.Sparkles size={14} />
        </button>
      </div>
      <div className="ai-buttons">
        <button disabled={busy} onClick={() => void run("auto", "auto")}>
          <Icon.Wand size={12} /> Auto
        </button>
        <button
          disabled={busy || !input.trim()}
          onClick={() => {
            void run(input.trim(), "explain");
            setInput("");
          }}
          title="Diagnose only — no edits"
        >
          Explain
        </button>
      </div>

      {(activity.length > 0 || reply || busy) && (
        <div className="ai-log" ref={logRef}>
          {activity.map((a, i) => (
            <div key={i} className={`ai-act ${a.kind}`}>
              {a.kind === "tool" ? <Icon.Sparkles size={11} /> : null} {a.text}
            </div>
          ))}
          {busy && <div className="ai-act muted">working…</div>}
          {reply && <div className="ai-reply">{reply}</div>}
        </div>
      )}

      {pending && (
        <div className="ai-verdict">
          <button className="ai-accept" onClick={accept}>
            Keep
          </button>
          <button className="ai-revert" onClick={() => void revert()}>
            Revert
          </button>
          <button
            className={comparing ? "active" : ""}
            onClick={() => void toggleCompare()}
            title="Hold to see before"
          >
            <Icon.Compare size={12} /> {comparing ? "Before" : "After"}
          </button>
        </div>
      )}
    </div>
  );
}
