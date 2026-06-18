// One-shot slider→preview verification (dev hook, MERATECH_VERIFY_SLIDER).
// Reads the ACTUAL viewport canvas pixels before/after a setParam, proving
// the visible preview redraws on an edit — the exact chain a slider uses.
import { reportFrontendStatus, setParam } from "./ipc/commands";
import { onFrameReady } from "./ipc/events";

function canvasCenter(): [number, number, number] | null {
  const c = document.querySelector(".viewport-canvas") as HTMLCanvasElement | null;
  if (!c || !c.width) return null;
  const ctx = c.getContext("2d");
  if (!ctx) return null;
  const x = Math.floor(c.width / 2);
  const y = Math.floor(c.height / 2);
  const d = ctx.getImageData(x, y, 1, 1).data;
  return [d[0], d[1], d[2]];
}

function waitFrame(timeout = 4000): Promise<void> {
  return new Promise((resolve) => {
    const t = setTimeout(resolve, timeout);
    const un = onFrameReady(() => {
      clearTimeout(t);
      un.then((f) => f());
      // let the canvas finish putImageData
      setTimeout(resolve, 120);
    });
  });
}

export async function verifySlider(): Promise<void> {
  try {
    const before = canvasCenter();
    if (!before) {
      await reportFrontendStatus("slider-verify: no canvas");
      return;
    }
    // exactly what moving the Exposure slider does
    await setParam("exposure.stops", 1.8);
    await waitFrame();
    const after = canvasCenter();
    if (!after) {
      await reportFrontendStatus("slider-verify: canvas gone");
      return;
    }
    const delta =
      Math.abs(after[0] - before[0]) +
      Math.abs(after[1] - before[1]) +
      Math.abs(after[2] - before[2]);
    await reportFrontendStatus(
      `slider-verify: canvas before=[${before}] after=[${after}] delta=${delta} reflected=${delta > 10}`,
    );
    // restore
    await setParam("exposure.stops", 0);
  } catch (e) {
    await reportFrontendStatus(`slider-verify-error: ${String(e)}`);
  }
}
