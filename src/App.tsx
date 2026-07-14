import { useEffect, useState } from "react";
import { Viewport } from "./viewport/Viewport";
import { useUiStore } from "./state/uiStore";
import {
  autoopenPath,
  openImage,
  pickFile,
  pickFolder,
  pingEngine,
  reportFrontendStatus,
  setPreviewBypass,
} from "./ipc/commands";
import {
  onDecodeError,
  onDocUpdated,
  onEngineCrashed,
  onEngineReady,
  onExportRequested,
  onFileOpened,
  onFolderOpened,
  onImageReady,
  onImportRequested,
  onPreviewReady,
  onSettingsRequested,
} from "./ipc/events";
import { isAppError, type ImageMeta } from "./ipc/types";
import { ExportDialog } from "./components/lr/ExportDialog";
import { KeyboardHelpOverlay } from "./components/KeyboardHelpOverlay";
import { LeftPanel } from "./components/lr/LeftPanel";
import { RightRail } from "./components/lr/RightRail";
import { ViewportToolbar } from "./components/lr/Toolbar";
import { ZenAiBar } from "./components/lr/ZenAiBar";
import { Icon } from "./components/lr/widgets";
import { Filmstrip, Library } from "./components/Library";
import { Education } from "./components/Education";
import { Histogram } from "./components/Histogram";
import { ReportProblem } from "./components/ReportProblem";
import { SettingsDialog } from "./components/SettingsDialog";
import { ViewportBgMenuButton } from "./components/ViewportBgPicker";
import { PsychedelicBg } from "./components/PsychedelicBg";
import { PsychedelicControls } from "./components/PsychedelicControls";
import { EarlySupporterModal } from "./components/EarlySupporterModal";
import { getSupporterStatus } from "./services/purchaseService";
import { setAppKeyboardContext } from "./keyboard/context";
import {
  applyViewportBgAttr,
  syncWindowBackdrop,
} from "./theme/viewportBackground";
import { useKeyboardShortcuts } from "./keyboard/useKeyboardShortcuts";
import { useDocStore } from "./state/docStore";
import type { KeymapScope } from "./keyboard/types";

// window-level: survives React StrictMode double-mount AND module reloads
declare global {
  interface Window {
    __meratechRan?: boolean;
    __meratechOpened?: boolean;
  }
}
let autoOpened = false;

