// frame:// transport helpers — shared by the viewport and the selftest rig.
const FRAME_BASE = "frame://localhost";

export function frameUrl(version: number, fmt?: "jpeg"): string {
  const q = fmt === "jpeg" ? "&fmt=jpeg" : "";
  return `${FRAME_BASE}/current?v=${version}${q}`;
}

/**
 * Prefetch a frame:// JPEG the same way filmstrip thumbs load thumb:// —
 * via <img>, not fetch. Custom-protocol fetch is cross-origin from the
 * Vite/dev page and used to fail CORS; <img> only needs img-src CSP.
 */
export function preloadFrame(version: number): Promise<string> {
  const url = frameUrl(version, "jpeg");
  return new Promise((resolve, reject) => {
    const probe = new Image();
    probe.onload = () => resolve(url);
    probe.onerror = () => reject(new Error("frame image load failed"));
    probe.src = url;
  });
}

export async function probeFrameTransport(retries = 5): Promise<boolean> {
  for (let attempt = 0; attempt < retries; attempt++) {
    try {
      await preloadFrame(0);
      return true;
    } catch {
      await new Promise((r) => setTimeout(r, 150 * (attempt + 1)));
    }
  }
  return false;
}
