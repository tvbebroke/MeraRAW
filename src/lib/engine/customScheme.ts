/**
 * Base URL for a Tauri-registered custom URI scheme.
 *
 * macOS / Linux: `<scheme>://localhost/...`
 * Windows (WebView2) / Android: `http(s)://<scheme>.localhost/...`
 *
 * Detect via the page origin — Windows production loads from
 * `https://tauri.localhost` (or http), never `tauri://localhost`.
 */
export function customSchemeBase(scheme: string): string {
  if (
    typeof location !== "undefined" &&
    location.hostname.endsWith(".localhost")
  ) {
    const proto = location.protocol === "https:" ? "https:" : "http:";
    return `${proto}//${scheme}.localhost`;
  }
  return `${scheme}://localhost`;
}

/** `pathAndQuery` like `current?v=1` or `123?tier=t` (leading slash optional). */
export function customSchemeUrl(scheme: string, pathAndQuery: string): string {
  const path = pathAndQuery.startsWith("/") ? pathAndQuery : `/${pathAndQuery}`;
  return `${customSchemeBase(scheme)}${path}`;
}
