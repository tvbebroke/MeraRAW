/** Clip extensions. Stills (RAW + rendered) are everything else the catalog imports. */
export const VIDEO_EXTS = ["mp4", "mov", "m4v", "mkv", "webm", "m4a"] as const;

export function extOf(path: string): string {
  const base = path.replace(/\\/g, "/").split("/").pop() ?? path;
  const dot = base.lastIndexOf(".");
  if (dot < 0) return "";
  return base.slice(dot + 1).toLowerCase();
}

export function isVideoPath(path: string | null | undefined): boolean {
  if (!path) return false;
  return (VIDEO_EXTS as readonly string[]).includes(extOf(path));
}
