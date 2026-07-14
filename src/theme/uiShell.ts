/** App chrome shell: Modern (Figma bento) vs Faithful (Lightroom Classic). */

export type UiShell = "modern" | "faithful";

export const UI_SHELL_OPTIONS: {
  id: UiShell;
  label: string;
  hint: string;
}[] = [
  {
    id: "modern",
    label: "Modern",
    hint: "Figma-aligned bento chrome, Zen mode, backdrop themes",
  },
  {
    id: "faithful",
    label: "Faithful",
    hint: "Lightroom Classic–style dense dark panels (v0.1.4)",
  },
];

const STORAGE_KEY = "meraraw.uiShell";

export function isUiShell(v: unknown): v is UiShell {
  return v === "modern" || v === "faithful";
}

export function readStoredUiShell(): UiShell {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (isUiShell(v)) return v;
  } catch {
    /* ignore */
  }
  return "modern";
}

export function persistUiShell(shell: UiShell): void {
  try {
    localStorage.setItem(STORAGE_KEY, shell);
  } catch {
    /* ignore */
  }
}

/** Apply shell before first paint / on change. */
export function applyUiShellAttr(shell: UiShell): void {
  document.documentElement.setAttribute("data-ui-shell", shell);
}
