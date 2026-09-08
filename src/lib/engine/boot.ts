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
  onPhotoWorkspace,
  onVideoWorkspace,
} from "../../ipc/events";
import { importAndBrowse, initBrowseBridge, pickAndImportPhotos, pinOpenedFile } from "../../stores/browse";
import {
  decodeState,
  engineReady,
  frameVersion,
  gpuAdapter,
  imageDims,
  imageMeta,
  imageOpen,
  lastOpenedPath,
  lastOpenedDocId,
  openingPreviewHint,
  openingPreviewUrl,
  selectedMask,
  statusMessage,
} from "../../stores/app";
import { clearDoc, reconcile, setDoc } from "../../stores/doc";
import { deselectMask } from "../../stores/mask";
import { isExportOpen, isSettingsOpen } from "../../stores/ui";
import { libraryRoute, setWorkspace, syncWorkspaceToOpenFile } from "../../stores/workspace";
import { licenseStatus } from "../../stores/session";
import { loadRegistry } from "./params";
import { flushCropDraft } from "../../crop/cropSession";

declare global {
  interface Window {
    __meratechRan?: boolean;
  }
}

let autoOpened = false;
let openSequence = 0;

export async function openPath(
  path: string,
  docId?: string | null,
  previewUrl?: string | null,
): Promise<void> {
  await flushCropDraft().catch(() => {});
  const sequence = ++openSequence;
  try {
    const name = path.split(/[/\\]/).pop() || path;
    statusMessage.set(`opening ${name}…`);
    decodeState.set("preview");
    openingPreviewUrl.set(previewUrl ?? null);
    imageMeta.set(null);
    imageDims.set(null);
    // Grid clicks set a hint first. Finder / IPC opens have no catalog size.
    if (previewUrl == null) {
      openingPreviewHint.set(null);
    }
    imageOpen.set(true);
    deselectMask();
    clearDoc();
    lastOpenedDocId.set(docId ?? null);
    const m = await openImage(path, docId);
    if (sequence !== openSequence) return;
    imageMeta.set(m);
    imageDims.set({ w: m.width, h: m.height });
    openingPreviewHint.set({
      w: m.width,
      h: m.height,
      orientation: m.orientation,
    });
    imageOpen.set(true);
    lastOpenedPath.set(path);
    statusMessage.set("decoding…");
    const d = await getDoc();
    if (d) {
      setDoc(d);
      lastOpenedDocId.set(d.doc_id);
    }
    push(syncWorkspaceToOpenFile(path, m.kind));
  } catch (e) {
    if (sequence !== openSequence) return;
    openingPreviewUrl.set(null);
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
        email: null,
        reason: isAppError(e) ? `${e.kind}: ${e.message}` : String(e),
      });
    });

  const unlistens = [
    onEngineReady((p) => {
      engineReady.set(true);
      gpuAdapter.set(p.adapter);
      void loadRegistry();
    }),
    onFileOpened((path) => {
      void openPath(path).then(() => pinOpenedFile(path));
    }),
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
      push(libraryRoute());
      void importAndBrowse(path).catch(() => null);
    }),
    onImportRequested(() => {
      push(libraryRoute());
      void pickAndImportPhotos().catch(() => null);
    }),
    onExportRequested(() => isExportOpen.set(true)),
    onSettingsRequested(() => isSettingsOpen.set(true)),
    onPhotoWorkspace(() => setWorkspace("photo")),
    onVideoWorkspace(() => setWorkspace("video")),
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
        void openPath(p).then(() => pinOpenedFile(p));
      }
    })
    .catch(() => {});

  return () => {
    window.clearInterval(retry);
    stopBrowse();
    unlistens.forEach((u) => u.then((f) => f()));
  };
}
