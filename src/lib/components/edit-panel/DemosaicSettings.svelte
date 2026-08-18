<script lang="ts">
  import { setDemosaic } from "../../../ipc/commands";
  import { imageMeta, statusMessage } from "../../../stores/app";

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

  const options = $derived.by(() => {
    const avail = $imageMeta?.availableDemosaic;
    if (avail && avail.length > 0) {
      const set = new Set(avail);
      return algorithms.filter((a) => set.has(a.value));
    }
    return algorithms.filter((a) => IN_PROCESS.has(a.value));
  });

  const methodHint =
    "Merawler demosaic runs in-process on every platform. RawTherapee / LibRaw sidecar options appear when their CLI workers are installed next to the app or on PATH.";

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
  <div class="control-row">
    <span title={methodHint}>Method</span>
  </div>
  <select value={current} onchange={onChange} class="rail-select" title={methodHint}>
    {#each options as alg}
      <option value={alg.value}>{alg.label}</option>
    {/each}
  </select>
{/if}
