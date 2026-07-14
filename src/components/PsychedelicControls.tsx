// Play / rewind / mute / volume for the psychedelic background video.
import { useSyncExternalStore } from "react";
import {
  getPsyMediaState,
  psyRewind,
  psySetVolume,
  psyToggleMute,
  psyTogglePlay,
  subscribePsyMedia,
} from "../theme/psychedelicMedia";
import { Icon } from "./lr/widgets";

export function PsychedelicControls({ compact = false }: { compact?: boolean }) {
  const { playing, muted, volume } = useSyncExternalStore(
    subscribePsyMedia,
    getPsyMediaState,
    getPsyMediaState,
  );

  return (
    <div
      className={`psy-controls ${compact ? "compact" : ""}`}
      role="group"
      aria-label="Background video"
      onMouseDown={(e) => e.stopPropagation()}
    >
      <button
        type="button"
        className="psy-ctrl-btn"
        title={playing ? "Pause" : "Play"}
        aria-label={playing ? "Pause background video" : "Play background video"}
        onClick={() => void psyTogglePlay()}
      >
        {playing ? <Icon.Pause size={14} /> : <Icon.Play size={14} />}
      </button>
      <button
        type="button"
        className="psy-ctrl-btn"
        title="Rewind to start"
        aria-label="Rewind background video"
        onClick={() => psyRewind()}
      >
        <Icon.Rewind size={14} />
      </button>
      <button
        type="button"
        className="psy-ctrl-btn"
        title={muted || volume === 0 ? "Unmute" : "Mute"}
        aria-label={muted || volume === 0 ? "Unmute" : "Mute"}
        onClick={() => psyToggleMute()}
      >
        {muted || volume === 0 ? (
          <Icon.VolumeMute size={14} />
        ) : (
          <Icon.Volume size={14} />
        )}
      </button>
      <label className="psy-vol" title="Volume">
        <span className="sr-only">Volume</span>
        <input
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={muted ? 0 : volume}
          onChange={(e) => psySetVolume(Number(e.target.value))}
        />
      </label>
    </div>
  );
}
