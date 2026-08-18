<!--
  Always-on agent column. Visual language is Pencil's agent pane:
  37px header with a 7px status dot, unboxed assistant prose, a
  sunken rounded composer, and a circular fg-fill send button.
  The assistant itself is the existing Claude edit loop.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import GlassPanel from "../primitives/GlassPanel.svelte";
  import { assistantAvailable, assistantSend } from "../../../ipc/commands";
  import { onAssistantProgress } from "../../../ipc/events";
  import { formatAppError } from "../../../ipc/types";

  let { embedded = false }: { embedded?: boolean } = $props();

  interface Message {
    id: string;
    role: "user" | "assistant";
    text: string;
    error?: boolean;
  }

  let available = $state(false);
  let messages = $state<Message[]>([]);
  let draft = $state("");
  let running = $state(false);
  let phase = $state("");
  let failed = $state(false);
  let scrollEl = $state<HTMLDivElement | null>(null);
  let ta = $state<HTMLTextAreaElement | null>(null);

  const canSend = $derived(available && !running && draft.trim().length > 0);

  const statusLabel = $derived.by(() => {
    if (running) return phase || "Working";
    if (failed) return "Failed";
    if (!available) return "Offline";
    return "Ready";
  });

  const dotClass = $derived.by(() => {
    if (running) return "status-dot status-dot--running";
    if (failed) return "status-dot status-dot--failed";
    return "status-dot";
  });

  const placeholder = $derived(
    messages.length ? "Ask for a follow-up" : "Describe an edit",
  );

  function scrollToBottom() {
    requestAnimationFrame(() => {
      if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
    });
  }

  function resizeTa() {
    if (!ta) return;
    ta.style.height = "auto";
    ta.style.height = `${Math.min(128, ta.scrollHeight)}px`;
  }

  $effect(() => {
    void draft;
    resizeTa();
  });

  onMount(() => {
    void assistantAvailable()
      .then((v) => {
        available = v;
      })
      .catch(() => {
        available = false;
      });

    let unlisten: (() => void) | undefined;
    void onAssistantProgress((p) => {
      if (p.kind === "tool") {
        phase = p.label;
      } else if (p.kind === "text" && running) {
        const last = messages.at(-1);
        if (last?.role === "assistant" && !last.error) {
          last.text = last.text ? last.text + p.label : p.label;
        }
        scrollToBottom();
      } else if (p.kind === "done") {
        phase = "";
      }
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});

    const focus = () => ta?.focus({ preventScroll: true });
    window.addEventListener("meraraw:focus-agent", focus);
    return () => {
      unlisten?.();
      window.removeEventListener("meraraw:focus-agent", focus);
    };
  });

  async function send() {
    const text = draft.trim();
    if (!text || running || !available) return;
    draft = "";
    failed = false;
    running = true;
    phase = "Looking…";
    const userId = crypto.randomUUID();
    const asstId = crypto.randomUUID();
    messages.push({ id: userId, role: "user", text });
    messages.push({ id: asstId, role: "assistant", text: "" });
    scrollToBottom();
    resizeTa();

    const asst = () => messages.find((m) => m.id === asstId);

    try {
      const reply = await assistantSend(text, "edit");
      const msg = asst();
      if (msg) msg.text = reply.trim() || msg.text || "Done.";
    } catch (e) {
      failed = true;
      const err = formatAppError(e);
      const msg = asst();
      if (msg && !msg.text) {
        msg.text = err;
        msg.error = true;
      } else {
        messages.push({
          id: crypto.randomUUID(),
          role: "assistant",
          text: err,
          error: true,
        });
      }
    } finally {
      running = false;
      phase = "";
      scrollToBottom();
      ta?.focus({ preventScroll: true });
    }
  }

  function onComposerKey(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      void send();
    }
  }

  function newThread() {
    if (running) return;
    messages = [];
    failed = false;
    phase = "";
    ta?.focus({ preventScroll: true });
  }
  function useSuggestion(text: string) {
    draft = text;
    ta?.focus({ preventScroll: true });
  }

  const suggestions = [
    "Make it cinematic",
    "Fix the exposure",
    "Warm up the colors",
    "Enhance the subject",
  ];
</script>

