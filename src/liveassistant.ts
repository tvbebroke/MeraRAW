// One-shot LIVE assistant test (real Claude API). Runs when
// MERATECH_LIVE_ASSISTANT is set: exports a "before", sends a real
// talk-to-edit prompt, captures the tool calls + reply, exports an
// "after", and reports a summary line test scripts grep.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getDoc, reportFrontendStatus } from "./ipc/commands";

const PROMPT =
  "Warm this up to a late-golden-hour feel, add gentle film contrast, and lift my subject off the background a little. Keep her skin natural.";

async function exportTo(dir: string): Promise<string> {
  return invoke<string>("export_image", {
    settings: {
      format: "jpeg",
      target: "srgb",
      quality: 90,
      maxDim: 1400,
      sharpen: 30,
      destDir: dir,
    },
  });
}

export async function runLiveAssistant(): Promise<void> {
  const report = (s: string) => reportFrontendStatus(s);
  const tools: string[] = [];
  const un = await listen<{ kind: string; label: string }>(
    "assistant-progress",
    (e) => {
      if (e.payload.kind === "tool") tools.push(e.payload.label);
    },
  );
  try {
    await report("live-assistant: starting");
    const before = await exportTo("/tmp/meratech-live/before");
    await report(`live-before: ${before}`);

    const t0 = Date.now();
    const reply = await invoke<string>("assistant_send", {
      message: PROMPT,
      mode: "edit",
    });
    const secs = ((Date.now() - t0) / 1000).toFixed(1);

    const doc = await getDoc();
    const modules = Object.keys(doc?.modules ?? {});
    const masks = (doc?.masks ?? []).map((m) => m.kind);

    const after = await exportTo("/tmp/meratech-live/after");

    // log each tool the assistant invoked
    for (const t of tools) await report(`live-tool: ${t}`);
    await report(`live-modules: ${modules.join(",") || "(none)"}`);
    await report(`live-masks: ${masks.join(",") || "(none)"}`);
    await report(`live-reply: ${reply.replace(/\s+/g, " ").slice(0, 400)}`);
    await report(`live-after: ${after}`);
    await report(`live-assistant-done: ${secs}s, ${tools.length} tool calls`);
  } catch (e) {
    await report(
      `live-assistant-fail: ${typeof e === "object" ? JSON.stringify(e) : String(e)}`,
    );
  } finally {
    un();
  }
}
