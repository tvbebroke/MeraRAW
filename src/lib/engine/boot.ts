// App bootstrap: engine event wiring + open flow (port of the old React
// App.tsx effect, feeding the nanostores the Svelte shell consumes).
import { push } from "svelte-spa-router";
import {
  autoopenPath,
  getDoc,
  licenseCheckLocal,
  openImage,
  pingEngine,
  reportFrontendStatus,
} from "../../ipc/commands";
import { isAppError } from "../../ipc/types";
import {
  onDecodeError,
  onDocUpdated,
  onEngineCrashed,
  onEngineReady,
  onFileOpened,
  onFolderOpened,
  onFrameReady,
  onImageReady,
  onImportRequested,
  onPreviewReady,
  onSettingsRequested,
  onExportRequested,
} from "../../ipc/events";
import { importAndBrowse, initBrowseBridge, pickAndImportFolder } from "../../stores/browse";
import {
  decodeState,
  engineReady,
  frameVersion,
  gpuAdapter,
  imageDims,
  imageMeta,
  imageOpen,
  lastOpenedPath,
  selectedMask,
  statusMessage,
} from "../../stores/app";
import { clearDoc, reconcile, setDoc } from "../../stores/doc";
import { isExportOpen, isSettingsOpen } from "../../stores/ui";
import { licenseStatus } from "../../stores/session";
import { loadRegistry } from "./params";

declare global {
  interface Window {
    __meratechRan?: boolean;
  }
}

let autoOpened = false;

export async function openPath(path: string): Promise<void> {
  try {
    const name = path.split(/[/\\]/).pop() || path;
    statusMessage.set(`opening ${name}…`);
    decodeState.set("preview");
    selectedMask.set(null);
    clearDoc();
    const m = await openImage(path);
    imageMeta.set(m);
    imageDims.set({ w: m.width, h: m.height });
    imageOpen.set(true);
    lastOpenedPath.set(path);
    statusMessage.set("decoding…");
    const d = await getDoc();
    if (d) setDoc(d);
    push("/edit");
  } catch (e) {
    decodeState.set("error");
    statusMessage.set(isAppError(e) ? `${e.kind}: ${e.message}` : String(e));
  }
}

export function initEngineBridge(): () => void {
  void loadRegistry();
  void licenseCheckLocal()
    .then((s) => licenseStatus.set(s))
    .catch((e) => {
      licenseStatus.set({
        licensed: false,
        userId: null,
        reason: isAppError(e) ? `${e.kind}: ${e.message}` : String(e),
      });
    });

  const unlistens = [
    onEngineReady((p) => {
      engineReady.set(true);
      gpuAdapter.set(p.adapter);
      void loadRegistry();
    }),
    onFileOpened((path) => void openPath(path)),
    onPreviewReady((version) => {
      statusMessage.set("preview · decoding…");
      frameVersion.set(version);
    }),
    onFrameReady((version) => frameVersion.set(version)),
    onImageReady((version) => {
      decodeState.set("ready");
      statusMessage.set("ready");
      frameVersion.set(version);
      reportFrontendStatus("image-ready-drawn").catch(() => {});
      // refresh meta — re-decodes (demosaic change, denoise reset) update it
      void import("../../ipc/commands").then(({ getMetadata }) =>
        getMetadata()
          .then((m) => {
            if (m) {
              imageMeta.set(m);
              imageDims.set({ w: m.width, h: m.height });
            }
          })
          .catch(() => {}),
      );
      if (!window.__meratechRan) {
        window.__meratechRan = true;
        void import("../../devHarness").then(({ runDevHarness }) =>
          runDevHarness(version),
        );
      }
    }),
    onEngineCrashed((m) => {
      decodeState.set("error");
      statusMessage.set(`engine crashed — restart app (${m})`);
    }),
    onDecodeError((m) => {
      decodeState.set("error");
      statusMessage.set(`decode error: ${m}`);
    }),
    onDocUpdated((delta) => reconcile(delta)),
    onFolderOpened((path) => {
      push("/library");
      void importAndBrowse(path).catch(() => null);
    }),
    onImportRequested(() => {
      push("/library");
      void pickAndImportFolder().catch(() => null);
    }),
    onExportRequested(() => isExportOpen.set(true)),
    onSettingsRequested(() => isSettingsOpen.set(true)),
  ];

  const stopBrowse = initBrowseBridge();

  pingEngine()
    .then((s) => {
      if (s.alive) {
        engineReady.set(true);
        gpuAdapter.set(s.adapter);
      } else statusMessage.set("engine ping: not alive yet — retrying…");
    })
    .catch((e) => {
      statusMessage.set(
        `engine ping failed: ${isAppError(e) ? `${e.kind}: ${e.message}` : String(e)}`,
      );
    });

  // Race-safe: engine-ready often fires before the webview listens.
  let tries = 0;
  const retry = window.setInterval(() => {
    tries += 1;
    if (engineReady.get() || tries > 40) {
      window.clearInterval(retry);
      return;
    }
    pingEngine()
      .then((s) => {
        if (s.alive) {
          engineReady.set(true);
          gpuAdapter.set(s.adapter);
          window.clearInterval(retry);
        }
      })
      .catch(() => {});
  }, 250);

  autoopenPath()
    .then((p) => {
      if (p && !autoOpened) {
        autoOpened = true;
        void openPath(p);
      }
    })
    .catch(() => {});

  return () => {
    window.clearInterval(retry);
    stopBrowse();
    unlistens.forEach((u) => u.then((f) => f()));
  };
}
