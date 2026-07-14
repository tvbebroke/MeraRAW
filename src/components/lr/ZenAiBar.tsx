// Zen-mode floating AI prompt — mic + text field over the viewport
// (Figma Develop Page - Zen / AI Chat).
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getDoc, snapshot } from "../../ipc/commands";
import { useDocStore } from "../../state/docStore";
import { Icon } from "./widgets";

const SNAP = "ai-pre-zen";

export function ZenAiBar() {
  const [available, setAvailable] = useState<boolean | null>(null);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [hint, setHint] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    invoke<boolean>("assistant_available")
      .then(setAvailable)
      .catch(() => setAvailable(false));
  }, []);

  async function run(message: string) {
    if (busy || !message.trim()) return;
    setBusy(true);
    setHint(null);
    await snapshot(SNAP).catch(() => {});
    try {
      const text = await invoke<string>("assistant_send", {
        message: message.trim(),
        mode: "edit",
      });
      const doc = await getDoc();
      if (doc) useDocStore.getState().setDoc(doc);
      setHint(text || "Applied.");
      setInput("");
    } catch (e) {
      const msg =
        typeof e === "object" && e !== null && "message" in e
          ? String((e as { message: unknown }).message)
          : String(e);
      setHint(msg);
    } finally {
      setBusy(false);
    }
  }

  function focusInput() {
    inputRef.current?.focus();
  }

  const disabled = available === false || busy;

  return (
    <div className="zen-ai-bar" role="search">
      <button
        type="button"
        className="zen-ai-mic"
        title={
          available === false
            ? "AI not available in this build"
            : "Focus AI prompt"
        }
        disabled={available === false}
        onClick={focusInput}
      >
        <Icon.Mic size={16} />
      </button>
      <input
        ref={inputRef}
        className="zen-ai-input"
        placeholder={
          available === false
            ? "AI Color Grader coming soon…"
            : busy
              ? "Working…"
              : "Describe the look…"
        }
        value={input}
        disabled={disabled}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && input.trim()) void run(input);
        }}
        aria-label="AI edit prompt"
      />
      {hint && (
        <div className="zen-ai-hint" title={hint}>
          {hint}
        </div>
      )}
    </div>
  );
}
