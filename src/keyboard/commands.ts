import { applyParamBatch, redo, setParam, snapshot, undo } from "../ipc/commands";
import { useDocStore } from "../state/docStore";
import { useUiStore } from "../state/uiStore";
import { getAppKeyboardContext, getFilmstripKeyboardContext, getLibraryKeyboardContext } from "./context";
import { registerCommandHandler, registerCommandHandlers } from "./dispatcher";
import { allKeyBindings } from "./loadKeymap";
import type { ResolvedBinding } from "./types";

function reconcile() {
  return useDocStore.getState().reconcile;
}

function ui() {
  return useUiStore.getState();
}

function notify(msg: string) {
  ui().setStatus(msg);
}

function stub(binding: ResolvedBinding) {
  notify(`${binding.action} — coming soon`);
}

async function confirmDestructive(message: string): Promise<boolean> {
  return window.confirm(message);
}

const HANDLERS: Record<string, (b: ResolvedBinding) => void | Promise<void>> = {
  "panels---chrome:undo": async () => {
    await undo().then(reconcile());
  },
  "panels---chrome:redo": async () => {
    await redo().then(reconcile());
  },
  "panels---chrome:redo--windows-only-": async () => {
    await redo().then(reconcile());
  },
  "panels---chrome:show-hide-side-panels": () => ui().toggleSidePanels(),
  "panels---chrome:show-hide-all-panels": () => ui().toggleAllPanels(),
  "panels---chrome:show-hide-toolbar": () => ui().toggleToolbar(),
  "panels---chrome:show-hide-filmstrip": () => ui().toggleFilmstrip(),
  "panels---chrome:show-hide-left-panels": () => ui().toggleLeftPanel(),
  "panels---chrome:show-hide-right-panels": () => ui().toggleRightPanel(),
  "panels---chrome:open-close-right-panels--library-develop": (b) => {
    const digit = b.chord.match(/(\d)$/)?.[1];
    if (!digit) return;
    const n = parseInt(digit, 10);
    const app = getAppKeyboardContext();
    if (app?.getMode() === "develop") {
      const tabs = ["presets", "edit", "crop", "remove", "masking"] as const;
      if (n >= 1 && n <= tabs.length) ui().setRightRailTab(tabs[n - 1]);
    } else {
      notify(`Library panel ${n} — coming soon`);
    }
  },
  "module-navigation:go-to-library": () => getAppKeyboardContext()?.setMode("library"),
  "module-navigation:go-to-develop": () => {
    const app = getAppKeyboardContext();
    if (app?.hasImage()) app.setMode("develop");
  },
  "module-navigation:go-back---forward": (b) => {
    notify(`${b.action} — coming soon`);
  },
  "module-navigation:go-back-to-previous-module": () => {
    const app = getAppKeyboardContext();
    if (!app) return;
    app.setMode(app.getMode() === "develop" ? "library" : "develop");
  },
  "views---screen-modes:enter-loupe-view": () => getLibraryKeyboardContext()?.openActive(),
  "views---screen-modes:enter-grid-view": () => {
    const app = getAppKeyboardContext();
    if (app?.getMode() === "library") {
      getLibraryKeyboardContext()?.toggleGridMode();
    } else {
      app?.setMode("library");
    }
  },
  "views---screen-modes:enter-compare-view": () => stub({ action: "Compare view" } as ResolvedBinding),
  "views---screen-modes:enter-survey-view": () => stub({ action: "Survey view" } as ResolvedBinding),
  "views---screen-modes:open-selected-photo-in-develop": () =>
    getLibraryKeyboardContext()?.openActive(),
  "views---screen-modes:cycle-screen-modes": () => ui().toggleFullscreen(),
  "views---screen-modes:cycle-info-overlay": () => ui().cycleInfoOverlay(),
  "views---screen-modes:show-hide-info-overlay": () => ui().toggleInfoOverlay(),
  "views---screen-modes:open-reference-view": () => stub({ action: "Reference view" } as ResolvedBinding),
  "views---screen-modes:zoom-to-100-": () => ui().sendViewCmd("oneToOne"),
  "photos---catalog-management:import-photos-from-disk": () => {
    void getAppKeyboardContext()?.triggerImport();
  },
  "photos---catalog-management:open-preferences": () => notify("Preferences — coming soon"),
  "photos---catalog-management:create-new-folder": () => notify("Create folder — coming soon"),
  "photos---catalog-management:create-virtual-copy": () => notify("Virtual copy — coming soon"),
  "photos---catalog-management:show-in-explorer-finder": () =>
    notify("Reveal in Finder — coming soon"),
  "photos---catalog-management:next-previous-photo-in-filmstrip": (b) => {
    const delta = b.chord.endsWith("Left") ? -1 : 1;
    const lib = getLibraryKeyboardContext();
    if (lib && !lib.isReviewOpen()) {
      lib.advanceSelection(delta);
      return;
    }
    getFilmstripKeyboardContext()?.advance(delta);
  },
  "photos---catalog-management:go-to-next-image": () =>
    getLibraryKeyboardContext()?.advanceSelection(1),
  "photos---catalog-management:go-to-previous-image": () =>
    getLibraryKeyboardContext()?.advanceSelection(-1),
  "photos---catalog-management:export-selected-photo-s": () =>
    getAppKeyboardContext()?.triggerExport(),
  "photos---catalog-management:export-with-previous-settings": () =>
    getAppKeyboardContext()?.triggerExportPrevious(),
  "photos---catalog-management:delete-selected-photo-s-": async (b) => {
    if (!(await confirmDestructive(`${b.action}?`))) return;
    notify("Delete photo — coming soon");
  },
  "photos---catalog-management:remove-from-catalog": async () => {
    if (!(await confirmDestructive("Remove from catalog?"))) return;
    notify("Remove from catalog — coming soon");
  },
  "photos---catalog-management:delete---move-to-recycle-trash": async (b) => {
    if (!(await confirmDestructive(`${b.action}?`))) return;
    notify("Move to trash — coming soon");
  },
  "photos---catalog-management:delete-rejected-photo-s-": async (b) => {
    if (!(await confirmDestructive(`${b.action}?`))) return;
    notify("Delete rejected — coming soon");
  },
  "photos---catalog-management:rename-photo": () => notify("Rename — coming soon"),
  "photos---catalog-management:headless-enhance": () => notify("Enhance — coming soon"),
  "photos---catalog-management:open-enhance-dialog": () => notify("Enhance — coming soon"),
  "comparing---selecting--library:switch-to-loupe": () => getLibraryKeyboardContext()?.openActive(),
  "comparing---selecting--library:switch-to-grid": (b) => {
    if (b.chord === "Escape") {
      getLibraryKeyboardContext()?.deselectAll();
      return;
    }
    const app = getAppKeyboardContext();
    if (app?.getMode() === "library") {
      getLibraryKeyboardContext()?.toggleGridMode();
    } else {
      app?.setMode("library");
    }
  },
  "comparing---selecting--library:switch-to-compare": () => stub({ action: "Compare" } as ResolvedBinding),
  "comparing---selecting--library:switch-to-survey": () => stub({ action: "Survey" } as ResolvedBinding),
  "comparing---selecting--library:grid-loupe": () => getLibraryKeyboardContext()?.openActive(),
  "comparing---selecting--library:toggle-zoom": () => ui().sendViewCmd("oneToOne"),
  "comparing---selecting--library:zoom-in-out--loupe": (b) =>
    ui().sendViewCmd(b.chord.includes("-") ? "zoomOut" : "zoomIn"),
  "comparing---selecting--library:go-to-start-end-of-grid": (b) =>
    getLibraryKeyboardContext()?.jumpGrid(b.chord === "Home" ? "home" : "end"),
  "comparing---selecting--library:rotate-right--cw": () => notify("Rotate — coming soon"),
  "comparing---selecting--library:rotate-left--ccw": () => notify("Rotate — coming soon"),
  "comparing---selecting--library:increase-decrease-thumbnail-size": (b) =>
    getLibraryKeyboardContext()?.adjustThumbnailSize(b.chord === "-" || b.chord === "=" ? -1 : 1),
  "comparing---selecting--library:select-all": () => getLibraryKeyboardContext()?.selectAll(),
  "comparing---selecting--library:deselect-all": () => getLibraryKeyboardContext()?.deselectAll(),
  "comparing---selecting--library:select-only-active-photo": () =>
    getLibraryKeyboardContext()?.selectOnlyActive(),
  "comparing---selecting--library:add-prev-next-to-selection": (b) =>
    getLibraryKeyboardContext()?.extendSelection(b.chord.endsWith("Left") ? -1 : 1),
  "comparing---selecting--library:select-flagged-photos": () =>
    notify("Select flagged — coming soon"),
  "rating--flagging---filtering:set-star-rating": (b) => {
    const rating = parseInt(b.chord, 10);
    if (rating >= 1 && rating <= 5) {
      void getLibraryKeyboardContext()?.patchSelected({ rating });
    }
  },
  "rating--flagging---filtering:set-rating---advance": (b) => {
    const rating = parseInt(b.chord.replace(/\D/g, ""), 10);
    if (rating >= 1 && rating <= 5) {
      void getLibraryKeyboardContext()?.patchSelected({ rating });
      getLibraryKeyboardContext()?.advanceSelection(1);
    }
  },
  "rating--flagging---filtering:remove-star-rating": () =>
    void getLibraryKeyboardContext()?.patchSelected({ rating: 0 }),
  "rating--flagging---filtering:increase-decrease-rating": (b) =>
    getLibraryKeyboardContext()?.bumpRating(b.chord.endsWith("[") ? -1 : 1),
  "rating--flagging---filtering:flag-as-pick": () =>
    void getLibraryKeyboardContext()?.patchSelected({ flag: "pick" }),
  "rating--flagging---filtering:flag-as-pick---advance": () => {
    void getLibraryKeyboardContext()?.patchSelected({ flag: "pick" });
    getLibraryKeyboardContext()?.advanceSelection(1);
  },
  "rating--flagging---filtering:flag-as-reject": () =>
    void getLibraryKeyboardContext()?.patchSelected({ flag: "reject" }),
  "rating--flagging---filtering:flag-as-reject---advance": () => {
    void getLibraryKeyboardContext()?.patchSelected({ flag: "reject" });
    getLibraryKeyboardContext()?.advanceSelection(1);
  },
  "rating--flagging---filtering:unflag": () =>
    void getLibraryKeyboardContext()?.patchSelected({ flag: "none" }),
  "rating--flagging---filtering:show-hide-filter-bar": () =>
    getLibraryKeyboardContext()?.toggleFilterBar(),
  "rating--flagging---filtering:toggle-filters-on-off": () =>
    getLibraryKeyboardContext()?.toggleFilters(),
  "rating--flagging---filtering:find-photo": () => getLibraryKeyboardContext()?.focusSearch(),
  "metadata---keywords--library:add-keywords": () => {
    getLibraryKeyboardContext()?.focusSearch();
    notify("Add keywords via the info panel");
  },
  "metadata---keywords--library:copy-paste-metadata": () => notify("Copy/paste metadata — coming soon"),
  "metadata---keywords--library:save-metadata-to-file": () => notify("Save metadata — coming soon"),
  "develop-module---global-edit-ops:convert-to-grayscale": async () => {
    const r = reconcile();
    await setParam("color_grade.global_chroma", -100).then(r);
    await setParam("color_grade.perceptual_sat", -100).then(r);
  },
  "develop-module---global-edit-ops:auto-tone": async () => {
    const r = reconcile();
    await applyParamBatch({
      exposure: { stops: 0 },
      tone_curve: { contrast: 8, highlights: -12, shadows: 18, lights: 0, darks: 0 },
      color_grade: { perceptual_sat: 6, global_chroma: 4 },
    }).then(r);
  },
  "develop-module---global-edit-ops:auto-white-balance": () => {
    ui().setTool("wb");
    ui().setRightRailTab("edit");
    notify("Click the image to sample white balance");
  },
  "develop-module---global-edit-ops:copy-paste-develop-settings": () =>
    notify("Copy/paste settings — coming soon"),
  "develop-module---global-edit-ops:paste-settings-from-previous": () =>
    notify("Paste from previous — coming soon"),
  "develop-module---global-edit-ops:nudge-slider--small": (b) =>
    notify(`Nudge slider ${b.chord} — focus a slider first`),
  "develop-module---global-edit-ops:nudge-slider--large": (b) =>
    notify(`Nudge slider ${b.chord} — focus a slider first`),
  "develop-module---global-edit-ops:cycle-basic-panel-settings": () =>
    notify("Cycle sliders — coming soon"),
  "develop-module---global-edit-ops:reset-all-settings": async () => {
    if (!(await confirmDestructive("Reset all develop settings?"))) return;
    notify("Reset all — coming soon");
  },
  "develop-module---global-edit-ops:sync-settings": () => notify("Sync settings — coming soon"),
  "develop-module---global-edit-ops:sync--no-dialog": () => notify("Sync — coming soon"),
  "develop-module---tools:white-balance-tool": () => {
    ui().setTool(ui().tool === "wb" ? "pan" : "wb");
    ui().setRightRailTab("edit");
  },
  "develop-module---tools:crop-tool": () => ui().setRightRailTab("crop"),
  "develop-module---tools:constrain-aspect-ratio--crop": () => notify("Constrain aspect — coming soon"),
  "develop-module---tools:toggle-crop-orientation": () => notify("Crop orientation — coming soon"),
  "develop-module---tools:reset-crop": () => notify("Reset crop — coming soon"),
  "develop-module---tools:spot-removal-tool": () => ui().setRightRailTab("remove"),
  "develop-module---tools:toggle-clone-heal": () => notify("Clone/Heal toggle — coming soon"),
  "develop-module---tools:adjustment-brush": () => {
    ui().setRightRailTab("masking");
    ui().setTool("brush");
  },
  "develop-module---tools:graduated-filter": () => {
    ui().setRightRailTab("masking");
    notify("Graduated filter — coming soon");
  },
  "develop-module---tools:toggle-mask-edit-brush": () => notify("Mask edit/brush — coming soon"),
  "develop-module---tools:increase-decrease-brush-size": (b) => {
    ui().setBrushRadius(ui().brushRadius + (b.chord.endsWith("[") ? -0.01 : 0.01));
  },
  "develop-module---tools:increase-decrease-brush-feather": () => notify("Brush feather — coming soon"),
  "develop-module---tools:switch-brush-a-b": () => notify("Brush A/B — coming soon"),
  "develop-module---tools:show-hide-local-pin": () => notify("Local pins — coming soon"),
  "develop-module---tools:show-hide-mask-overlay": () => notify("Mask overlay — coming soon"),
  "develop-module---targeted-adjustment--tat----masks:show-clipping": () => ui().toggleClipping(),
  "develop-module---targeted-adjustment--tat----masks:toggle-loupe-1-1-zoom": () =>
    ui().sendViewCmd("oneToOne"),
  "develop-module---targeted-adjustment--tat----masks:zoom-in-out": (b) =>
    ui().sendViewCmd(b.chord.includes("-") ? "zoomOut" : "zoomIn"),
  "develop-module---targeted-adjustment--tat----masks:before-after-left-right": () =>
    ui().setBeforeAfter(!ui().beforeAfter),
  "develop-module---targeted-adjustment--tat----masks:view-before-only": () =>
    ui().setBeforeAfter(!ui().beforeAfter),
  "develop-module---targeted-adjustment--tat----masks:new-preset": () => {
    ui().setRightRailTab("presets");
    notify("Type a name in the Presets panel to save");
  },
  "develop-module---targeted-adjustment--tat----masks:new-snapshot": async () => {
    const name = window.prompt("Snapshot name");
    if (name?.trim()) await snapshot(name.trim());
  },
  "develop-module---targeted-adjustment--tat----masks:create-radial-filter": () => {
    ui().setRightRailTab("masking");
    notify("Radial filter — coming soon");
  },
  "develop-module---targeted-adjustment--tat----masks:open-close-masking": () => {
    ui().setRightRailTab(ui().rightRailTab === "masking" ? "edit" : "masking");
  },
  "develop-module---targeted-adjustment--tat----masks:never-show-overlay-pins": () =>
    notify("Hide overlays — coming soon"),
  "help:display-current-module-shortcuts": () => ui().setHelpOverlay(!ui().helpOverlay),
  "help:hide-current-module-shortcuts": () => ui().setHelpOverlay(false),
};

let registered = false;

export function registerAllCommandHandlers() {
  if (registered) return;
  registered = true;

  registerCommandHandlers(HANDLERS);

  // Default stub for any core binding without an explicit handler.
  const handled = new Set(Object.keys(HANDLERS));
  const coreIds = new Set(
    allKeyBindings()
      .filter((b) => b.relevance === "core")
      .map((b) => b.id),
  );
  for (const id of coreIds) {
    if (handled.has(id)) continue;
    registerCommandHandler(id, (b) => stub(b));
  }
}

export { stub };
