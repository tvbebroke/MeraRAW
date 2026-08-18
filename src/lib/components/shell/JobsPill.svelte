<!--
  Background-task pill + popover.

  A faithful port of Pencil's jobs surface: a quiet outlined pill in the header
  that only speaks up when there is something to say, opening a popover of rows
  with a status dot, a hairline progress bar, and — crucially — errors that are
  inline, selectable and copyable rather than thrown into a modal.
-->
<script lang="ts">
  import { jobs, activeJobs, failedJobs, dismissJob, clearFinished, type Job } from "../../../stores/jobs";

  let open = $state(false);
  let expanded = $state<Record<string, boolean>>({});
  let copied = $state<string | null>(null);

  const running = $derived($activeJobs.length);
  const failed = $derived($failedJobs.length);
  // Idle with nothing worth reporting → the pill disappears entirely.
  const visible = $derived($jobs.length > 0);

  const label = $derived(
    failed > 0
      ? `${failed} failed`
      : running > 0
        ? `${running} task${running === 1 ? "" : "s"}`
        : "Tasks",
  );

  function dotClass(j: Job): string {
    if (j.status === "failed") return "status-dot status-dot--failed";
    if (j.status === "ok") return "status-dot status-dot--ok";
    if (j.status === "running") return "status-dot status-dot--running";
    return "status-dot";
  }

  async function copyError(j: Job) {
    if (!j.error) return;
    try {
      await navigator.clipboard.writeText(j.error);
      copied = j.id;
      setTimeout(() => (copied = null), 1400);
    } catch {
      /* clipboard unavailable */
    }
  }

  function onWindowKey(e: KeyboardEvent) {
    if (e.key === "Escape" && open) {
      e.stopPropagation();
      open = false;
    }
  }
</script>

<svelte:window onkeydown={onWindowKey} />

{#if visible}
  <div class="jobs-wrap">
    <button
      class="jobs-pill {failed > 0 ? 'jobs-pill--failed' : ''} {running === 0 && failed === 0 ? 'jobs-pill--quiet' : ''}"
      onclick={() => (open = !open)}
      aria-expanded={open}
      aria-label="Background tasks"
    >
      {#if running > 0}
        <span class="mr-spinner"></span>
      {:else}
        <span class={failed > 0 ? "status-dot status-dot--failed" : "status-dot status-dot--ok"}></span>
      {/if}
      <span>{label}</span>
    </button>

    {#if open}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div class="jobs-scrim" onclick={() => (open = false)}></div>
      <div class="jobs-popover" role="dialog" aria-label="Background tasks">
        <div class="jobs-head">
          <span class="eyebrow">Background tasks</span>
          {#if $jobs.some((j) => j.status === "ok" || j.status === "failed")}
            <button class="mini-button" onclick={clearFinished}>Clear finished</button>
          {/if}
        </div>

        {#each $jobs as job (job.id)}
          <div class="job-row">
            <div class="job-top">
              {#if job.status === "running"}
                <span class="mr-spinner"></span>
              {:else}
                <span class={dotClass(job)}></span>
              {/if}
              <span class="job-label">{job.label}</span>
              <span class="job-status {job.status === 'failed' ? 'is-failed' : ''}">
                {#if job.status === "failed"}Failed
                {:else if job.status === "ok"}Done
                {:else if job.detail}<span class="num">{job.detail}</span>
                {:else}Working{/if}
              </span>
              <button class="job-dismiss" onclick={() => dismissJob(job.id)} aria-label="Dismiss">×</button>
            </div>

            {#if job.phase && job.status === "running"}
              <div class="job-phase">{job.phase}</div>
            {/if}

            {#if job.status === "running" && job.pct != null}
              <div class="mr-progress"><span style="width: {job.pct}%"></span></div>
            {/if}

            {#if job.error}
              <div class="error-text">{job.error.split("\n")[0]}</div>
              {#if expanded[job.id]}
                <pre class="error-detail">{job.error}</pre>
              {/if}
              <div class="job-actions">
                <button class="mini-button" onclick={() => (expanded[job.id] = !expanded[job.id])}>
                  {expanded[job.id] ? "Hide details" : "Details"}
                </button>
                <button class="mini-button" onclick={() => copyError(job)}>
                  {copied === job.id ? "Copied" : "Copy error"}
                </button>
                {#if job.retry}
                  <button class="mini-button" onclick={() => job.retry?.()}>Retry</button>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .jobs-wrap {
    position: relative;
    flex: none;
  }

  .jobs-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    max-width: 200px;
    padding: 2px 8px;
    border: 1px solid var(--color-border);
    border-radius: 999px;
    color: var(--color-secondary);
    font-size: 11px;
    cursor: pointer;
    background: transparent;
    transition: background-color 0.15s var(--ease-std), color 0.15s var(--ease-std);
  }
  .jobs-pill:hover { background: var(--color-hover); color: var(--color-fg); }
  .jobs-pill--failed {
    color: var(--color-destructive);
    border-color: color-mix(in srgb, var(--color-destructive) 35%, transparent);
  }
  .jobs-pill--quiet { color: var(--color-subtle); }

  .jobs-scrim { position: fixed; inset: 0; z-index: 39; }

  .jobs-popover {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 40;
    width: 330px;
    max-height: 420px;
    overflow-y: auto;
    padding: 6px;
    background: var(--color-panel);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-popover);
  }

  .jobs-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 4px 6px 6px;
  }

  .job-row { padding: 8px 9px; border-radius: 7px; }
  .job-row:hover { background: var(--color-hover); }

  .job-top { display: flex; align-items: center; gap: 7px; }
  .job-label {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    color: var(--color-fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .job-status { flex: none; font-size: 10.5px; color: var(--color-subtle); }
  .job-status.is-failed { color: var(--color-destructive); }

  .job-dismiss {
    flex: none;
    width: 16px;
    height: 16px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--color-subtle);
    font-size: 13px;
    line-height: 1;
    cursor: pointer;
  }
  .job-dismiss:hover { background: var(--color-active); color: var(--color-fg); }

  .job-phase { margin-top: 2px; font-size: 11px; color: var(--color-subtle); }
  .mr-progress { margin-top: 6px; }
  .job-actions { display: flex; gap: 6px; margin-top: 6px; }

  .mini-button {
    padding: 2px 8px;
    border: 1px solid var(--color-border);
    border-radius: 5px;
    background: transparent;
    color: var(--color-secondary);
    font-size: 10.5px;
    cursor: pointer;
    transition: background-color 0.15s var(--ease-std), color 0.15s var(--ease-std);
  }
  .mini-button:hover { background: var(--color-hover); color: var(--color-fg); }
</style>