<div class="agent-shell">
  {#if embedded}
    {@render body()}
  {:else}
    <GlassPanel class="flex h-full min-h-0 flex-col overflow-hidden">
      {@render body()}
    </GlassPanel>
  {/if}
</div>

{#snippet body()}
  <header class="agent-header">
    <span class={dotClass} aria-hidden="true"></span>
    <span class="title">Agent</span>
    <span class="status-label" title={statusLabel}>{statusLabel}</span>
    <span class="model">{available ? "Sonnet" : "—"}</span>
    <button
      type="button"
      class="agent-new"
      onclick={newThread}
      disabled={running || messages.length === 0}
      title="New thread"
      aria-label="New thread"
    >
      New
    </button>
  </header>

  <div bind:this={scrollEl} class="agent-thread" role="log" aria-live="polite">
    {#if messages.length === 0}
      <div class="agent-empty">
        <p class="empty-title">What would you like to change?</p>
        <p class="empty-copy">
          {available
            ? "Describe an edit in natural language."
            : "The assistant needs an API key to run."}
        </p>
        {#if available}
          <div class="suggest">
            {#each suggestions as s}
              <button type="button" class="suggest-chip" onclick={() => useSuggestion(s)}>{s}</button>
            {/each}
          </div>
        {/if}
      </div>
    {:else}
      {#each messages as msg, i (msg.id)}
        <div class="msg" class:is-user={msg.role === "user"} class:is-assistant={msg.role === "assistant"}>
          <div class="msg-body" class:is-error={msg.error}>
            {msg.text}
            {#if msg.role === "assistant" && running && i === messages.length - 1 && !msg.text}
              <span class="msg-pending">Working…</span>
            {/if}
          </div>
        </div>
      {/each}
    {/if}
  </div>

  <form
    class="composer-wrap"
    onsubmit={(e) => {
      e.preventDefault();
      void send();
    }}
  >
    <div class="composer">
      <textarea
        bind:this={ta}
        bind:value={draft}
        rows="1"
        {placeholder}
        disabled={!available || running}
        onkeydown={onComposerKey}
        aria-label="Agent message"
      ></textarea>
      <div class="composer-bar">
        <span class="composer-hint">{running ? (phase || "Working") : "↵ send"}</span>
        <button
          type="submit"
          class="composer-send"
          disabled={!canSend}
          aria-label={running ? "Working" : "Send"}
        >
          {#if running}
            <span class="mr-spinner"></span>
          {:else}
            <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M2 6H10M10 6L6.5 2.5M10 6L6.5 9.5" />
            </svg>
          {/if}
        </button>
      </div>
    </div>
  </form>
{/snippet}

<style>
  .agent-shell {
    display: flex;
    flex-direction: column;
    flex: 1;
    height: 100%;
    min-height: 0;
  }
  .agent-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 24px 16px;
    text-align: center;
  }
  .empty-title {
    margin: 0;
    font-size: 14px;
    font-weight: 500;
    color: var(--color-fg);
  }
  .empty-copy {
    margin: 0;
    font-size: 12px;
    color: var(--color-subtle);
  }
  .suggest {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 6px;
    margin-top: 10px;
  }
  .suggest-chip {
    padding: 4px 10px;
    border: 1px solid var(--color-border);
    border-radius: 999px;
    background: transparent;
    color: var(--color-secondary);
    font-size: 11px;
    cursor: pointer;
  }
  .suggest-chip:hover {
    background: var(--color-hover);
    color: var(--color-fg);
  }

  .agent-thread {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px 14px 8px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .agent-thread > :global(.empty-state) {
    flex: 1;
  }

  .msg {
    display: flex;
    width: 100%;
    max-width: 95%;
    flex-direction: column;
  }
  .msg.is-user {
    margin-left: auto;
    align-items: flex-end;
  }
  .msg.is-assistant {
    align-items: flex-start;
  }

  .msg-body {
    font-size: 13px;
    line-height: 1.55;
    color: var(--color-fg);
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
    -webkit-user-select: text;
  }
  .msg.is-user .msg-body {
    width: fit-content;
    max-width: 100%;
    padding: 10px 14px;
    border-radius: 8px;
    background: var(--color-sunken);
  }
  .msg-body.is-error {
    color: var(--color-destructive);
    font-size: 11px;
  }
  .msg-pending {
    color: var(--color-subtle);
    font-size: 11px;
  }

  .agent-new {
    flex: none;
    height: 22px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: 5px;
    background: transparent;
    color: var(--color-subtle);
    font-size: 11px;
    cursor: pointer;
    transition: background-color 0.15s var(--ease-std), color 0.15s var(--ease-std);
  }
  .agent-new:hover:not(:disabled) {
    background: var(--color-hover);
    color: var(--color-fg);
  }
  .agent-new:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .composer-wrap {
    flex: none;
    padding: 4px 12px 12px;
  }
  .composer {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    background: var(--color-bg);
    transition: border-color 0.15s var(--ease-std);
  }
  .composer:focus-within {
    border-color: var(--color-border-strong);
  }
  .composer textarea {
    width: 100%;
    resize: none;
    overflow-y: auto;
    padding: 10px 12px 4px;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--color-fg);
    font: inherit;
    font-size: 13px;
    line-height: 1.4;
  }
  .composer textarea::placeholder {
    color: var(--color-subtle);
  }
  .composer textarea:disabled {
    opacity: 0.55;
  }

  .composer-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 4px 6px 6px 12px;
  }
  .composer-hint {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--color-subtle);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .composer-send {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 999px;
    background: var(--color-fg);
    color: var(--color-panel);
    cursor: pointer;
    transition: opacity 0.15s var(--ease-std);
  }
  .composer-send:hover:not(:disabled) {
    opacity: 0.88;
  }
  .composer-send:disabled {
    opacity: 0.3;
    cursor: default;
  }
</style>
