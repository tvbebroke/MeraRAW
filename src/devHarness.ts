// Env-gated dev automation — single entry from App.tsx after image-ready.
import { reportFrontendStatus } from "./ipc/commands";

export async function runDevHarness(imageVersion: number): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  if (await invoke<boolean>("selftest_enabled")) {
    const { runSelfTest } = await import("./selftest");
    void runSelfTest(imageVersion);
    return;
  }
  if (await invoke<boolean>("live_assistant_enabled")) {
    const { runLiveAssistant } = await import("./liveassistant");
    void runLiveAssistant();
    return;
  }
  if (await invoke<boolean>("verify_slider_enabled").catch(() => false)) {
    const { verifySlider } = await import("./verifyslider");
    void verifySlider();
    return;
  }
  await reportFrontendStatus("dev-harness: idle").catch(() => {});
}
