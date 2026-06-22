// Export as a modal dialog action (LR-style), out of the always-visible
// rail. Uses the existing export_image command.
import { useState } from "react";
import { exportImage } from "../../ipc/commands";

export function ExportDialog({ onClose }: { onClose: () => void }) {
  const [format, setFormat] = useState("jpeg");
  const [target, setTarget] = useState("srgb");
  const [maxDim, setMaxDim] = useState(2560);
  const [quality, setQuality] = useState(90);
  const [sharpen, setSharpen] = useState(30);
  const [stripMetadata, setStripMetadata] = useState(false);
  const [copyright, setCopyright] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // quality applies to the lossy encoders
  const lossy = format === "jpeg" || format === "heic";

  async function run() {
    setBusy(true);
    setStatus("exporting…");
    try {
      const path = await exportImage({
        format,
        target,
        quality,
        maxDim: maxDim || null,
        sharpen,
        destDir: "",
        stripMetadata,
        copyright: copyright.trim() ? copyright.trim() : null,
      });
      setStatus(`✓ ${path}`);
    } catch (e) {
      setStatus(`✗ ${typeof e === "object" ? JSON.stringify(e) : String(e)}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>Export</h2>
        <div className="modal-grid">
          <label>Format</label>
          <select value={format} onChange={(e) => setFormat(e.target.value)}>
            <option value="jpeg">JPEG</option>
            <option value="png">PNG</option>
            <option value="tiff16">TIFF 16-bit</option>
            <option value="heic">HEIC</option>
          </select>
          <label>Color space</label>
          <select value={target} onChange={(e) => setTarget(e.target.value)}>
            <option value="srgb">sRGB (web)</option>
            <option value="display-p3">Display P3</option>
            <option value="adobe-rgb">Adobe RGB (print)</option>
            <option value="prophoto">ProPhoto (wide gamut)</option>
          </select>
          <label>Long edge</label>
          <select
            value={maxDim}
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
            onChange={(e) => setSharpen(parseInt(e.target.value, 10))}
          />
          <label>Metadata</label>
          <label className="export-check">
            <input
              type="checkbox"
              checked={stripMetadata}
              onChange={(e) => setStripMetadata(e.target.checked)}
            />
            Strip EXIF (privacy)
          </label>
          <label>Copyright</label>
          <input
            type="text"
            value={copyright}
            disabled={stripMetadata}
            placeholder="© Your Name"
            onChange={(e) => setCopyright(e.target.value)}
          />
        </div>
        {status && (
          <div className="muted" style={{ userSelect: "text", marginTop: 8 }}>
            {status}
          </div>
        )}
        <div className="modal-actions">
          <button onClick={onClose}>Close</button>
          <button className="primary" disabled={busy} onClick={run}>
            Export
          </button>
        </div>
      </div>
    </div>
  );
}
