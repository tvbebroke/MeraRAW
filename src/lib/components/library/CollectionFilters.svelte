<script lang="ts">
  import {
    applyLibraryFilters,
    clearLibraryFilters,
    libraryFilters,
    libraryFiltersActive,
    type LibraryFlag,
  } from "../../../stores/browse";

  const f = $derived($libraryFilters);
  const active = $derived(libraryFiltersActive(f));

  async function setRating(n: number) {
    await applyLibraryFilters({ ratingMin: f.ratingMin === n ? 0 : n });
  }

  async function toggleFlag(flag: Exclude<LibraryFlag, "any">) {
    await applyLibraryFilters({ flag: f.flag === flag ? "any" : flag });
  }

  async function toggleEdited() {
    await applyLibraryFilters({ hasEdits: f.hasEdits === true ? null : true });
  }

  async function toggleBlurry() {
    await applyLibraryFilters({ blurryOnly: !f.blurryOnly });
  }

  async function toggleDupes() {
    await applyLibraryFilters({ dupesOnly: !f.dupesOnly });
  }
</script>

<div class="filters" role="toolbar" aria-label="Collection filters">
  <div class="stars" role="group" aria-label="Minimum rating">
    {#each [1, 2, 3, 4, 5] as n}
      <button
        type="button"
        class="star {f.ratingMin >= n ? 'is-on' : ''}"
        aria-pressed={f.ratingMin >= n}
        aria-label="Rated {n} or higher"
        title="Rated {n} or higher"
        onclick={() => void setRating(n)}
      >★</button>
    {/each}
  </div>

  <button
    type="button"
    class="chip {f.flag === 'pick' ? 'is-pick' : ''}"
    aria-pressed={f.flag === "pick"}
    onclick={() => void toggleFlag("pick")}
  >Pick</button>
  <button
    type="button"
    class="chip {f.flag === 'reject' ? 'is-reject' : ''}"
    aria-pressed={f.flag === "reject"}
    onclick={() => void toggleFlag("reject")}
  >Reject</button>
  <button
    type="button"
    class="chip {f.hasEdits === true ? 'is-on' : ''}"
    aria-pressed={f.hasEdits === true}
    title="Only photos with edits"
    onclick={() => void toggleEdited()}
  >Edited</button>
  <button
    type="button"
    class="chip {f.blurryOnly ? 'is-on' : ''}"
    aria-pressed={f.blurryOnly}
    title="Likely blurry (catalog score)"
    onclick={() => void toggleBlurry()}
  >Blurry</button>
  <button
    type="button"
    class="chip {f.dupesOnly ? 'is-on' : ''}"
    aria-pressed={f.dupesOnly}
    title="Near-duplicate hashes"
    onclick={() => void toggleDupes()}
  >Dupes</button>

  {#if active}
    <button type="button" class="clear" onclick={() => void clearLibraryFilters()}>
      Clear
    </button>
  {/if}
</div>

<style>
  .filters {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    min-width: 0;
  }

  .stars {
    display: flex;
    align-items: center;
    gap: 0;
  }

  .star {
    appearance: none;
    border: none;
    background: transparent;
    color: var(--color-subtle);
    font-size: 13px;
    line-height: 1;
    padding: 2px 1px;
    cursor: pointer;
  }
  .star.is-on { color: rgba(255, 210, 120, 0.95); }
  .star:hover { color: rgba(255, 220, 140, 0.9); }

  .chip {
    appearance: none;
    border: 1px solid var(--color-border-strong);
    background: var(--color-hover);
    color: var(--color-secondary);
    font-size: 10px;
    font-weight: 600;
    padding: 5px 8px;
    border-radius: 8px;
    cursor: pointer;
  }
  .chip:hover { color: var(--color-fg); }
  .chip.is-on {
    color: var(--color-fg);
    border-color: var(--color-border-strong);
    background: var(--color-active);
  }
  .chip.is-pick {
    border-color: rgba(120, 200, 120, 0.35);
    color: rgba(160, 220, 160, 0.95);
  }
  .chip.is-reject {
    border-color: rgba(220, 120, 120, 0.35);
    color: rgba(230, 150, 150, 0.95);
  }

  .clear {
    appearance: none;
    border: none;
    background: transparent;
    color: var(--color-subtle);
    font-size: 11px;
    padding: 4px 6px;
    cursor: pointer;
  }
  .clear:hover { color: var(--color-fg); }
</style>
