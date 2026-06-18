import { useEffect, useState } from "react";
import { Viewport } from "./viewport/Viewport";
import { useUiStore } from "./state/uiStore";
import {
  autoopenPath,
  openImage,
  pickFile,
  pingEngine,
  redo,
  reportFrontendStatus,
  setPreviewBypass,
  undo,
} from "./ipc/commands";
import {
  onDecodeError,
  onDocUpdated,
  onEngineReady,
  onFileOpened,
  onImageReady,
  onPreviewReady,
} from "./ipc/events";
import { isAppError, type ImageMeta } from "./ipc/types";
import { DevelopRail } from "./components/lr/DevelopRail";
import { ExportDialog } from "./components/lr/ExportDialog";
import { LeftPanel } from "./components/lr/LeftPanel";
import { ViewportToolbar } from "./components/lr/Toolbar";
import { Icon } from "./components/lr/widgets";
import { Filmstrip, Library } from "./components/Library";
import { useDocStore } from "./state/docStore";

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
  const setBeforeAfter = useUiStore((s) => s.setBeforeAfter);
  const setBrushRadius = useUiStore((s) => s.setBrushRadius);
  const [meta, setMeta] = useState<ImageMeta | null>(null);
  const [mode, setMode] = useState<"library" | "develop">("library");
  const [showExport, setShowExport] = useState(false);

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
      onPreviewReady(() => setStatus("preview (decoding…)")),
      onImageReady((version) => {
        setDecodeState("ready");
        setStatus("ready");
        reportFrontendStatus("image-ready-drawn").catch(() => {});
        if (!window.__meratechRan) {
          window.__meratechRan = true;
          void (async () => {
            const { invoke } = await import("@tauri-apps/api/core");
            if (await invoke<boolean>("selftest_enabled")) {
              const { runSelfTest } = await import("./selftest");
              void runSelfTest(version);
            } else if (await invoke<boolean>("live_assistant_enabled")) {
              const { runLiveAssistant } = await import("./liveassistant");
              void runLiveAssistant();
            } else if (await invoke<boolean>("verify_slider_enabled").catch(() => false)) {
              const { verifySlider } = await import("./verifyslider");
              void verifySlider();
            }
          })();
        }
      }),
      onDecodeError((m) => {
        setDecodeState("error");
        setStatus(`decode error: ${m}`);
      }),
      onDocUpdated((delta) => useDocStore.getState().reconcile(delta)),
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

  // Develop keyboard map (skip when typing in an input)
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const t = e.target as HTMLElement;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA")) return;
      if (mode !== "develop") return;
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z") {
        e.preventDefault();
        (e.shiftKey ? redo() : undo())
          .then((d) => useDocStore.getState().reconcile(d))
          .catch(() => {});
      } else if (e.key === "\\") {
        e.preventDefault();
        setBeforeAfter(!useUiStore.getState().beforeAfter);
      } else if (e.key === "[") {
        setBrushRadius(useUiStore.getState().brushRadius - 0.01);
      } else if (e.key === "]") {
        setBrushRadius(useUiStore.getState().brushRadius + 0.01);
      } else if (e.key.toLowerCase() === "d") {
        setMode("develop");
      } else if (e.key.toLowerCase() === "g") {
        setMode("library");
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [mode, setBeforeAfter, setBrushRadius]);

  async function handleOpen() {
    const path = await pickFile();
    if (path) void open(path);
  }

  return (
    <div className="app">
      <div className="topbar">
        <span className="brand">MeraRAW</span>
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
          <button className="tab" disabled={!meta} onClick={() => setShowExport(true)}>
            <Icon.Export size={13} /> Export…
          </button>
        </div>
      </div>

      {mode === "library" ? (
        <Library onOpen={(p) => void open(p)} />
      ) : (
        <div className="develop-wrap">
          <div className="develop">
            <LeftPanel />
            <div className="develop-center">
              <Viewport />
              <ViewportToolbar />
            </div>
            <DevelopRail meta={meta} />
          </div>
          <Filmstrip currentPath={lastOpenedPath} onOpen={(p) => void open(p)} />
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

      {showExport && <ExportDialog onClose={() => setShowExport(false)} />}
    </div>
  );
}
