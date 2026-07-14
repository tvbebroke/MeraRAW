// Full-window looping video backdrop. Starts paused — use PsychedelicControls.
import { useEffect, useRef } from "react";
import {
  bindPsyVideo,
  psyOnPause,
  psyOnPlay,
} from "../theme/psychedelicMedia";

const SRC = "/backgrounds/psychedelic.mp4";

export function PsychedelicBg() {
  const ref = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    bindPsyVideo(el);
    const onPlay = () => psyOnPlay();
    const onPause = () => psyOnPause();
    el.addEventListener("play", onPlay);
    el.addEventListener("pause", onPause);

    return () => {
      el.removeEventListener("play", onPlay);
      el.removeEventListener("pause", onPause);
      el.pause();
      bindPsyVideo(null);
    };
  }, []);

  return (
    <video
      ref={ref}
      className="psy-bg-video"
      src={SRC}
      loop
      playsInline
      preload="metadata"
      muted
      aria-hidden="true"
    />
  );
}
