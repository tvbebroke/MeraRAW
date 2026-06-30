import { useEffect, useMemo, useState } from "react";
import {
  applyPreset,
  listPresetCatalog,
  PRESET_TAGS,
  savePresetNamed,
  type PresetCatalogEntry,
} from "../../ipc/commands";
import { useDocStore } from "../../state/docStore";

const DEVELOP_MODULES = [
  "exposure",
  "white_balance",
  "calibration",
  "detail",
  "color_grade",
  "hsl",
  "tone_curve",
];

function matchesQuery(preset: PresetCatalogEntry, query: string, activeTag: string | null): boolean {
  if (activeTag && !preset.tags.includes(activeTag)) return false;
  const q = query.trim().toLowerCase();
  if (!q) return true;
  if (preset.label.toLowerCase().includes(q)) return true;
  if (preset.id.toLowerCase().includes(q)) return true;
  return preset.tags.some((t) => t.toLowerCase().includes(q));
}

export function PresetsPanel() {
  const [presets, setPresets] = useState<PresetCatalogEntry[]>([]);
  const [query, setQuery] = useState("");
  const [activeTag, setActiveTag] = useState<string | null>(null);
  const [name, setName] = useState("");
  const reconcile = useDocStore((s) => s.reconcile);
  const refresh = () =>
    listPresetCatalog()
      .then((p) => setPresets(p))
      .catch(() => {});

  useEffect(() => {
    void refresh();
  }, []);

  const filtered = useMemo(
    () => presets.filter((p) => matchesQuery(p, query, activeTag)),
    [presets, query, activeTag],
  );

  const usedTags = useMemo(() => {
    const set = new Set<string>();
    for (const p of presets) for (const t of p.tags) set.add(t);
    return PRESET_TAGS.filter((t) => set.has(t));
  }, [presets]);

  return (
    <div className="lr-rail-panel">
      <header className="lr-rail-panel-head">
        <h2>Presets</h2>
      </header>
      <div className="lr-rail-panel-body">
        <div className="lr-list-panel">
          <input
            className="lr-preset-search"
            placeholder="Search presets or tags…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          {usedTags.length > 0 && (
            <div className="lr-preset-tag-filters">
              <button
                type="button"
                className={`lr-preset-tag-filter${activeTag === null ? " is-active" : ""}`}
                onClick={() => setActiveTag(null)}
              >
                All
              </button>
              {usedTags.map((tag) => (
                <button
                  key={tag}
                  type="button"
                  className={`lr-preset-tag-filter${activeTag === tag ? " is-active" : ""}`}
                  onClick={() => setActiveTag((cur) => (cur === tag ? null : tag))}
                >
                  {tag}
                </button>
              ))}
            </div>
          )}
          <div className="muted sm" style={{ marginBottom: 6 }}>
            {query.trim() || activeTag
              ? `Showing ${filtered.length} of ${presets.length}`
              : `${presets.length} preset${presets.length === 1 ? "" : "s"}`}
          </div>
          {presets.length === 0 && (
            <div className="muted sm">No presets yet — save your current look below.</div>
          )}
          {presets.length > 0 && filtered.length === 0 && (
            <div className="muted sm">No presets match your search.</div>
          )}
          {filtered.map((p) => (
            <button
              key={p.id}
              type="button"
              className="lr-list-row lr-preset-row"
              onClick={() => applyPreset(p.id).then(reconcile).catch(() => {})}
            >
              <span className="lr-preset-label">{p.label}</span>
              {p.tags.length > 0 && (
                <span className="lr-preset-tags">
                  {p.tags.map((tag) => (
                    <span
                      key={tag}
                      className="lr-preset-tag"
                      onClick={(e) => {
                        e.stopPropagation();
                        setActiveTag(tag);
                        setQuery("");
                      }}
                      role="presentation"
                    >
                      {tag}
                    </span>
                  ))}
                </span>
              )}
            </button>
          ))}
          <div className="lr-add-row">
            <input
              placeholder="Save preset as…"
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && name.trim()) {
                  savePresetNamed(name.trim(), DEVELOP_MODULES)
                    .then(() => {
                      setName("");
                      void refresh();
                    })
                    .catch(() => {});
                }
              }}
            />
          </div>
        </div>
      </div>
    </div>
  );
}
