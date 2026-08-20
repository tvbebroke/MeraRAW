<script lang="ts">
  import { seekVideo } from "../../../ipc/commands";
  import { imageMeta, statusMessage, videoMark } from "../../../stores/app";

  const v = $derived($imageMeta?.video ?? null);
  const isVideo = $derived($imageMeta?.kind === "video" && !!v);

  let playing = $state(false);
  let reverse = $state(false);
  let looping = $state(true);
  let busy = $state(false);
  let inFrame = $state(0);
  let outFrame = $state(0);
  let raf = 0;
  let originTs = 0;
  let originFrame = 0;

  function syncMarks() {
    videoMark.set({ inFrame, outFrame });
  }

  let lastPath = "";
  $effect(() => {
    const p = $imageMeta?.path ?? "";
    const meta = $imageMeta?.video;
    if (!p || !meta) return;
    if (p === lastPath) return;
    lastPath = p;
    inFrame = meta.inFrame ?? 0;
    outFrame = meta.outFrame ?? Math.max(0, (meta.frameCount ?? 1) - 1);
    syncMarks();
  });

  function span(): number {
    return Math.max(1, outFrame - inFrame + 1);
  }

  function desiredAt(now: number, fps: number): number {
    const delta = ((now - originTs) / 1000) * Math.max(fps, 1);
    const loop = span();
    if (!reverse) {
      let f = originFrame + delta;
      if (f > outFrame) {
        if (!looping) return outFrame;
        f = inFrame + (((f - inFrame) % loop) + loop) % loop;
      }
      return Math.round(f);
    }
    let f = originFrame - delta;
    if (f < inFrame) {
      if (!looping) return inFrame;
      f = outFrame - (((outFrame - f) % loop) + loop) % loop;
    }
    return Math.round(f);
  }

  function startClock() {
    originTs = performance.now();
    originFrame = v?.frame ?? 0;
  }

  async function go(frame: number) {
    if (!v || busy) return;
    const max = Math.max(0, v.frameCount - 1);
    const f = Math.max(0, Math.min(max, Math.round(frame)));
    busy = true;
    try {
      const meta = await seekVideo(f);
      imageMeta.set(meta);
    } catch (e) {
      statusMessage.set(e instanceof Error ? e.message : String(e));
      playing = false;
    } finally {
      busy = false;
    }
  }

  function tick(ts: number) {
    if (!playing || !v) return;
    const fps = v.fps || 24;
    let want = Math.max(inFrame, Math.min(outFrame, desiredAt(ts, fps)));
    const shown = v.frame ?? 0;
    const wrappedFwd = looping && !reverse && shown >= outFrame - 1 && want <= inFrame + 1;
    const wrappedRev = looping && reverse && shown <= inFrame + 1 && want >= outFrame - 1;
    if (!reverse && want < shown && !wrappedFwd) want = shown;
    if (reverse && want > shown && !wrappedRev) want = shown;
    if (!looping && !reverse && shown >= outFrame && want >= outFrame) {
      playing = false;
      return;
    }
    if (!looping && reverse && shown <= inFrame && want <= inFrame) {
      playing = false;
      return;
    }
    if (want !== shown && !busy) void go(want);
    raf = requestAnimationFrame(tick);
  }

  function togglePlay() {
    playing = !playing;
    if (playing) {
      startClock();
      raf = requestAnimationFrame(tick);
    } else {
      cancelAnimationFrame(raf);
    }
  }

  function playForward() {
    reverse = false;
    if (!playing) togglePlay();
    else startClock();
  }

  function playReverse() {
    reverse = true;
    if (!playing) togglePlay();
    else startClock();
  }

  function pause() {
    if (playing) togglePlay();
  }

  $effect(() => {
    const onPlay = () => {
      if (playing) pause();
      else playForward();
    };
    const onReverse = () => playReverse();
    const onPause = () => pause();
    const onStep = (e: Event) => {
      const dir = Number((e as CustomEvent<number>).detail ?? 1);
      if (!v) return;
      void go((v.frame ?? 0) + dir);
    };
    const onIn = () => {
      if (!v) return;
      inFrame = v.frame;
      syncMarks();
    };
    const onOut = () => {
      if (!v) return;
      outFrame = v.frame;
      syncMarks();
    };
    const onLoop = () => {
      looping = !looping;
    };
    window.addEventListener("meraraw:video-play", onPlay);
    window.addEventListener("meraraw:video-reverse", onReverse);
    window.addEventListener("meraraw:video-pause", onPause);
    window.addEventListener("meraraw:video-step", onStep);
    window.addEventListener("meraraw:mark-in", onIn);
    window.addEventListener("meraraw:mark-out", onOut);
    window.addEventListener("meraraw:video-loop", onLoop);
    return () => {
      playing = false;
      cancelAnimationFrame(raf);
      window.removeEventListener("meraraw:video-play", onPlay);
      window.removeEventListener("meraraw:video-reverse", onReverse);
      window.removeEventListener("meraraw:video-pause", onPause);
      window.removeEventListener("meraraw:video-step", onStep);
      window.removeEventListener("meraraw:mark-in", onIn);
      window.removeEventListener("meraraw:mark-out", onOut);
      window.removeEventListener("meraraw:video-loop", onLoop);
    };
  });

  function fmtTime(frame: number, fps: number): string {
    const s = frame / Math.max(fps, 1);
    const m = Math.floor(s / 60);
    const r = s - m * 60;
    return `${m}:${r.toFixed(2).padStart(5, "0")}`;
  }
</script>

{#if isVideo && v}
  <div class="scrub">
    <button type="button" class="tb-btn" onclick={() => (playing ? pause() : playForward())}>
      {playing ? "Pause" : "Play"}
    </button>
    <button
      type="button"
      class="tb-btn"
      class:is-on={playing && reverse}
      onclick={playReverse}
      title="Reverse play (J)"
    >Rev</button>
    <button
      type="button"
      class="tb-btn"
      class:is-on={looping}
      onclick={() => (looping = !looping)}
      title="Loop playback between In and Out"
    >Loop</button>
    <input
      class="bar"
      type="range"
      min="0"
      max={Math.max(0, v.frameCount - 1)}
      step="1"
      value={v.frame}
      oninput={(e) => {
        if (playing) pause();
        void go(Number((e.currentTarget as HTMLInputElement).value));
      }}
    />
    <span class="num">{v.frame} / {Math.max(0, v.frameCount - 1)}</span>
    <span class="num">{fmtTime(v.frame, v.fps)} · {v.fps.toFixed(2)} fps</span>
    <button
      type="button"
      class="tb-btn"
      onclick={() => {
        inFrame = v.frame;
        syncMarks();
      }}>In</button>
    <button
      type="button"
      class="tb-btn"
      onclick={() => {
        outFrame = v.frame;
        syncMarks();
      }}>Out</button>
    <span class="num">{inFrame}–{outFrame}</span>
  </div>
{/if}

<style>
  .scrub {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    flex: none;
    font-size: 11px;
    border-top: 1px solid var(--color-border-strong);
  }
  .bar {
    flex: 1;
    min-width: 80px;
    accent-color: var(--color-fg);
  }
  .num {
    font-variant-numeric: tabular-nums;
    color: var(--color-muted, #aaa);
    white-space: nowrap;
  }
</style>
