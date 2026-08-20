// In-app integration self-test (dev only). Runs when MERATECH_SELFTEST is
// set: drives real ops through the command path after image-ready and
// reports pass/fail via FRONTEND-REPORT log lines that test scripts grep.
import {
  applyOp,
  getGrid,
  getStats,
  importFolder,
  readFileMeta,
  reportFrontendStatus,
  setAssetMeta,
  setDemosaic,
  setParam,
  undo,
  wbFromPoint,
} from "./ipc/commands";
import { onFrameReady, onMaskReady } from "./ipc/events";
import { frameUrl } from "./lib/engine/frame";

function nextMaskReady(timeoutMs = 30000): Promise<string> {
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error("mask-ready timeout")), timeoutMs);
    const un = onMaskReady((id) => {
      clearTimeout(t);
      un.then((f) => f());
      resolve(id);
    });
  });
}

function nextFrame(timeoutMs = 4000): Promise<number> {
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error("frame-ready timeout")), timeoutMs);
    const un = onFrameReady((v) => {
      clearTimeout(t);
      un.then((f) => f());
      resolve(v);
    });
  });
}

async function centerLuma(version: number): Promise<number> {
  const res = await fetch(frameUrl(version));
  const w = parseInt(res.headers.get("X-Frame-Width") ?? "0", 10);
  const h = parseInt(res.headers.get("X-Frame-Height") ?? "0", 10);
  const buf = new Uint8Array(await res.arrayBuffer());
  const i = (Math.floor(h / 2) * w + Math.floor(w / 2)) * 4;
  return (buf[i] + buf[i + 1] + buf[i + 2]) / 3;
}

async function frameBytes(): Promise<Uint8Array> {
  const res = await fetch(frameUrl(Date.now()));
  return new Uint8Array(await res.arrayBuffer());
}

function framesDiffer(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return true;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return true;
  return false;
}

/** Resolve once no FrameReady has arrived for `quietMs`. Used around decode
 * boundaries where several frames can land (preview, initial decode,
 * re-decode) and we need the screen to represent the final one. */
function settleFrames(quietMs = 2500, timeoutMs = 35000): Promise<void> {
  return new Promise((resolve, reject) => {
    const done = (fn: () => void) => {
      clearTimeout(hard);
      clearTimeout(quiet);
      un.then((f) => f());
      fn();
    };
    const hard = setTimeout(
      () => done(() => reject(new Error("settleFrames timeout"))),
      timeoutMs,
    );
    let quiet = setTimeout(() => done(resolve), quietMs);
    const un = onFrameReady(() => {
      clearTimeout(quiet);
      quiet = setTimeout(() => done(resolve), quietMs);
    });
  });
}

/** Poll the rendered frame until `pred(centerLuma)` holds (renders are
 * debounced; event ordering races — pixels are the truth). */
async function waitLuma(
  pred: (l: number) => boolean,
  timeoutMs = 5000,
): Promise<number> {
  const t0 = Date.now();
  let last = -1;
  while (Date.now() - t0 < timeoutMs) {
    last = await centerLuma(Date.now());
    if (pred(last)) return last;
    await new Promise((r) => setTimeout(r, 120));
  }
  throw new Error(`waitLuma timeout, last=${last}`);
}

