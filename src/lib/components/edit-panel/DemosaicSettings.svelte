<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import Slider from "./Slider.svelte";
  import { setDemosaic } from "../../../ipc/commands";
  import { imageMeta, statusMessage } from "../../../stores/app";

  let falseColorSuppression = $state(0);

  /** In-process engines (merawler + rawler) — always usable on every OS. */
  const IN_PROCESS = new Set([
    "rawler",
    "bilinear",
    "malvar",
    "rcd",
    "lmmse",
    "amaze",
    "igv",
    "ddfapd",
  ]);

  const algorithms = [
    { value: "rawler", label: "Rawler (built-in)" },
    { value: "bilinear", label: "Bilinear" },
    { value: "malvar", label: "Malvar" },
    { value: "rcd", label: "RCD — merawler default" },
    { value: "lmmse", label: "LMMSE — best for noise" },
    { value: "amaze", label: "AMaZE — max detail" },
    { value: "igv", label: "IGV — anti-aliasing" },
    { value: "ddfapd", label: "DDFAPD (Menon)" },
    { value: "rt-rcd", label: "RCD — sidecar · RawTherapee" },
    { value: "rt-lmmse", label: "LMMSE — sidecar · RawTherapee" },
    { value: "rt-amaze", label: "AMaZE — sidecar · RawTherapee" },
    { value: "dht", label: "DHT — sidecar · LibRaw" },
  ];

  const current = $derived($imageMeta?.demosaic || "rcd");
  const isRaw = $derived($imageMeta?.kind === "raw");

  /** Prefer engine-reported availability; fall back to in-process only. */
  const options = $derived.by(() => {
    const avail = $imageMeta?.availableDemosaic;
    if (avail && avail.length > 0) {
      const set = new Set(avail);
      return algorithms.filter((a) => set.has(a.value));
    }
    return algorithms.filter((a) => IN_PROCESS.has(a.value));
  });

  async function onChange(e: Event) {
    const algo = (e.currentTarget as HTMLSelectElement).value;
    try {
      const m = await setDemosaic(algo);
      imageMeta.set(m);
      statusMessage.set(`demosaic → ${m.demosaic}`);
    } catch (err) {
      statusMessage.set(
        err && typeof err === "object" && "message" in err
          ? String((err as { message: string }).message)
          : `demosaic failed: ${String(err)}`,
      );
    }
  }
</script>

{#if isRaw}
  <CollapsibleSection id="demosaic" title="Demosaic Settings">
    <div class="flex flex-col gap-[12px]">
      <div class="grid grid-cols-[64px_1fr] items-center gap-x-[10px]">
        <span class="text-[8px] text-white/80">Method</span>
        <select
          value={current}
          onchange={onChange}
          class="demosaic-select h-[26px] w-full text-[8px] text-white/90 focus:outline-none"
        >
          {#each options as alg}
            <option value={alg.value}>{alg.label}</option>
          {/each}
        </select>
      </div>
      {#if options.length <= IN_PROCESS.size}
        <p class="text-[7px] leading-snug text-white/35">
          Merawler demosaic runs in-process on every platform. RawTherapee / LibRaw
          sidecar options appear when their CLI workers are installed next to the app
          or on PATH.
        </p>
      {/if}

      <div class="grid h-[13px] grid-cols-[64px_1fr] items-center gap-x-[10px]">
        <span class="text-[8px] text-white/80">False Color</span>
        <Slider
          label="False Color Suppression"
          min={0}
          max={100}
          value={falseColorSuppression}
          onchange={(v) => (falseColorSuppression = v)}
        />
      </div>
    </div>
  </CollapsibleSection>
{/if}

<style>
  .demosaic-select {
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(33, 33, 35, 0.6)
      url("data:image/svg+xml,%3Csvg width='8' height='5' viewBox='0 0 8 5' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1L4 4L7 1' stroke='rgba%28255,255,255,0.6%29' stroke-width='1.2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")
      no-repeat right 8px center;
    background-size: 8px 5px;
    border-radius: 6px;
    padding: 0 20px 0 8px;
    outline: none;
    cursor: pointer;
  }
  .demosaic-select option {
    background: #222224;
    color: #fff;
    font-size: 8px;
  }
</style>
