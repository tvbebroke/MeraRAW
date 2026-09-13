/** Tiny preset-list snapshots. Approximate the grade; not the full GPU path. */

export type PresetModules = Record<string, Record<string, unknown>>;

export const SNAP_W = 80;
export const SNAP_H = 50;

const cache = new Map<string, string>();
const inflight = new Map<string, Promise<string>>();

export function f32(
  modules: PresetModules | undefined,
  module: string,
  key: string,
  fallback = 0,
): number {
  const v = modules?.[module]?.[key];
  return typeof v === "number" && Number.isFinite(v) ? v : fallback;
}

function clamp01(v: number): number {
  return v < 0 ? 0 : v > 1 ? 1 : v;
}

function hueRgb(h: number): [number, number, number] {
  const x = (((h % 360) + 360) % 360) / 60;
  const c = 1;
  const m = 0;
  const i = Math.floor(x);
  const f = x - i;
  const p = m;
  const q = m + c * (1 - f);
  const t = m + c * f;
  const v = m + c;
  switch (i) {
    case 0:
      return [v, t, p];
    case 1:
      return [q, v, p];
    case 2:
      return [p, v, t];
    case 3:
      return [p, q, v];
    case 4:
      return [t, p, v];
    default:
      return [v, p, q];
  }
}

/** Apply develop-ish modules to sRGB bytes in place. */
export function applyPresetLook(
  data: Uint8ClampedArray,
  w: number,
  h: number,
  modules: PresetModules | undefined,
): void {
  const exp = 2 ** f32(modules, "exposure", "stops");
  const contrast = f32(modules, "tone_curve", "contrast") / 100;
  const highlights = f32(modules, "tone_curve", "highlights") / 100;
  const shadows = f32(modules, "tone_curve", "shadows") / 100;
  const sat = 1 + f32(modules, "color_grade", "perceptual_sat") / 100;
  const chroma = 1 + f32(modules, "color_grade", "global_chroma") / 100;
  const temp = f32(modules, "white_balance", "temp", 5500);
  const tint = f32(modules, "white_balance", "tint");
  const vignette = f32(modules, "effects", "vignette_amount") / 100;
  const tw = (temp - 5500) / 4000;
  const rGain = 1 + tw * 0.28;
  const bGain = 1 - tw * 0.28;
  const gGain = 1 - (tint / 150) * 0.12;
  const shHue = f32(modules, "color_grade", "shadows_hue");
  const shSat = f32(modules, "color_grade", "shadows_sat") / 100;
  const midHue = f32(modules, "color_grade", "midtones_hue");
  const midSat = f32(modules, "color_grade", "midtones_sat") / 100;
  const hiHue = f32(modules, "color_grade", "highlights_hue");
  const hiSat = f32(modules, "color_grade", "highlights_sat") / 100;
  const [shr, shg, shb] = hueRgb(shHue);
  const [mr, mg, mb] = hueRgb(midHue);
  const [hir, hig, hib] = hueRgb(hiHue);
  const bw = chroma <= 0.08;

  for (let i = 0; i < data.length; i += 4) {
    let r = (data[i] / 255) * exp * rGain;
    let g = (data[i + 1] / 255) * exp * gGain;
    let b = (data[i + 2] / 255) * exp * bGain;
    const k = 1 + contrast * 0.85;
    r = (r - 0.5) * k + 0.5;
    g = (g - 0.5) * k + 0.5;
    b = (b - 0.5) * k + 0.5;
    const y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    const shW = y < 0.5 ? 1 - y * 2 : 0;
    const hiW = y > 0.5 ? (y - 0.5) * 2 : 0;
    const lift = shadows * 0.22 * shW - highlights * 0.18 * hiW;
    r += lift;
    g += lift;
    b += lift;
    const satK = sat * chroma;
    r = y + (r - y) * satK;
    g = y + (g - y) * satK;
    b = y + (b - y) * satK;
    const midW = 1 - Math.abs(y - 0.5) * 2;
    r += shr * shSat * shW * 0.35 + mr * midSat * midW * 0.22 + hir * hiSat * hiW * 0.3;
    g += shg * shSat * shW * 0.35 + mg * midSat * midW * 0.22 + hig * hiSat * hiW * 0.3;
    b += shb * shSat * shW * 0.35 + mb * midSat * midW * 0.22 + hib * hiSat * hiW * 0.3;
    const yy = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    if (bw) {
      r = g = b = yy;
    }
    const px = ((i / 4) % w) / w - 0.5;
    const py = Math.floor(i / 4 / w) / h - 0.5;
    const d = Math.hypot(px, py) * 1.55;
    const vig = 1 - Math.max(0, vignette) * Math.max(0, d - 0.28);
    r *= vig;
    g *= vig;
    b *= vig;
    data[i] = Math.round(clamp01(r) * 255);
    data[i + 1] = Math.round(clamp01(g) * 255);
    data[i + 2] = Math.round(clamp01(b) * 255);
  }
}

