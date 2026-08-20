<script lang="ts">
  import ParamRow from "./ParamRow.svelte";
  import { listLooks, pickLut, setLut, type LookInfo } from "../../../ipc/commands";
  import { setParam } from "../../../ipc/commands";
  import { deleteUserLook, revealInFinder, userLooksDir } from "../../../ipc/commands";
  import { doc, reconcile } from "../../../stores/doc";
  import { statusMessage } from "../../../stores/app";

  const lutFile = $derived(
    ($doc?.meta as { lut_file?: string; look_id?: string } | undefined)?.lut_file ?? null,
  );
  const lookId = $derived(
    ($doc?.meta as { look_id?: string } | undefined)?.look_id ??
      (lutFile?.startsWith("bundled:")
        ? lutFile.slice("bundled:".length)
        : lutFile?.startsWith("user:")
          ? lutFile
          : null),
  );
  const hasLut = $derived(!!lutFile);
  const lutName = $derived(
    lookId ?? (lutFile ? lutFile.split("/").pop() : "No Look"),
  );

  let looks = $state<LookInfo[]>([]);
  let lookError = $state<string | null>(null);
  let comparing = $state(false);
  let enabledBeforeCompare = $state(1);

  $effect(() => {
    void reloadLooks();
  });

  const enabled = $derived(
    Number(($doc?.modules?.lut as { enabled?: number } | undefined)?.enabled ?? 1) >= 0.5,
  );

  async function applyLook(id: string) {
    lookError = null;
    try {
      const path = id.startsWith("user:") ? id : `bundled:${id}`;
      await setLut(path);
      statusMessage.set(`look → ${id}`);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      lookError = msg;
      statusMessage.set(`look failed: ${msg}`);
    }
  }

  async function reloadLooks() {
    try {
      looks = await listLooks();
      lookError = null;
    } catch (e) {
      lookError = e instanceof Error ? e.message : String(e);
    }
  }

  async function importLut() {
    lookError = null;
    const path = await pickLut();
    if (!path) return;
    try {
      await setLut(path);
      statusMessage.set(`LUT loaded · Rec.709 display (override below)`);
      await reloadLooks();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      lookError = msg;
      statusMessage.set(`LUT failed: ${msg}`);
    }
  }

  async function clearLut() {
    lookError = null;
    try {
      await setLut(null);
      statusMessage.set("look cleared");
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      lookError = msg;
      statusMessage.set(`clear look failed: ${msg}`);
    }
  }

  async function openLooksFolder() {
    try {
      const dir = await userLooksDir();
      await revealInFinder(dir);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      lookError = msg;
      statusMessage.set(`looks folder: ${msg}`);
    }
  }

  async function removeUserLook(id: string) {
    lookError = null;
    try {
      await deleteUserLook(id);
      if (lookId === id) await setLut(null);
      statusMessage.set("user look deleted");
      await reloadLooks();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      lookError = msg;
      statusMessage.set(`delete look failed: ${msg}`);
    }
  }

  function setEnabled(on: boolean) {
    void setParam("lut.enabled", on ? 1 : 0).then(reconcile);
  }

  function swatchStyle(look: LookInfo): string {
    const p = look.preview;
    if (!p || p.length < 3) return "";
    const css = (rgb: number[]) => `rgb(${rgb[0]}, ${rgb[1]}, ${rgb[2]})`;
    return `linear-gradient(90deg, ${css(p[0])}, ${css(p[1])}, ${css(p[2])})`;
  }

  function cycleLook(dir: number) {
    if (!looks.length) return;
    const idx = looks.findIndex((l) => l.id === lookId);
    const next = looks[(idx < 0 ? 0 : idx + dir + looks.length) % looks.length];
    if (next) void applyLook(next.id);
  }

  $effect(() => {
    const onNext = () => cycleLook(1);
    const onPrev = () => cycleLook(-1);
    const onToggle = () => {
      if (hasLut) setEnabled(!enabled);
    };
    window.addEventListener("meraraw:look-next", onNext);
    window.addEventListener("meraraw:look-prev", onPrev);
    window.addEventListener("meraraw:look-toggle", onToggle);
    return () => {
      window.removeEventListener("meraraw:look-next", onNext);
      window.removeEventListener("meraraw:look-prev", onPrev);
      window.removeEventListener("meraraw:look-toggle", onToggle);
    };
  });

  function startCompare() {
    if (!hasLut) return;
    comparing = true;
    enabledBeforeCompare = enabled ? 1 : 0;
    void setParam("lut.enabled", 0).then(reconcile);
  }

  function endCompare() {
    if (!comparing) return;
    comparing = false;
    void setParam("lut.enabled", enabledBeforeCompare).then(reconcile);
  }

  const kinds = [
    { v: 0, l: "Display" },
    { v: 1, l: "Log" },
    { v: 2, l: "IDT" },
    { v: 3, l: "Show" },
  ];
  const primaries = [
    { v: 0, l: "2020" },
    { v: 1, l: "709" },
    { v: 2, l: "P3" },
  ];
  const shapers = [
    { v: 0, l: "Lin" },
    { v: 1, l: "sRGB" },
    { v: 2, l: "709" },
    { v: 5, l: "LogC3" },
    { v: 7, l: "SLog3" },
  ];

  const kindVal = $derived(Number(($doc?.modules?.lut as { kind?: number } | undefined)?.kind ?? 0));
  const inPri = $derived(
    Number(($doc?.modules?.lut as { input_primaries?: number } | undefined)?.input_primaries ?? 1),
  );
  const outPri = $derived(
    Number(($doc?.modules?.lut as { output_primaries?: number } | undefined)?.output_primaries ?? 1),
  );
  const shaperVal = $derived(
    Number(($doc?.modules?.lut as { shaper?: number } | undefined)?.shaper ?? 1),
  );
  const interpVal = $derived(
    Number(($doc?.modules?.lut as { interpolation?: number } | undefined)?.interpolation ?? 0),
  );
  const grouped = $derived.by(() => {
    const map = new Map<string, LookInfo[]>();
    for (const look of looks) {
      const cat = look.category || "Looks";
      const rows = map.get(cat) ?? [];
      rows.push(look);
      map.set(cat, rows);
    }
    return [...map.entries()];
  });
