// Export as a modal dialog action (LR-style), out of the always-visible
// rail. Supports single open image or a queued library selection.
import { useEffect, useState } from "react";
import { exportImage, openImage, pickFolder, revealInFinder } from "../../ipc/commands";
import { onExportProgress } from "../../ipc/events";
import { formatAppError, type ExportSettings } from "../../ipc/types";
import { useUiStore } from "../../state/uiStore";

const SIZE_PRESETS: { label: string; value: number }[] = [
  { label: "Small (2048 px)", value: 2048 },
  { label: "Large (2560 px)", value: 2560 },
  { label: "Full (4096 px)", value: 4096 },
  { label: "Original (full resolution)", value: 0 },
];

const SHARPEN_PRESETS: { label: string; value: number }[] = [
  { label: "None", value: 0 },
  { label: "Screen", value: 25 },
  { label: "Matte paper", value: 40 },
  { label: "Glossy paper", value: 55 },
];

export function ExportDialog({
  onClose,
  queuePaths,
}: {
  onClose: () => void;
  /** When set, export each path sequentially (library batch export). */
  queuePaths?: string[];
}) {
  const decodeState = useUiStore((s) => s.decodeState);
  const lastOpenedPath = useUiStore((s) => s.lastOpenedPath);
  const batch = (queuePaths?.length ?? 0) > 0;
  const [format, setFormat] = useState<ExportSettings["format"]>("jpeg");
  const [target, setTarget] = useState<ExportSettings["target"]>("srgb");
  const [maxDim, setMaxDim] = useState(2560);
  const [quality, setQuality] = useState(90);
  const [sharpenPreset, setSharpenPreset] = useState(25);
  const [stripMetadata, setStripMetadata] = useState(false);
  const [copyright, setCopyright] = useState("");
  const [watermarkText, setWatermarkText] = useState("");
  const [destDir, setDestDir] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<{ phase: string; pct: number } | null>(
    null,
  );
  const [savedPaths, setSavedPaths] = useState<string[]>([]);

  const lossy = format === "jpeg" || format === "heic";
  const decodeReady = batch || decodeState === "ready";

  useEffect(() => {
    const un = onExportProgress((p) => {
      const pct = p.total > 0 ? Math.round((p.done / p.total) * 100) : 0;
      setProgress({ phase: p.phase, pct });
      if (p.phase === "render") {
        setStatus(`Rendering tiles ${p.done}/${p.total}…`);
      } else if (p.phase === "encode") {
        setStatus("Encoding…");
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

  function buildSettings(): ExportSettings {
    return {
      format,
      target,
      quality,
      maxDim: maxDim || null,
      sharpen: sharpenPreset,
      destDir: destDir ?? "",
      stripMetadata,
      copyright: copyright.trim() ? copyright.trim() : null,
      watermarkText: watermarkText.trim() ? watermarkText.trim() : null,
    };
  }

  async function run() {
    if (!decodeReady) {
      setStatus("Wait until the image is ready to export.");
      return;
    }

    setBusy(true);
    setSavedPaths([]);
    setProgress(null);
    setStatus(destDir ? "Starting export…" : "Choose a save folder…");
    const settings = buildSettings();
    const paths = batch ? queuePaths! : lastOpenedPath ? [lastOpenedPath] : [];
    if (paths.length === 0) {
      setStatus("No image to export.");
      setBusy(false);
      return;
    }

    const outputs: string[] = [];
    try {
      for (let i = 0; i < paths.length; i++) {
        const path = paths[i];
        if (batch || path !== lastOpenedPath) {
          setStatus(`Opening ${path.split("/").pop()} (${i + 1}/${paths.length})…`);
          await openImage(path);
          // Wait briefly for decode — export_image checks working texture
          for (let t = 0; t < 120; t++) {
            if (useUiStore.getState().decodeState === "ready") break;
            await new Promise((r) => setTimeout(r, 250));
          }
        }
        setStatus(`Exporting ${path.split("/").pop()} (${i + 1}/${paths.length})…`);
        const out = await exportImage(settings);
        outputs.push(out);
        if (!destDir) {
          const slash = out.lastIndexOf("/");
          if (slash > 0) setDestDir(out.slice(0, slash));
        }
      }
      setSavedPaths(outputs);
      setStatus(
        outputs.length === 1
          ? `Saved: ${outputs[0]}`
          : `Saved ${outputs.length} files to ${destDir ?? "export folder"}`,
      );
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
        <h2>{batch ? `Export ${queuePaths!.length} Photos` : "Export"}</h2>
        <p className="muted sm export-hint">
          Renders with your slider edits and camera profile (DCP). Metadata can be
          included or stripped for privacy.
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
          <label>Size</label>
          <select
            value={maxDim}
            disabled={busy}
            onChange={(e) => setMaxDim(parseInt(e.target.value, 10))}
          >
            {SIZE_PRESETS.map((p) => (
              <option key={p.value} value={p.value}>
                {p.label}
              </option>
            ))}
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
          <select
            value={sharpenPreset}
            disabled={busy}
            onChange={(e) => setSharpenPreset(parseInt(e.target.value, 10))}
          >
            {SHARPEN_PRESETS.map((p) => (
              <option key={p.label} value={p.value}>
                {p.label}
              </option>
            ))}
          </select>
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
          <label>Watermark</label>
          <input
            type="text"
            value={watermarkText}
            disabled={busy}
            placeholder="Optional text watermark"
            onChange={(e) => setWatermarkText(e.target.value)}
          />
        </div>
        {busy && progress && progress.phase !== "done" && (
          <div className="export-progress-bar">
            <div className="export-progress-fill" style={{ width: `${progress.pct}%` }} />
          </div>
        )}
        {!decodeReady && !busy && !batch && (
          <p className="export-status err">
            Image still decoding — export unlocks when status bar says “ready · export OK”.
          </p>
        )}
        {status && (
          <div
            className={`export-status ${status.startsWith("Saved") ? "ok" : status.startsWith("Export failed") ? "err" : "muted"}`}
          >
            {status}
          </div>
        )}
        <div className="modal-actions">
          {savedPaths.length === 1 && (
            <button type="button" className="tab" onClick={() => void revealInFinder(savedPaths[0])}>
              Reveal in Finder
            </button>
          )}
          <button disabled={busy} onClick={onClose}>
            Close
          </button>
          <button className="primary" disabled={busy || !decodeReady} onClick={() => void run()}>
            {busy ? "Exporting…" : batch ? `Export ${queuePaths!.length}` : "Export"}
          </button>
        </div>
      </div>
    </div>
  );
}