export async function runSelfTest(_latestVersion: number, scope = "1"): Promise<void> {
  const fail = (m: string) => reportFrontendStatus(`selftest-fail: ${m}`);
  try {
    // 0) demosaic switch re-decodes and repaints (merawler engine). Runs first
    // so it is independent of later, environment-dependent steps (import/grid).
    // Pixel-diff, not just frame events: a re-decode that leaves the render
    // graph cache warm emits FrameReady with the previous algorithm's pixels
    // (regression: finish_decode without invalidate_all).
    // Normalize to rcd first — the sidecar persists the demosaic, so a prior
    // (possibly interrupted) run may have left the image opening as amaze,
    // which would make the amaze switch below a pixel-identical no-op.
    const frameNorm = nextFrame(30000);
    const mN = await setDemosaic("rcd");
    if (mN.demosaic !== "rcd") {
      return void (await fail(`demosaic normalize: ${mN.demosaic}`));
    }
    await frameNorm;
    await settleFrames();
    const pixRcd = await frameBytes();
    const frameDem = nextFrame(30000); // re-decode + AMaZE takes a few seconds
    const mA = await setDemosaic("amaze");
    if (mA.demosaic !== "amaze") {
      return void (await fail(`demosaic not set: ${mA.demosaic}`));
    }
    await frameDem; // the re-decode repaint (FrameReady is emitted even if stale)
    await settleFrames();
    const pixAmaze = await frameBytes();
    if (!framesDiffer(pixRcd, pixAmaze)) {
      return void (await fail("demosaic switch left frame pixels unchanged (stale render cache)"));
    }
    const frameDem2 = nextFrame(30000);
    const mR = await setDemosaic("rcd");
    if (mR.demosaic !== "rcd") {
      return void (await fail(`demosaic restore: ${mR.demosaic}`));
    }
    await frameDem2;
    await settleFrames();
    const pixRcd2 = await frameBytes();
    if (!framesDiffer(pixAmaze, pixRcd2)) {
      return void (await fail("demosaic restore left frame pixels unchanged (stale render cache)"));
    }
    await reportFrontendStatus("selftest-demosaic-ok");

    // Hermetic baseline: a prior FAILED run leaves its edits in the sidecar
    // (the doc settles on fail), so the image can open with exposure already
    // at 1.5 — making the absolute setParam below a pixel-identical no-op
    // and every later run false-fail with "exposure no-op: X → X".
    const frameReset = nextFrame();
    await applyOp({ op: "reset_all" });
    await frameReset;
    await settleFrames();

    // luma baseline from the pixels on screen (not the boot FrameReady
    // version — that can be a cached frame and makes exposure look like a no-op).
    const baseline = await centerLuma(Date.now());

    // 1) exposure +1.5 stops through the real op path
    const delta = await setParam("exposure.stops", 1.5);
    if (delta.label !== "exposure.stops" || delta.undoDepth < 1) {
      return void (await fail(`bad delta ${JSON.stringify(delta)}`));
    }
    const brighter = await waitLuma((l) => l > baseline + 8, 8000);
    if (brighter <= baseline + 8) {
      return void (await fail(`exposure no-op: ${baseline} → ${brighter}`));
    }

    // 2) undo restores (depth is relative — the reset above is history too)
    const d2 = await undo();
    if (d2.undoDepth !== delta.undoDepth - 1) {
      return void (await fail("undo depth"));
    }
    const restored = await waitLuma((l) => Math.abs(l - baseline) <= 6, 8000);
    if (Math.abs(restored - baseline) > 6) {
      return void (await fail(`undo mismatch: ${baseline} vs ${restored}`));
    }

    // MERATECH_SELFTEST=exposure stops after the pixel checks that used to
    // false-fail on a stale FrameReady (`exposure no-op: X → X`).
    if (scope === "exposure") {
      await reportFrontendStatus("selftest-pass");
      return;
    }

    // 3) guard-wall: unknown path must reject as typed error
    let rejected = false;
    try {
      await applyOp({ op: "set_param", path: "hax.pwn", value: 1 });
    } catch {
      rejected = true;
    }
    if (!rejected) return void (await fail("guard-wall accepted bad path"));

    // 4) out-of-range clamps (not rejects)
    const d3 = await setParam("exposure.stops", 400);
    const clamped = (d3.doc.modules?.exposure as { stops?: number })?.stops;
    if (clamped !== 5) return void (await fail(`clamp got ${clamped}`));

    // restore sane exposure before later pixel checks (+5EV blows out center)
    const frameRestore = nextFrame();
    await setParam("exposure.stops", 0);
    await frameRestore;

    // 5) sidecar lands on settle (poll — webview timers can be throttled)
    const docPath = d3.doc.source_ref.path;
    const sidecar = docPath.replace(/\.[^.]+$/, ".mrt.json");
    let sidecarSeen = false;
    for (let i = 0; i < 20 && !sidecarSeen; i++) {
      await new Promise((r) => setTimeout(r, 300));
      sidecarSeen = (await readFileMeta(sidecar)).exists;
    }
    if (!sidecarSeen) return void (await fail(`sidecar missing: ${sidecar}`));

    // ---- Phase 3 coverage ----

    // 6) P3 module params route through registry + render (tone curve, HSL)
    const frameP3 = nextFrame();
    const d4 = await setParam("tone_curve.contrast", 55);
    if (!d4.doc.modules?.tone_curve) return void (await fail("tone_curve missing"));
    await frameP3;
    const frameP4 = nextFrame();
    await setParam("hsl.orange.sat", -40);
    await frameP4;

    // 7) histogram stats exist and are sane
    const stats = await getStats();
    if (!stats || stats.r.length !== stats.bins) {
      return void (await fail("stats missing"));
    }
    if (!isFinite(stats.clipHighPct) || stats.clipHighPct > 100) {
      return void (await fail(`clip pct ${stats.clipHighPct}`));
    }

    // 8) WB eyedropper: sample near top (background) → wb params land as
    // one undoable step
    const frameP5 = nextFrame();
    const d5 = await wbFromPoint(0.5, 0.12);
    const wb = d5.doc.modules?.white_balance as
      | { temp?: number; tint?: number }
      | undefined;
    if (!wb || typeof wb.temp !== "number") {
      return void (await fail("eyedropper set no temp"));
    }
    if (d5.label !== "wb eyedropper") return void (await fail("eyedropper label"));
    await frameP5;

    // ---- Phase 4 coverage ----

    // 9) radial mask: scoped exposure brightens, undoable
    const d6 = await applyOp({
      op: "add_mask",
      kind: "radial",
      source: { type: "radial", center: [0.5, 0.5], radii: [0.35, 0.3], rotation: 0 },
    });
    const maskId = d6.newMaskId;
    if (!maskId) return void (await fail("no mask id returned"));
    const beforeMask = await centerLuma(Date.now()).catch(() => 0);
    await setParam(`mask.${maskId}.exposure.stops`, 1.5);
    const afterMask = await waitLuma((l) => l > beforeMask + 4).catch(() => -1);
    if (afterMask < 0) {
      return void (await fail(`mask exposure no-op at ${beforeMask}`));
    }

    // 10) refine: opacity 0 neutralizes
    await applyOp({ op: "refine_mask", id: maskId, opacity: 0 });
    const neutral = await waitLuma((l) => Math.abs(l - beforeMask) <= 6).catch(
      () => -1,
    );
    if (neutral < 0) {
      return void (await fail(`opacity-0 mask still active (want ~${beforeMask})`));
    }

    // 11) subject mask: on-device segmentation completes + renders
    const maskReadyP = nextMaskReady();
    const d7 = await applyOp({
      op: "add_mask",
      kind: "subject",
      source: { type: "segmented", model: "subject_v1", hint: null },
    });
    if (!d7.newMaskId) return void (await fail("no subject mask id"));
    await setParam(`mask.${d7.newMaskId}.exposure.stops`, 1.0);
    const readyId = await maskReadyP;
    if (readyId !== d7.newMaskId) {
      return void (await fail(`mask-ready id mismatch ${readyId}`));
    }
    await nextFrame(8000).catch(() => 0); // segmentation render lands

    // cleanup: full reset, settle write
    await applyOp({ op: "reset_all" });
    await new Promise((r) => setTimeout(r, 900));

    // ---- Phase 5 coverage ----

    // 12) import the test folder; grid populates with thumbs. Scope queries
    // to the test folder — the default sort is captured_at over the WHOLE
    // catalog, so on a machine with prior imports the test asset can rank
    // past any sane limit.
    const folder = docPath.slice(0, docPath.lastIndexOf("/"));
    await importFolder(folder);
    // 120 × 250ms = 30s: hermetic runs decode 3 RAWs fresh in debug builds
    let item: import("./ipc/types").GridItem | undefined;
    for (let i = 0; i < 120 && !item; i++) {
      await new Promise((r) => setTimeout(r, 250));
      const grid = await getGrid({ limit: 50, folder });
      item = grid.find((g) => g.path === docPath && g.hasThumb);
    }
    if (!item) return void (await fail("import produced no thumbed grid item"));

    // 13) thumb protocol serves bytes (frame:// on macOS, http://thumb.localhost on Win)
    const { customSchemeUrl } = await import("./lib/engine/customScheme");
    const tres = await fetch(customSchemeUrl("thumb", `${item.id}?tier=t`));
    if (!tres.ok) return void (await fail(`thumb fetch ${tres.status}`));
    if ((await tres.arrayBuffer()).byteLength < 500) {
      return void (await fail("thumb too small"));
    }

    // 14) culling write: DB + sidecar mirror
    await setAssetMeta([item.id], {
      rating: 4,
      flag: "pick",
      addKeyword: "selftest",
    });
    const grid2 = await getGrid({ ratingMin: 4, limit: 50, folder });
    if (!grid2.some((g) => g.id === item!.id)) {
      return void (await fail("rating filter missed the rated asset"));
    }
    const sc = await readFileMeta(docPath.replace(/\.[^.]+$/, ".mrt.json"));
    if (!sc.exists) return void (await fail("sidecar mirror missing"));

    const { invoke } = await import("@tauri-apps/api/core");

    // ---- Phase 6 coverage (mock executor — real tools, no API) ----
    if (await invoke<boolean>("assistant_available")) {
      const reply = await invoke<string>("assistant_send", {
        message: "warm this up",
        mode: "edit",
      });
      if (!reply.includes("MOCK RUN COMPLETE")) {
        return void (await fail("assistant mock did not complete"));
      }
      // guard-wall held inside the assistant path
      if (!reply.includes("hax.pwn") || !reply.includes('"ok": false')) {
        return void (await fail("assistant guard-wall probe missing"));
      }
      if (!reply.includes('"had_image": true')) {
        return void (await fail("look-again preview missing"));
      }
      // assistant edits land on the SAME undo stack (DoD 6.10 #1)
      const afterAssistant = await import("./ipc/commands").then((c) => c.getDoc());
      const exp = (afterAssistant?.modules?.exposure as { stops?: number })?.stops;
      if (exp !== 5) {
        return void (await fail(`assistant clamp landed ${exp}, want 5`));
      }
      const u = await undo(); // user undoes the assistant
      if (u.undoDepth < 0) return void (await fail("assistant undo failed"));
      await applyOp({ op: "reset_all" });
      await new Promise((r) => setTimeout(r, 700));
    }

    // ---- Phase 7 coverage ----

    // 15) preset save → reset → apply (undoable partial-doc)
    await setParam("exposure.stops", 0.7);
    await invoke("save_preset", { name: "selftest-look", modules: ["exposure"] });
    const names = await invoke<string[]>("list_presets");
    if (!names.includes("selftest-look")) {
      return void (await fail("preset missing from list"));
    }
    await applyOp({ op: "reset_all" });
    const dp = await invoke<import("./ipc/types").DocDelta>("apply_preset", {
      name: "selftest-look",
    });
    const pexp = (dp.doc.modules?.exposure as { stops?: number })?.stops;
    if (pexp === undefined || Math.abs(pexp - 0.7) > 1e-4) {
      return void (await fail(`preset apply got ${pexp}`));
    }

    // 16) export: real output transform + ICC, file lands
    const out = await invoke<string>("export_image", {
      settings: {
        format: "jpeg",
        target: "srgb",
        quality: 88,
        maxDim: 1024,
        sharpen: 30,
        destDir: "/tmp/meratech-selftest-exports",
      },
    });
    const exported = await readFileMeta(out);
    if (!exported.exists || exported.size < 30_000) {
      return void (await fail(`export bad: ${out} ${exported.size}`));
    }

    // 16b) batch export: engine-side queue (decode+render per image, open
    // image untouched); completion arrives as export-batch-done
    {
      const { listen } = await import("@tauri-apps/api/event");
      let unBatch: (() => void) | undefined;
      const donePromise = new Promise<import("./ipc/types").ExportBatchDone>(
        (resolve, reject) => {
          // debug-build decodes are 6-8s each idle but balloon under load
          const t = setTimeout(
            () => reject(new Error("batch export timeout")),
            360000,
          );
          void listen<import("./ipc/types").ExportBatchDone>(
            "export-batch-done",
            (e) => {
              clearTimeout(t);
              resolve(e.payload);
            },
          ).then((u) => {
            unBatch = u;
          });
        },
      );
      const batchPaths = (await getGrid({ limit: 3, folder })).map((g) => g.path);
      if (batchPaths.length < 2) return void (await fail("batch: <2 grid paths"));
      const accepted = await invoke<number>("export_batch", {
        paths: batchPaths,
        settings: {
          format: "jpeg",
          target: "srgb",
          quality: 85,
          maxDim: 1024,
          sharpen: 25,
          destDir: "/tmp/meratech-selftest-batch",
        },
      });
      if (accepted !== batchPaths.length) {
        return void (await fail(`batch accepted ${accepted}/${batchPaths.length}`));
      }
      const done = await donePromise.finally(() => unBatch?.());
      if (done.cancelled) return void (await fail("batch cancelled unexpectedly"));
      if (done.failed.length > 0) {
        return void (await fail(`batch failed: ${done.failed[0].error}`));
      }
      if (done.ok.length !== batchPaths.length) {
        return void (await fail(`batch wrote ${done.ok.length}/${batchPaths.length}`));
      }
      for (const outPath of done.ok) {
        const m = await readFileMeta(outPath);
        if (!m.exists || m.size < 20_000) {
          return void (await fail(`batch output bad: ${outPath} ${m.size}`));
        }
      }
    }

    // 17) perf instrumentation: render budget + cache reuse counted
    const perf = await invoke<{
      lastRenderMs: number;
      renders: number;
      cachedRenders: number;
    }>("get_perf_stats");
    if (perf.renders < 5) return void (await fail("perf: no renders counted"));
    if (perf.cachedRenders < 1) {
      return void (await fail("perf: cache-the-chain never reused upstream"));
    }
    if (perf.lastRenderMs > 100) {
      return void (await fail(`perf: render ${perf.lastRenderMs}ms over budget`));
    }

    // final cleanup
    await applyOp({ op: "reset_all" });
    await new Promise((r) => setTimeout(r, 700));

    await reportFrontendStatus("selftest-pass");
  } catch (e) {
    await fail(e instanceof Error ? e.message : String(e));
  }
}
