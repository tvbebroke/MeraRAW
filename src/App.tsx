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
} from "./ipc/events";
import { isAppError, type ImageMeta } from "./ipc/types";
import { ExportDialog } from "./components/lr/ExportDialog";
import { KeyboardHelpOverlay } from "./components/KeyboardHelpOverlay";
import { LeftPanel } from "./components/lr/LeftPanel";
import { RightRail } from "./components/lr/RightRail";
import { ViewportToolbar } from "./components/lr/Toolbar";
import { Icon } from "./components/lr/widgets";
import { Filmstrip, Library } from "./components/Library";
import { ReportProblem } from "./components/ReportProblem";
import { setAppKeyboardContext } from "./keyboard/context";
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
  const helpOverlay = useUiStore((s) => s.helpOverlay);
  const setHelpOverlay = useUiStore((s) => s.setHelpOverlay);
  const [meta, setMeta] = useState<ImageMeta | null>(null);
  const [mode, setMode] = useState<"library" | "develop">("library");
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
      getMode: () => mode,
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
    ];
    pingEngine()
      .then((s) => {
        if (s.alive) setEngineReady(s.adapter);
      })
      .catch(() => {});
    autoopenPath()
      .then((p) => {
        if (p && !autoOpened) {
          autoOpened = true;
          void open(p);
        }
      })
      .catch(() => {});
    return () => {
      unlistens.forEach((u) => u.then((f) => f()));
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // before/after → engine bypass
  useEffect(() => {
    setPreviewBypass(beforeAfter).catch(() => {});
  }, [beforeAfter]);

  async function handleOpen() {
    const path = await pickFile();
    if (path) void open(path);
  }

  return (
    <div className={`app ${showTopbar ? "" : "chrome-all-hidden"}`}>
      {showTopbar && (
      <div className="topbar">
        <span className="brand">
          <img src="/logo.png" alt="" className="brand-logo" aria-hidden="true" />
          MeraRAW
        </span>
        <span
          className="beta-badge"
          title="Beta — the full version is coming soon. Built by a solo dev who's passionate about color grading."
        >
          BETA
        </span>
        <div className="topbar-tabs">
          <button
            className={mode === "library" ? "tab active" : "tab"}
            onClick={() => setMode("library")}
          >
            Library
          </button>
          <button
            className={mode === "develop" ? "tab active" : "tab"}
            onClick={() => setMode("develop")}
            disabled={!meta}
          >
            Develop
          </button>
        </div>
        <div className="topbar-right">
          <button className="tab" onClick={handleOpen}>
            Open…
          </button>
          <button className="tab" disabled={!meta} onClick={() => {
            setExportQueue(undefined);
            setShowExport(true);
          }}>
            <Icon.Export size={13} /> Export…
          </button>
        </div>
      </div>
      )}

      {mode === "library" ? (
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
              {showToolbar && <ViewportToolbar />}
            </div>
            <RightRail meta={meta} onMetaChange={setMeta} />
          </div>
          {showFilmstrip && (
            <Filmstrip currentPath={lastOpenedPath} onOpen={(p) => void open(p)} />
          )}
        </div>
      )}

      <div className="statusbar">
        <span className={engineReady ? "ok" : "err"}>
          {engineReady ? "ready" : "starting…"}
        </span>
        <span>{gpuAdapter ?? "—"}</span>
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
    </div>
  );
}
