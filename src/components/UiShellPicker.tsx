import { useUiStore } from "../state/uiStore";
import { UI_SHELL_OPTIONS, type UiShell } from "../theme/uiShell";

export function UiShellSettings() {
  const uiShell = useUiStore((s) => s.uiShell);
  const setUiShell = useUiStore((s) => s.setUiShell);

  return (
    <section className="settings-section">
      <h3 className="settings-h">Interface</h3>
      <p className="muted sm">
        Choose the app chrome. Modern is the current Figma look; Faithful matches
        Lightroom Classic–style panels from v0.1.4.
      </p>
      <div className="ui-shell-picker" role="radiogroup" aria-label="Interface style">
        {UI_SHELL_OPTIONS.map((opt) => (
          <button
            key={opt.id}
            type="button"
            role="radio"
            aria-checked={uiShell === opt.id}
            className={`ui-shell-option ${uiShell === opt.id ? "active" : ""}`}
            onClick={() => setUiShell(opt.id as UiShell)}
          >
            <span className="ui-shell-option-label">{opt.label}</span>
            <span className="ui-shell-option-hint muted xs">{opt.hint}</span>
          </button>
        ))}
      </div>
    </section>
  );
}