</script>

<div class="look-head">
  <p class="rail-empty" title={lutFile ?? undefined}>{comparing ? "Look off (hold)" : lutName}</p>
  <button
    type="button"
    class="rail-chip"
    class:is-active={enabled && hasLut}
    disabled={!hasLut}
    onclick={() => setEnabled(!enabled)}
  >
    {enabled ? "On" : "Off"}
  </button>
</div>

{#if lookError}
  <p class="err">{lookError}</p>
{/if}

<div class="look-list">
  {#each grouped as [cat, rows] (cat)}
    <p class="mini">{cat}</p>
    <div class="look-grid">
      {#each rows as look (look.id)}
        <button
          type="button"
          class="look-card"
          class:is-active={lookId === look.id}
          title={look.description}
          onclick={() => void applyLook(look.id)}
        >
          <span class="look-swatch" style={swatchStyle(look)}></span>
          <span class="look-name">{look.name}</span>
        </button>
      {/each}
    </div>
  {/each}
</div>

{#if hasLut}
  <ParamRow path="lut.opacity" label="Intensity" />
  <ParamRow path="lut.enabled" label="Enabled" />
{/if}

<div class="row">
  <button type="button" class="rail-btn" onclick={() => void importLut()}>Import .cube</button>
  <button type="button" class="rail-btn" onclick={() => void openLooksFolder()}>Looks folder</button>
  {#if hasLut}
    <button type="button" class="rail-btn" onclick={() => void clearLut()}>Reset</button>
    <button
      type="button"
      class="rail-btn"
      onpointerdown={startCompare}
      onpointerup={endCompare}
      onpointerleave={endCompare}
    >
      A/B
    </button>
  {/if}
  {#if lookId?.startsWith("user:")}
    <button type="button" class="rail-btn" onclick={() => void removeUserLook(lookId ?? "")}>Delete</button>
  {/if}
</div>

{#if hasLut}
  <p class="group-label">Interpretation</p>
  <p class="hint">Unknown cubes default to Rec.709 display + sRGB shaper.</p>
  <div class="pill">
    {#each kinds as k (k.v)}
      <button type="button" class="rail-chip" class:is-active={kindVal === k.v} onclick={() => void setParam("lut.kind", k.v).then(reconcile)}>
        {k.l}
      </button>
    {/each}
  </div>
  <p class="mini">Input primaries</p>
  <div class="pill">
    {#each primaries as k (k.v)}
      <button
        type="button"
        class="rail-chip"
        class:is-active={inPri === k.v}
        onclick={() => void setParam("lut.input_primaries", k.v).then(reconcile)}>{k.l}</button
      >
    {/each}
  </div>
  <p class="mini">Output primaries</p>
  <div class="pill">
    {#each primaries as k (k.v)}
      <button
        type="button"
        class="rail-chip"
        class:is-active={outPri === k.v}
        onclick={() => void setParam("lut.output_primaries", k.v).then(reconcile)}>{k.l}</button
      >
    {/each}
  </div>
  <p class="mini">Shaper</p>
  <div class="pill">
    {#each shapers as k (k.v)}
      <button type="button" class="rail-chip" class:is-active={shaperVal === k.v} onclick={() => void setParam("lut.shaper", k.v).then(reconcile)}>
        {k.l}
      </button>
    {/each}
  </div>
  <div class="pill" style="margin-top: 6px">
    <button type="button" class="rail-chip" class:is-active={interpVal === 0} onclick={() => void setParam("lut.interpolation", 0).then(reconcile)}>
      Tetrahedral
    </button>
    <button type="button" class="rail-chip" class:is-active={interpVal === 1} onclick={() => void setParam("lut.interpolation", 1).then(reconcile)}>Trilinear</button>
  </div>
  <ParamRow path="effects.grain_amount" label="Grain" />
  <ParamRow path="effects.grain_size" label="Grain size" />
{/if}

<style>
  .look-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 6px;
  }
  .look-list {
    max-height: 220px;
    overflow: auto;
    margin: 6px 0 8px;
  }
  .look-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
    margin: 0 0 8px;
  }
  .look-card {
    display: flex;
    align-items: center;
    gap: 6px;
    text-align: left;
    padding: 4px 6px;
    border-radius: 6px;
    border: 1px solid var(--color-border-strong);
    background: var(--color-sunken);
    color: var(--color-fg);
    font-size: 10px;
    line-height: 1.2;
    cursor: pointer;
  }
  .look-card.is-active {
    border-color: var(--color-fg);
    background: var(--color-active);
  }
  .look-swatch {
    width: 22px;
    height: 14px;
    border-radius: 3px;
    flex: none;
    background: var(--color-sunken);
  }
  .look-name {
    min-width: 0;
  }
  .row {
    display: flex;
    gap: 4px;
    margin: 6px 0;
  }
  .row :global(.rail-btn) {
    flex: 1;
  }
  .err {
    color: #f07178;
    font-size: 10px;
    margin: 0 0 6px;
  }
  .hint,
  .mini {
    font-size: 10px;
    color: var(--color-muted, #888);
    margin: 4px 0 2px;
  }
  .pill {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
    margin-bottom: 4px;
  }
</style>