/** Postcard: sky, ground, and a warm subject oval. */
export function paintBaseScene(data: Uint8ClampedArray, w: number, h: number): void {
  for (let y = 0; y < h; y++) {
    const v = y / h;
    for (let x = 0; x < w; x++) {
      const u = x / w;
      const i = (y * w + x) * 4;
      const dx = (u - 0.46) / 0.17;
      const dy = (v - 0.58) / 0.24;
      if (dx * dx + dy * dy < 1) {
        data[i] = 196;
        data[i + 1] = 146;
        data[i + 2] = 114;
      } else if (v < 0.44) {
        const t = v / 0.44;
        data[i] = Math.round(118 + t * 40);
        data[i + 1] = Math.round(168 - t * 20);
        data[i + 2] = Math.round(214 - t * 30);
      } else {
        const t = (v - 0.44) / 0.56;
        data[i] = Math.round(92 - t * 18);
        data[i + 1] = Math.round(118 - t * 28);
        data[i + 2] = Math.round(64 - t * 16);
      }
      data[i + 3] = 255;
    }
  }
}

function drawCover(
  ctx: CanvasRenderingContext2D,
  img: CanvasImageSource,
  iw: number,
  ih: number,
): void {
  const scale = Math.max(SNAP_W / iw, SNAP_H / ih);
  const dw = iw * scale;
  const dh = ih * scale;
  ctx.drawImage(img, (SNAP_W - dw) / 2, (SNAP_H - dh) / 2, dw, dh);
}

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    // Required so getImageData / toDataURL work when the thumb protocol
    // sends Access-Control-Allow-Origin (otherwise the canvas is tainted).
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("preset snapshot source failed"));
    img.src = url;
  });
}

function encodeSnapshot(
  ctx: CanvasRenderingContext2D,
  modules: PresetModules | undefined,
  usedPhoto: boolean,
): string {
  let pix: ImageData;
  try {
    pix = ctx.getImageData(0, 0, SNAP_W, SNAP_H);
  } catch {
    // Custom-scheme thumbs often taint the canvas in WKWebView even with
    // CORS headers — fall back to the postcard so the row still shows.
    pix = ctx.createImageData(SNAP_W, SNAP_H);
    paintBaseScene(pix.data, SNAP_W, SNAP_H);
    usedPhoto = false;
  }
  if (!usedPhoto) paintBaseScene(pix.data, SNAP_W, SNAP_H);
  applyPresetLook(pix.data, SNAP_W, SNAP_H, modules);
  ctx.putImageData(pix, 0, 0);
  try {
    return ctx.canvas.toDataURL("image/jpeg", 0.72);
  } catch {
    return ctx.canvas.toDataURL("image/png");
  }
}

/**
 * Graded postcard snapshot. Photo thumbs are best-effort — if they fail or
 * taint the canvas, we still return the postcard look.
 */
export async function renderPresetSnapshot(
  modules: PresetModules | undefined,
  photoUrl?: string | null,
): Promise<string> {
  const canvas = document.createElement("canvas");
  canvas.width = SNAP_W;
  canvas.height = SNAP_H;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return "";

  let usedPhoto = false;
  if (photoUrl) {
    try {
      const img = await loadImage(photoUrl);
      if (img.naturalWidth > 0) {
        drawCover(ctx, img, img.naturalWidth, img.naturalHeight);
        usedPhoto = true;
      }
    } catch {
      usedPhoto = false;
    }
  }

  return encodeSnapshot(ctx, modules, usedPhoto);
}

/** Sync postcard — always safe, no network. Use while photo upgrade loads. */
export function renderPostcardSnapshot(modules: PresetModules | undefined): string {
  const canvas = document.createElement("canvas");
  canvas.width = SNAP_W;
  canvas.height = SNAP_H;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return "";
  return encodeSnapshot(ctx, modules, false);
}

export function snapshotCacheKey(id: string, photoUrl: string | null): string {
  return `${id}::${photoUrl ?? ""}`;
}

export async function cachedPresetSnapshot(
  id: string,
  modules: PresetModules | undefined,
  photoUrl?: string | null,
): Promise<string> {
  const key = snapshotCacheKey(id, photoUrl ?? null);
  const hit = cache.get(key);
  if (hit) return hit;
  const pending = inflight.get(key);
  if (pending) return pending;

  const work = (async () => {
    try {
      // Prefer a postcard that always works; only attempt photo when asked.
      if (!photoUrl) {
        const postcard = renderPostcardSnapshot(modules);
        if (postcard) cache.set(key, postcard);
        return postcard;
      }
      const url = await renderPresetSnapshot(modules, photoUrl);
      if (url) cache.set(key, url);
      return url || renderPostcardSnapshot(modules);
    } catch {
      const postcard = renderPostcardSnapshot(modules);
      if (postcard) cache.set(key, postcard);
      return postcard;
    } finally {
      inflight.delete(key);
    }
  })();

  inflight.set(key, work);
  return work;
}
