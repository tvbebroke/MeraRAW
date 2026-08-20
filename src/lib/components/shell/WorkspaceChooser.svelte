<script lang="ts">
  import { workspaceChosen, setWorkspace } from "../../../stores/workspace";
  import { licenseStatus } from "../../../stores/session";
  import logoIcon from "../../icons/logo.png";

  const inTauri = typeof window !== "undefined" && Boolean((window as any).__TAURI_INTERNALS__);
  const blocked = $derived(
    inTauri && $licenseStatus !== null && $licenseStatus.licensed === false,
  );
  const show = $derived(!$workspaceChosen && !blocked);
</script>

{#if show}
  <div class="gate" role="dialog" aria-modal="true" aria-labelledby="ws-title">
    <div class="card">
      <img src={logoIcon} alt="" class="logo" width="28" height="28" />
      <h1 id="ws-title">Choose an editor</h1>
      <p class="lede">
        Same color engine. Two workspaces. You can switch anytime from the title bar.
      </p>
      <div class="choices">
        <button type="button" class="choice" onclick={() => setWorkspace("photo")}>
          <span class="choice-kicker">Original MeraRAW</span>
          <span class="choice-title">Photo</span>
          <span class="choice-copy">
            RAW stills, DCP, demosaic, crop, masks, retouch, and export. The stills editor is unchanged.
          </span>
        </button>
        <button type="button" class="choice" onclick={() => setWorkspace("video")}>
          <span class="choice-kicker">Colorist</span>
          <span class="choice-title">Video</span>
          <span class="choice-copy">
            Grade clips frame-by-frame with Looks, wheels, scopes, and silent H.264 export. No timeline.
          </span>
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .gate {
    position: absolute;
    inset: 0;
    z-index: 90;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.55);
    padding: 24px;
  }
  .card {
    width: min(560px, 100%);
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 28px 28px 24px;
    border: 1px solid var(--color-border-strong);
    border-radius: 12px;
    background: var(--color-panel);
  }
  .logo {
    width: 28px;
    height: 28px;
    border-radius: 6px;
  }
  h1 {
    margin: 4px 0 0;
    font-size: 18px;
    font-weight: 560;
    color: var(--color-fg);
  }
  .lede {
    margin: 0;
    font-size: var(--text-ui);
    line-height: 1.45;
    color: var(--color-subtle);
  }
  .choices {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin-top: 8px;
  }
  .choice {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    text-align: left;
    padding: 14px;
    border: 1px solid var(--color-border-strong);
    border-radius: 10px;
    background: var(--color-sunken);
    color: var(--color-fg);
    cursor: pointer;
  }
  .choice:hover {
    border-color: var(--color-fg);
    background: var(--color-hover);
  }
  .choice-kicker {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--color-subtle);
  }
  .choice-title {
    font-size: 16px;
    font-weight: 560;
  }
  .choice-copy {
    font-size: 12px;
    line-height: 1.4;
    color: var(--color-subtle);
  }
</style>
