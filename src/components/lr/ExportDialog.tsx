// Export as a modal dialog action (LR-style), out of the always-visible
// rail. Uses the existing export_image command.
import { useEffect, useState } from "react";
import { exportImage, pickFolder, revealInFinder } from "../../ipc/commands";
import { onExportProgress } from "../../ipc/events";
import { formatAppError, type ExportSettings } from "../../ipc/types";
import { useUiStore } from "../../state/uiStore";

export function ExportDialog({ onClose }: { onClose: () => void }) {
  const decodeState = useUiStore((s) => s.decodeState);
  const [format, setFormat] = useState<ExportSettings["format"]>("jpeg");
  const [target, setTarget] = useState<ExportSettings["target"]>("srgb");
  const [maxDim, setMaxDim] = useState(2560);
  const [quality, setQuality] = useState(90);
  const [sharpen, setSharpen] = useState(30);
  const [stripMetadata, setStripMetadata] = useState(false);
  const [copyright, setCopyright] = useState("");
  const [destDir, setDestDir] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<{ phase: string; pct: number } | null>(
    null,
  );
  const [savedPath, setSavedPath] = useState<string | null>(null);

  const lossy = format === "jpeg" || format === "heic";
  const decodeReady = decodeState === "ready";

  useEffect(() => {
    const un = onExportProgress((p) => {
      const pct = p.total > 0 ? Math.round((p.done / p.total) * 100) : 0;
      setProgress({ phase: p.phase, pct });
      if (p.phase === "render") {
        setStatus(`Rendering tiles ${p.done}/${p.total}…`);
      } else if (p.phase === "encode") {
        setStatus("Encoding JPEG…");
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  async function chooseFolder() {
    const folder = await pickFolder();
    if (folder) {
      setDestDir(folder);
      setStatus(null);
    }
  }

  async function run() {
    if (!decodeReady) {
      setStatus("Wait until the status bar shows “ready · export OK”.");
      return;
    }

    setBusy(true);
    setSavedPath(null);
    setProgress(null);
    setStatus(
      destDir
        ? "Starting export…"
        : "Choose a save folder…",
    );
    try {
      const path = await exportImage({
        format,
        target,
        quality,
        maxDim: maxDim || null,
        sharpen,
        destDir: destDir ?? "",
        stripMetadata,
        copyright: copyright.trim() ? copyright.trim() : null,
      });
      if (!destDir) {
        const slash = path.lastIndexOf("/");
        if (slash > 0) setDestDir(path.slice(0, slash));
      }
      setSavedPath(path);
      setStatus(`Saved: ${path}`);
      setProgress({ phase: "done", pct: 100 });
    } catch (e) {
      setStatus(`Export failed: ${formatAppError(e)}`);
      setProgress(null);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={busy ? undefined : onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>Export</h2>
        <p className="muted sm export-hint">
          Renders the open RAW with your slider edits and camera profile (DCP). Edits load from{" "}
          <code>.mrt.json</code> or, if missing, Adobe Lightroom <code>.xmp</code> next to the file.
        </p>
        <div className="modal-grid">
          <label>Save to</label>
          <div className="export-dest-row">
            <span className="export-dest-path muted">
              {destDir ?? "Pick on Export (or Choose… first)"}
            </span>
            <button type="button" className="tab" disabled={busy} onClick={() => void chooseFolder()}>
              Choose…
            </button>
          </div>
          <label>Format</label>
          <select
            value={format}
            disabled={busy}
            onChange={(e) => setFormat(e.target.value as ExportSettings["format"])}
          >
            <option value="jpeg">JPEG</option>
            <option value="png">PNG</option>
            <option value="tiff16">TIFF 16-bit</option>
            <option value="heic">HEIC</option>
          </select>
          <label>Color space</label>
          <select
            value={target}
            disabled={busy}
            onChange={(e) => setTarget(e.target.value as ExportSettings["target"])}
          >
            <option value="srgb">sRGB (web)</option>
            <option value="display-p3">Display P3</option>
            <option value="adobe-rgb">Adobe RGB (print)</option>
            <option value="prophoto">ProPhoto (wide gamut)</option>
          </select>
          <label>Long edge</label>
          <select
            value={maxDim}
            disabled={busy}
            onChange={(e) => setMaxDim(parseInt(e.target.value, 10))}
          >
            <option value={2048}>2048 px</option>
            <option value={2560}>2560 px</option>
            <option value={4096}>4096 px</option>
            <option value={0}>Full resolution</option>
          </select>
          {lossy && (
            <>
              <label>Quality</label>
              <input
                type="range"
                min={50}
                max={100}
                value={quality}
                disabled={busy}
                onChange={(e) => setQuality(parseInt(e.target.value, 10))}
              />
            </>
          )}
          <label>Output sharpen</label>
          <input
            type="range"
            min={0}
            max={100}
            value={sharpen}
            disabled={busy}
            onChange={(e) => setSharpen(parseInt(e.target.value, 10))}
          />
          <label>Metadata</label>
          <label className="export-check">
            <input
              type="checkbox"
              checked={stripMetadata}
              disabled={busy}
              onChange={(e) => setStripMetadata(e.target.checked)}
            />
            Strip EXIF (privacy)
          </label>
          <label>Copyright</label>
          <input
            type="text"
            value={copyright}
            disabled={stripMetadata || busy}
            placeholder="© Your Name"
            onChange={(e) => setCopyright(e.target.value)}
          />
        </div>
        {busy && progress && progress.phase !== "done" && (
          <div className="export-progress-bar">
            <div className="export-progress-fill" style={{ width: `${progress.pct}%` }} />
          </div>
        )}
        {!decodeReady && !busy && (
          <p className="export-status err">
            Image still decoding — export unlocks when status bar says “ready · export OK”.
          </p>
        )}
        {status && (
          <div
            className={`export-status ${status.startsWith("Saved:") ? "ok" : status.startsWith("Export failed") ? "err" : "muted"}`}
          >
            {status}
          </div>
        )}
        <div className="modal-actions">
          {savedPath && (
            <button type="button" className="tab" onClick={() => void revealInFinder(savedPath)}>
              Reveal in Finder
            </button>
          )}
          <button disabled={busy} onClick={onClose}>
            Close
          </button>
          <button className="primary" disabled={busy || !decodeReady} onClick={() => void run()}>
            {busy ? "Exporting…" : "Export"}
          </button>
        </div>
      </div>
    </div>
  );
}