export default function App() {
  const {
    engineReady,
    gpuAdapter,
    statusMessage,
    decodeState,
    lastOpenedPath,
    setEngineReady,
    setStatus,
    setLastOpenedPath,
    setImage,
    setDecodeState,
  } = useUiStore();
  const beforeAfter = useUiStore((s) => s.beforeAfter);
  const showTopbar = useUiStore((s) => s.showTopbar);
  const showToolbar = useUiStore((s) => s.showToolbar);
  const showFilmstrip = useUiStore((s) => s.showFilmstrip);
  const showLeftPanel = useUiStore((s) => s.showLeftPanel);
  const showRightPanel = useUiStore((s) => s.showRightPanel);
  const viewportBg = useUiStore((s) => s.viewportBg);
  const helpOverlay = useUiStore((s) => s.helpOverlay);
  const setHelpOverlay = useUiStore((s) => s.setHelpOverlay);
  const settingsOpen = useUiStore((s) => s.settingsOpen);
  const setSettingsOpen = useUiStore((s) => s.setSettingsOpen);
  const earlySupporterOpen = useUiStore((s) => s.earlySupporterOpen);
  const setEarlySupporterOpen = useUiStore((s) => s.setEarlySupporterOpen);
  const isEarlySupporter = useUiStore((s) => s.isEarlySupporter);
  const setIsEarlySupporter = useUiStore((s) => s.setIsEarlySupporter);
  const [meta, setMeta] = useState<ImageMeta | null>(null);
  const [mode, setMode] = useState<"library" | "develop" | "education">("library");
  const [showExport, setShowExport] = useState(false);
  const [exportQueue, setExportQueue] = useState<string[] | undefined>();
  const [importReviewPath, setImportReviewPath] = useState<string | null>(null);

  useKeyboardShortcuts(true);

  useEffect(() => {
    setAppKeyboardContext({
      setMode,
      openPath: (p) => void open(p),
      openFilePicker: () => void handleOpen(),
      triggerImport: () => {
        setMode("library");
        void pickFolder().then((p) => {
          if (p) setImportReviewPath(p);
        });
      },
      triggerExport: () => {
        setExportQueue(undefined);
        setShowExport(true);
      },
      triggerExportPrevious: () => {
        notifyExportPrevious();
      },
      getMode: () => (mode === "education" ? "library" : mode),
      hasImage: () => meta !== null,
    });
    return () => setAppKeyboardContext(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, meta]);

  function notifyExportPrevious() {
    setStatus("Quick export with previous settings — coming soon");
    setShowExport(true);
  }

  const helpScopes: KeymapScope[] =
    mode === "develop"
      ? ["module:develop", "global"]
      : ["module:library", "global"];

  async function open(path: string) {
    setMode("develop");
    try {
      setStatus(`opening ${path.split("/").pop()}…`);
      setDecodeState("preview");
      useDocStore.getState().clear();
      const m = await openImage(path);
      setMeta(m);
      setImage({ w: m.width, h: m.height });
      setLastOpenedPath(path);
      setStatus("decoding…");
      const { getDoc } = await import("./ipc/commands");
      const doc = await getDoc();
      if (doc) useDocStore.getState().setDoc(doc);
    } catch (e) {
      setDecodeState("error");
      setStatus(isAppError(e) ? `${e.kind}: ${e.message}` : String(e));
    }
  }

  useEffect(() => {
    const unlistens = [
      onEngineReady((p) => setEngineReady(p.adapter)),
      onFileOpened((path) => void open(path)),
      onPreviewReady(() => setStatus("preview · decoding…")),
      onImageReady((version) => {
        setDecodeState("ready");
        setStatus("ready · export OK");
        reportFrontendStatus("image-ready-drawn").catch(() => {});
        if (!window.__meratechRan) {
          window.__meratechRan = true;
          void import("./devHarness").then(({ runDevHarness }) => runDevHarness(version));
        }
      }),
      onEngineCrashed((m) => {
        setDecodeState("error");
        setStatus(`engine crashed — restart app (${m})`);
      }),
      onDecodeError((m) => {
        setDecodeState("error");
        setStatus(`decode error: ${m}`);
      }),
      onDocUpdated((delta) => useDocStore.getState().reconcile(delta)),
      onFolderOpened((path) => {
        setMode("library");
        setImportReviewPath(path);
      }),
      onImportRequested(() => {
        setMode("library");
        void pickFolder().then((p) => {
          if (p) setImportReviewPath(p);
        });
      }),
      onExportRequested(() => {
        if (meta || exportQueue) setShowExport(true);
      }),
      onSettingsRequested(() => setSettingsOpen(true)),
    ];
    pingEngine()
      .then((s) => {
        if (s.alive) setEngineReady(s.adapter);
        else setStatus("engine ping: not alive yet — retrying…");
      })
      .catch((e) => {
        const msg = isAppError(e) ? `${e.kind}: ${e.message}` : String(e);
        setStatus(`engine ping failed: ${msg}`);
        console.error("ping_engine failed", e);
      });
    // Race-safe: engine-ready often fires before the webview listens.
    let tries = 0;
    const retry = window.setInterval(() => {
      tries += 1;
      if (useUiStore.getState().engineReady || tries > 40) {
        window.clearInterval(retry);
        return;
      }
      pingEngine()
        .then((s) => {
          if (s.alive) {
            setEngineReady(s.adapter);
            window.clearInterval(retry);
          }
        })
        .catch(() => {});
    }, 250);
    autoopenPath()
      .then((p) => {
        if (p && !autoOpened) {
          autoOpened = true;
          void open(p);
        }
      })
      .catch(() => {});
    return () => {
      window.clearInterval(retry);
      unlistens.forEach((u) => u.then((f) => f()));
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    void getSupporterStatus().then((s) => setIsEarlySupporter(s.isEarlySupporter));
  }, [setIsEarlySupporter]);

  // before/after → engine bypass
  useEffect(() => {
    setPreviewBypass(beforeAfter).catch(() => {});
  }, [beforeAfter]);

  // Preview backdrop preference (CSS + optional macOS vibrancy).
  useEffect(() => {
    applyViewportBgAttr(viewportBg);
    void syncWindowBackdrop(viewportBg);
  }, [viewportBg]);

  async function handleOpen() {
    const path = await pickFile();
    if (path) void open(path);
  }

  return (
    <div
      className={`app ${showTopbar ? "" : "chrome-all-hidden"}`}
      data-viewport-bg={viewportBg}
    >
      {viewportBg === "psychedelic" && (
        <div className="psy-layer">
          <PsychedelicBg />
          <div className="psy-controls-float topbar-no-drag">
            <PsychedelicControls compact />
          </div>
        </div>
      )}
      {showTopbar && (
      <div className="topbar" data-tauri-drag-region>
        <span className="brand" data-tauri-drag-region>
          <img src="/logo.png" alt="" className="brand-logo" aria-hidden="true" />
          MeraRAW
        </span>
        <span
          className="beta-badge"
          data-tauri-drag-region
          title="Beta — the full version is coming soon. Built by a solo dev who's passionate about color grading."
        >
          BETA
        </span>
        <nav className="topbar-tabs topbar-no-drag" aria-label="Main">
          <button
            type="button"
            className={mode === "library" ? "tab active" : "tab"}
            onClick={() => setMode("library")}
          >
            Library
          </button>
          <button
            type="button"
            className={mode === "develop" ? "tab active" : "tab"}
            onClick={() => setMode("develop")}
            disabled={!meta}
            title={meta ? "Develop" : "Open a photo first"}
          >
            Develop
          </button>
          <button
            type="button"
            className={mode === "education" ? "tab active" : "tab"}
            onClick={() => setMode("education")}
          >
            Education
          </button>
        </nav>
        <div className="topbar-right topbar-no-drag">
          {mode === "develop" ? (
            <>
              <button
                type="button"
                className="topbar-icon-btn"
                title="Settings"
                onClick={() => setSettingsOpen(true)}
              >
                <Icon.Settings size={16} />
              </button>
              <ViewportBgMenuButton />
              <button
                type="button"
                className={`zen-toggle ${showRightPanel ? "" : "zen"}`}
                title={showRightPanel ? "Switch to Zen" : "Switch to Expert"}
                onClick={() => useUiStore.getState().toggleRightPanel()}
              >
                <span className="zen-toggle-knob" />
                <span className="zen-toggle-label">
                  {showRightPanel ? "Expert" : "Zen"}
                </span>
              </button>
              <button
                type="button"
                className="topbar-icon-btn"
                title="Keyboard shortcuts"
                onClick={() => setHelpOverlay(true)}
              >
                <Icon.Help size={16} />
              </button>
              <button
                type="button"
                className="topbar-export-btn"
                disabled={!meta}
                title="Export"
                onClick={() => {
                  setExportQueue(undefined);
                  setShowExport(true);
                }}
              >
                <Icon.Export size={16} />
              </button>
            </>
          ) : mode === "library" ? (
            <>
              <button
                type="button"
                className="topbar-icon-btn"
                title="Settings"
                onClick={() => setSettingsOpen(true)}
              >
                <Icon.Settings size={16} />
              </button>
              <ViewportBgMenuButton />
              <button
                type="button"
                className="topbar-icon-btn"
                title="Keyboard shortcuts"
                onClick={() => setHelpOverlay(true)}
              >
                <Icon.Help size={16} />
              </button>
              <button
                type="button"
                className="topbar-edit-btn"
                disabled={!meta}
                title="Open Develop"
                onClick={() => meta && setMode("develop")}
              >
                Edit
                <Icon.Pencil size={14} />
              </button>
            </>
          ) : (
            <>
              <button className="tab" onClick={handleOpen}>
                Open…
              </button>
              <button className="tab" disabled={!meta} onClick={() => {
                setExportQueue(undefined);
                setShowExport(true);
              }}>
                <Icon.Export size={13} /> Export…
              </button>
            </>
          )}
        </div>
      </div>
      )}

      {mode === "education" ? (
        <Education
          onSupportDevelopment={() => setEarlySupporterOpen(true)}
          isEarlySupporter={isEarlySupporter}
        />
      ) : mode === "library" ? (
        <Library
          onOpen={(p) => void open(p)}
          onExportSelection={(paths) => {
            setExportQueue(paths);
            setShowExport(true);
          }}
          pendingReviewRoot={importReviewPath}
          onReviewClosed={() => setImportReviewPath(null)}
        />
      ) : (
        <div className={`develop-wrap ${showFilmstrip ? "" : "hide-filmstrip"}`}>
          <div
            className={`develop ${showLeftPanel ? "" : "hide-left"} ${showRightPanel ? "" : "hide-right"}`}
          >
            <LeftPanel currentPath={lastOpenedPath} onOpen={(p) => void open(p)} />
            <div className="develop-center">
              <Viewport />
              {!showRightPanel && <ZenAiBar />}
              {showToolbar && <ViewportToolbar />}
            </div>
            <RightRail meta={meta} onMetaChange={setMeta} />
          </div>
          {showFilmstrip && (
            <div className="develop-bottom">
              <Filmstrip currentPath={lastOpenedPath} onOpen={(p) => void open(p)} />
              {showRightPanel && (
                <div className="develop-hist">
                  <Histogram />
                </div>
              )}
            </div>
          )}
        </div>
      )}

      <div className="statusbar">
        <span className={engineReady ? "ok" : "err"}>
          {engineReady ? "ready" : "starting…"}
        </span>
        <span>{gpuAdapter ?? "—"}</span>
        {meta && (
          <span
            className={`file-badge ${meta.kind === "raw" ? "is-raw" : "is-rendered"}`}
            title={
              meta.kind === "raw"
                ? `${meta.format} — camera RAW (full sensor data)`
                : `${meta.format} — already-rendered image${meta.bitDepth ? `, ${meta.bitDepth}-bit` : ""}`
            }
          >
            {meta.format}
            {meta.kind === "raw" ? " · RAW" : ""}
            {meta.bitDepth ? ` · ${meta.bitDepth}-bit` : ""}
          </span>
        )}
        {meta && (
          <span className="status-meta">
            {meta.cameraModel} · {meta.lens ?? "—"} · ISO {meta.iso ?? "—"} ·{" "}
            {meta.shutter ?? "—"} · {meta.aperture ? `f/${meta.aperture.toFixed(1)}` : "—"}{" "}
            · {meta.width}×{meta.height}
          </span>
        )}
        <span className={`status-msg ${decodeState === "error" ? "err" : ""}`}>
          {statusMessage}
        </span>
      </div>

      {showExport && (
        <ExportDialog
          queuePaths={exportQueue}
          onClose={() => {
            setShowExport(false);
            setExportQueue(undefined);
          }}
        />
      )}

      <ReportProblem />

      <KeyboardHelpOverlay
        open={helpOverlay}
        scopes={helpScopes}
        onClose={() => setHelpOverlay(false)}
      />

      <SettingsDialog
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        onSupportDevelopment={() => {
          setSettingsOpen(false);
          setEarlySupporterOpen(true);
        }}
      />

      <EarlySupporterModal
        open={earlySupporterOpen}
        onClose={() => setEarlySupporterOpen(false)}
        isSupporter={isEarlySupporter}
      />
    </div>
  );
}
