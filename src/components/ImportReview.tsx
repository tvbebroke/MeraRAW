// Review-for-import screen (doc 05): scan → toggle selection → confirm import.
import { useCallback, useEffect, useMemo, useState } from "react";
import { importSelected, scanImportFolder } from "../ipc/commands";
import { formatAppError, type ImportCandidate } from "../ipc/types";

export function ImportReview({
  root,
  onClose,
  onDone,
}: {
  root: string;
  onClose: () => void;
  onDone: (root: string) => void;
}) {
  const [candidates, setCandidates] = useState<
    (ImportCandidate & { selected: boolean })[]
  >([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [anchor, setAnchor] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    scanImportFolder(root)
      .then((list) => {
        if (cancelled) return;
        setCandidates(
          list.map((c) => ({ ...c, selected: c.isNew })),
        );
      })
      .catch((e) => {
        if (!cancelled) setError(formatAppError(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [root]);

  const selectedCount = useMemo(
    () => candidates.filter((c) => c.selected).length,
    [candidates],
  );

  const toggleAt = useCallback((idx: number, extend: boolean) => {
    setCandidates((prev) => {
      const next = [...prev];
      if (extend && anchor !== null && anchor >= 0 && anchor < prev.length) {
        const lo = Math.min(anchor, idx);
        const hi = Math.max(anchor, idx);
        const target = !prev[idx].selected;
        for (let i = lo; i <= hi; i++) next[i] = { ...next[i], selected: target };
      } else {
        next[idx] = { ...next[idx], selected: !next[idx].selected };
      }
      return next;
    });
    setAnchor(idx);
  }, [anchor]);

  function selectAll(on: boolean) {
    setCandidates((prev) => prev.map((c) => ({ ...c, selected: on })));
  }

  async function confirm() {
    const paths = candidates.filter((c) => c.selected).map((c) => c.path);
    if (paths.length === 0) {
      setError("Select at least one photo.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await importSelected(root, paths);
      onDone(root);
      onClose();
    } catch (e) {
      setError(formatAppError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={busy ? undefined : onClose}>
      <div className="modal import-review-modal" onClick={(e) => e.stopPropagation()}>
        <h2>Add Photos</h2>
        <p className="muted sm">
          Review photos from <code>{root}</code>. Selected files are indexed into your
          library (originals stay in place). Already-imported files are deselected by
          default.
        </p>
        <div className="import-review-toolbar">
          <button type="button" disabled={busy || loading} onClick={() => selectAll(true)}>
            Select all
          </button>
          <button type="button" disabled={busy || loading} onClick={() => selectAll(false)}>
            Select none
          </button>
          <span className="muted">
            {selectedCount} of {candidates.length} selected
          </span>
        </div>
        {loading ? (
          <p className="muted">Scanning folder…</p>
        ) : (
          <div className="import-review-grid">
            {candidates.map((c, idx) => (
              <button
                key={c.path}
                type="button"
                className={`import-review-cell ${c.selected ? "selected" : ""}`}
                onClick={(e) => toggleAt(idx, e.shiftKey)}
                title={c.path}
              >
                <span className="import-review-check">{c.selected ? "✓" : ""}</span>
                <span className="import-review-name">{c.filename}</span>
                {!c.isNew && <span className="import-review-tag">imported</span>}
              </button>
            ))}
            {candidates.length === 0 && (
              <p className="muted">No supported photos found in this folder.</p>
            )}
          </div>
        )}
        {error && <p className="export-status err">{error}</p>}
        <div className="modal-actions">
          <button disabled={busy} onClick={onClose}>
            Cancel
          </button>
          <button
            className="primary"
            disabled={busy || loading || selectedCount === 0}
            onClick={() => void confirm()}
          >
            {busy ? "Importing…" : `Import ${selectedCount} photo${selectedCount === 1 ? "" : "s"}`}
          </button>
        </div>
      </div>
    </div>
  );
}
