import { atom, computed } from "nanostores";
import {
  onDecodeError,
  onDenoiseDone,
  onDenoiseError,
  onDenoiseProgress,
  onEngineCrashed,
  onExportBatchDone,
  onExportBatchProgress,
  onExportProgress,
  onImportDone,
  onImportProgress,
  onRetouchDone,
  onRetouchError,
} from "../ipc/events";

/**
 * Unified background-job registry.
 *
 * MeraRAW emits seven kinds of long-running work — import, decode, denoise,
 * retouch, mask, export, batch export — but before this store only two of
 * them were visible anywhere, and both only while a particular panel or modal
 * happened to be open. Close the export dialog mid-batch, or import 200 RAWs,
 * and the app looked idle while it ground away. Failures were worse: a decode
 * error or an engine crash surfaced nowhere at all.
 *
 * Everything now funnels here, and the title bar shows one pill.
 */

export type JobStatus = "queued" | "running" | "ok" | "failed";

export interface Job {
  id: string;
  label: string;
  /** Short prose phase ("Encoding…"). Rendered in the sans face. */
  phase?: string;
  /** Numeric detail ("42% · tile 3/16"). Rendered mono + tabular. */
  detail?: string;
  /** 0–100, or null for indeterminate work. */
  pct?: number | null;
  status: JobStatus;
  error?: string;
  /** Present only when the job can genuinely be re-run. */
  retry?: () => void;
  at: number;
}

export const jobs = atom<Job[]>([]);

export const activeJobs = computed(jobs, (list) =>
  list.filter((j) => j.status === "running" || j.status === "queued"),
);
export const failedJobs = computed(jobs, (list) =>
  list.filter((j) => j.status === "failed"),
);

/** Newest first, capped so a long session can't grow without bound. */
const MAX_JOBS = 40;

export function upsertJob(patch: Partial<Job> & { id: string }): void {
  const list = jobs.get();
  const i = list.findIndex((j) => j.id === patch.id);
  if (i === -1) {
    const next: Job = {
      label: patch.id,
      status: "running",
      at: Date.now(),
      ...patch,
    };
    jobs.set([next, ...list].slice(0, MAX_JOBS));
  } else {
    const next = [...list];
    next[i] = { ...next[i], ...patch };
    jobs.set(next);
  }
}

export function failJob(id: string, label: string, error: string, retry?: () => void): void {
  upsertJob({ id, label, status: "failed", error, retry, pct: null, phase: undefined });
}

export function dismissJob(id: string): void {
  jobs.set(jobs.get().filter((j) => j.id !== id));
}

export function clearFinished(): void {
  jobs.set(jobs.get().filter((j) => j.status === "running" || j.status === "queued"));
}

// Dev-only handle so the jobs UI can be exercised without a running backend.
// `import.meta.env.DEV` is false in production builds, so this is stripped.
if (import.meta.env.DEV && typeof window !== "undefined") {
  (window as any).__jobs = { jobs, upsertJob, failJob, dismissJob, clearFinished };
}

/**
 * Subscribe to every backend job event. Returns a teardown.
 * Safe to call once at app start.
 */
export function initJobsBridge(): () => void {
  const pending: Promise<() => void>[] = [];
  const track = (p: Promise<() => void>) => pending.push(p);

  // ── import ──────────────────────────────────────────────────────────
  track(
    onImportProgress(({ done, total }) => {
      upsertJob({
        id: "import",
        label: "Importing photos",
        status: "running",
        pct: total > 0 ? Math.round((done / total) * 100) : null,
        detail: `${done}/${total}`,
        phase: "Reading files",
      });
    }),
  );
  track(
    onImportDone((total) => {
      upsertJob({
        id: "import",
        label: "Import complete",
        status: "ok",
        pct: 100,
        detail: `${total} photo${total === 1 ? "" : "s"}`,
        phase: undefined,
      });
    }),
  );

  // ── AI denoise ──────────────────────────────────────────────────────
  track(
    onDenoiseProgress((p) => {
      upsertJob({
        id: `denoise:${p.job}`,
        label: "AI denoise",
        status: "running",
        pct: Math.round(p.pct),
        detail: `${Math.round(p.pct)}% · tile ${p.tile}/${p.tiles}`,
        phase: "Denoising",
      });
    }),
  );
  track(
    onDenoiseDone((job) => {
      upsertJob({
        id: `denoise:${job}`,
        label: "AI denoise",
        status: "ok",
        pct: 100,
        detail: undefined,
        phase: undefined,
      });
    }),
  );
  track(
    onDenoiseError(({ job, message }) => failJob(`denoise:${job}`, "AI denoise", message)),
  );

  // ── retouch ─────────────────────────────────────────────────────────
  track(onRetouchDone(() => upsertJob({ id: "retouch", label: "Retouch", status: "ok", pct: 100 })));
  track(onRetouchError((message) => failJob("retouch", "Retouch", message)));

  // ── export (single + batch) ─────────────────────────────────────────
  track(
    onExportProgress((p) => {
      upsertJob({
        id: "export",
        label: "Exporting photo",
        status: "running",
        pct: p.total > 0 ? Math.round((p.done / p.total) * 100) : null,
        detail: p.total > 0 ? `${p.done}/${p.total}` : undefined,
        phase: p.phase === "render" ? "Rendering tiles" : "Encoding",
      });
    }),
  );
  track(
    onExportBatchProgress((p) => {
      const filePct = p.total > 0 ? p.done / p.total : 0;
      upsertJob({
        id: "export",
        label: "Exporting folder",
        status: "running",
        pct: Math.round(((p.index + filePct) / Math.max(p.count, 1)) * 100),
        detail: `${p.index + 1}/${p.count}`,
        phase: p.path.split("/").pop() ?? p.phase,
      });
    }),
  );
  track(
    onExportBatchDone((d) => {
      if (d.failed.length) {
        failJob(
          "export",
          "Export finished with errors",
          `${d.failed.length} of ${d.ok.length + d.failed.length} failed:\n${d.failed.join("\n")}`,
        );
      } else {
        upsertJob({
          id: "export",
          label: d.cancelled ? "Export cancelled" : "Export complete",
          status: "ok",
          pct: 100,
          detail: `${d.ok.length} file${d.ok.length === 1 ? "" : "s"}`,
          phase: undefined,
        });
      }
    }),
  );

  // ── failures that previously surfaced nowhere ───────────────────────
  track(onDecodeError((message) => failJob("decode", "Could not decode RAW", message)));
  track(onEngineCrashed((message) => failJob("engine", "Render engine crashed", message)));

  let disposed = false;
  const unlistens: (() => void)[] = [];
  pending.forEach((p) =>
    p.then((u) => (disposed ? u() : unlistens.push(u))).catch(() => {}),
  );

  return () => {
    disposed = true;
    unlistens.forEach((u) => u());
  };
}
