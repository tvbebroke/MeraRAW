import { useMemo } from "react";
import { isMacOs } from "../keyboard/chord";
import { bindingsForScopes } from "../keyboard/dispatcher";
import { allKeyBindings } from "../keyboard/loadKeymap";
import type { KeymapScope, ResolvedBinding } from "../keyboard/types";

export function KeyboardHelpOverlay({
  open,
  scopes,
  onClose,
}: {
  open: boolean;
  scopes: KeymapScope[];
  onClose: () => void;
}) {
  const bindings = useMemo(() => {
    if (!open) return [];
    return bindingsForScopes(scopes, allKeyBindings()).filter(
      (b) => b.relevance === "core" || b.relevance === "optional",
    );
  }, [open, scopes]);

  if (!open) return null;

  const grouped = groupBySection(bindings);

  return (
    <div className="kb-help-backdrop" onClick={onClose} role="presentation">
      <div
        className="kb-help-panel"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-label="Keyboard shortcuts"
      >
        <header className="kb-help-head">
          <h2>Keyboard shortcuts</h2>
          <p className="muted sm">
            {isMacOs() ? "macOS" : "Windows"} · active context ·{" "}
            <kbd>{isMacOs() ? "⌘" : "Ctrl"}+/</kbd> to close
          </p>
          <button type="button" className="kb-help-close" onClick={onClose}>
            ×
          </button>
        </header>
        <div className="kb-help-body">
          {grouped.map(([section, rows]) => (
            <section key={section} className="kb-help-section">
              <h3>{section}</h3>
              <table>
                <tbody>
                  {rows.map((b) => (
                    <tr key={`${b.id}|${b.chord}`}>
                      <td className="kb-help-keys">
                        <kbd>{formatChord(b.chord)}</kbd>
                      </td>
                      <td>{b.action}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          ))}
        </div>
      </div>
    </div>
  );
}

function groupBySection(bindings: ResolvedBinding[]): [string, ResolvedBinding[]][] {
  const map = new Map<string, ResolvedBinding[]>();
  for (const b of bindings) {
    const list = map.get(b.section) ?? [];
    list.push(b);
    map.set(b.section, list);
  }
  return [...map.entries()];
}

function formatChord(chord: string): string {
  return chord
    .replace(/Cmd/g, "⌘")
    .replace(/Alt/g, "⌥")
    .replace(/Shift/g, "⇧")
    .replace(/Ctrl/g, "⌃")
    .replace(/\+/g, " ");
}
