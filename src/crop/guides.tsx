// Standalone composition-guide engine (plan P4). Plugin-style guide
// definitions consumed by the crop tool today and reusable by any future
// geometry tool (perspective, framing, local adjustments — darktable model).
// All rendering is pure SVG in the caller's coordinate space.

export type GuideId =
  | "none"
  | "grid"
  | "thirds"
  | "diagonal"
  | "triangle"
  | "golden"
  | "spiral"
  | "aspects"
  | "center"
  | "epassport";

export interface GuideCtx {
  /** Crop rect in screen px. */
  x: number;
  y: number;
  w: number;
  h: number;
  /** Shift+O orientation index (asymmetric guides). */
  variant: number;
  /** Guide engine settings. */
  gridSize: number;
  aspectRatios: string[]; // "w:h" strings for the aspect-preview guide
}

export interface GuideDef {
  id: GuideId;
  label: string;
  /** Responds to Shift+O (flip/rotate). */
  asymmetric?: boolean;
  render: (ctx: GuideCtx) => JSX.Element | null;
}

const PHI = (1 + Math.sqrt(5)) / 2;

function line(x1: number, y1: number, x2: number, y2: number, key: string) {
  return <line key={key} x1={x1} y1={y1} x2={x2} y2={y2} />;
}

/** Foot of the perpendicular from point p to the line a–b. */
function perpFoot(
  ax: number,
  ay: number,
  bx: number,
  by: number,
  px: number,
  py: number,
): [number, number] {
  const dx = bx - ax;
  const dy = by - ay;
  const t = ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy);
  return [ax + t * dx, ay + t * dy];
}

const grid: GuideDef = {
  id: "grid",
  label: "Grid",
  render: ({ x, y, w, h, gridSize }) => {
    const n = Math.max(2, gridSize);
    const out: JSX.Element[] = [];
    for (let i = 1; i < n; i++) {
      const t = i / n;
      out.push(line(x + w * t, y, x + w * t, y + h, `v${i}`));
      out.push(line(x, y + h * t, x + w, y + h * t, `h${i}`));
    }
    return <g>{out}</g>;
  },
};

const thirds: GuideDef = {
  id: "thirds",
  label: "Rule of Thirds",
  render: ({ x, y, w, h }) => (
    <g>
      {[1, 2].map((i) => line(x + (w * i) / 3, y, x + (w * i) / 3, y + h, `v${i}`))}
      {[1, 2].map((i) => line(x, y + (h * i) / 3, x + w, y + (h * i) / 3, `h${i}`))}
    </g>
  ),
};

const diagonal: GuideDef = {
  id: "diagonal",
  label: "Diagonals",
  render: ({ x, y, w, h }) => {
    // 45° lines from each corner (classic diagonal method)
    const d = Math.min(w, h);
    return (
      <g>
        {line(x, y, x + d, y + d, "tl")}
        {line(x + w, y, x + w - d, y + d, "tr")}
        {line(x, y + h, x + d, y + h - d, "bl")}
        {line(x + w, y + h, x + w - d, y + h - d, "br")}
      </g>
    );
  },
};

const triangle: GuideDef = {
  id: "triangle",
  label: "Golden Triangle",
  asymmetric: true,
  render: ({ x, y, w, h, variant }) => {
    const flip = variant % 2 === 1;
    // main diagonal + true perpendiculars from the two remaining corners
    const [ax, ay, bx, by] = flip
      ? [x + w, y, x, y + h]
      : [x, y, x + w, y + h];
    const c1: [number, number] = flip ? [x, y] : [x + w, y];
    const c2: [number, number] = flip ? [x + w, y + h] : [x, y + h];
    const f1 = perpFoot(ax, ay, bx, by, c1[0], c1[1]);
    const f2 = perpFoot(ax, ay, bx, by, c2[0], c2[1]);
    return (
      <g>
        {line(ax, ay, bx, by, "diag")}
        {line(c1[0], c1[1], f1[0], f1[1], "p1")}
        {line(c2[0], c2[1], f2[0], f2[1], "p2")}
      </g>
    );
  },
};

const golden: GuideDef = {
  id: "golden",
  label: "Golden Ratio",
  render: ({ x, y, w, h }) => {
    // phi grid: three bands in ratio 1 : 1/φ : 1
    const lo = 1 / (2 + 1 / PHI);
    const hi = 1 - lo;
    return (
      <g>
        {line(x + w * lo, y, x + w * lo, y + h, "v1")}
        {line(x + w * hi, y, x + w * hi, y + h, "v2")}
        {line(x, y + h * lo, x + w, y + h * lo, "h1")}
        {line(x, y + h * hi, x + w, y + h * hi, "h2")}
      </g>
    );
  },
};

/**
 * True golden spiral: quarter-circle arcs through the golden-section
 * subdivision of the rect. 8 variants (4 corners × 2 winding directions).
 */
const spiral: GuideDef = {
  id: "spiral",
  label: "Golden Spiral",
  asymmetric: true,
  render: ({ x, y, w, h, variant }) => {
    const mirrored = variant >= 4;
    const rot = variant % 4;
    // Build the spiral in a unit landscape golden rect, then map into the
    // crop rect (stretch — matches Lightroom's behavior on non-phi crops).
    type Pt = [number, number];
    const pts: Pt[] = [];
    // squares spiral inward; track the current sub-rect in unit coords
    let rx = 0;
    let ry = 0;
    let rw = PHI; // unit golden rect: phi × 1
    let rh = 1;
    let dir = 0; // 0=left square, 1=top, 2=right, 3=bottom
    const segs: { cx: number; cy: number; r: number; a0: number; a1: number }[] = [];
    for (let i = 0; i < 9; i++) {
      const s = Math.min(rw, rh); // side of the split-off square
      if (dir === 0) {
        segs.push({ cx: rx + s, cy: ry + s, r: s, a0: Math.PI, a1: 1.5 * Math.PI });
        rx += s;
        rw -= s;
      } else if (dir === 1) {
        segs.push({ cx: rx + rw - s, cy: ry + s, r: s, a0: 1.5 * Math.PI, a1: 2 * Math.PI });
        ry += s;
        rh -= s;
      } else if (dir === 2) {
        segs.push({ cx: rx + rw - s, cy: ry + rh - s, r: s, a0: 0, a1: 0.5 * Math.PI });
        rw -= s;
      } else {
        segs.push({ cx: rx + s, cy: ry + rh - s, r: s, a0: 0.5 * Math.PI, a1: Math.PI });
        rh -= s;
      }
      dir = (dir + 1) % 4;
    }
    // sample arcs into a polyline (simple + robust vs. SVG arc flags)
    for (const sg of segs) {
      const steps = 12;
      for (let i = 0; i <= steps; i++) {
        const a = sg.a0 + ((sg.a1 - sg.a0) * i) / steps;
        pts.push([sg.cx + sg.r * Math.cos(a), sg.cy + sg.r * Math.sin(a)]);
      }
    }
    const mapped = pts.map(([ux, uy]): Pt => {
      let u = ux / PHI; // 0..1
      let v = uy; // 0..1
      if (mirrored) u = 1 - u;
      for (let r = 0; r < rot; r++) [u, v] = [1 - v, u];
      return [x + u * w, y + v * h];
    });
    const d = mapped
      .map(([px, py], i) => `${i === 0 ? "M" : "L"} ${px.toFixed(1)} ${py.toFixed(1)}`)
      .join(" ");
    return <path d={d} fill="none" />;
  },
};

const aspects: GuideDef = {
  id: "aspects",
  label: "Aspect Ratios",
  render: ({ x, y, w, h, aspectRatios }) => {
    const out: JSX.Element[] = [];
    aspectRatios.forEach((spec, i) => {
      const m = spec.split(":");
      const rw = parseFloat(m[0] ?? "");
      const rh = parseFloat(m[1] ?? "");
      if (!(rw > 0) || !(rh > 0)) return;
      const r = rw / rh;
      let bw = w;
      let bh = bw / r;
      if (bh > h) {
        bh = h;
        bw = bh * r;
      }
      out.push(
        <rect
          key={`ar${i}`}
          x={x + (w - bw) / 2}
          y={y + (h - bh) / 2}
          width={bw}
          height={bh}
          fill="none"
          strokeDasharray="4 3"
        />,
      );
    });
    return <g>{out}</g>;
  },
};

const center: GuideDef = {
  id: "center",
  label: "Center Cross",
  render: ({ x, y, w, h }) => (
    <g>
      {line(x + w / 2, y, x + w / 2, y + h, "v")}
      {line(x, y + h / 2, x + w, y + h / 2, "h")}
      {line(x, y, x + w, y + h, "d1")}
      {line(x + w, y, x, y + h, "d2")}
    </g>
  ),
};

/**
 * ePassport / biometric guides (RawTherapee): crown, eye and chin lines for
 * the 45:35 ratio — ICAO head placement (head 70–80% of frame height).
 */
const epassport: GuideDef = {
  id: "epassport",
  label: "ePassport",
  render: ({ x, y, w, h }) => (
    <g>
      {line(x, y + h * 0.1, x + w, y + h * 0.1, "crown")}
      {line(x, y + h * 0.45, x + w, y + h * 0.45, "eyes")}
      {line(x, y + h * 0.6, x + w, y + h * 0.6, "nostrils")}
      {line(x, y + h * 0.85, x + w, y + h * 0.85, "chin")}
      {line(x + w * 0.5, y, x + w * 0.5, y + h, "axis")}
    </g>
  ),
};

export const GUIDES: GuideDef[] = [
  grid,
  thirds,
  diagonal,
  triangle,
  golden,
  spiral,
  aspects,
  center,
  epassport,
];

export function guideById(id: string): GuideDef | undefined {
  return GUIDES.find((g) => g.id === id);
}

/** Render a guide into the given rect; null for "none"/unknown ids. */
export function renderGuide(id: string, ctx: GuideCtx): JSX.Element | null {
  const def = guideById(id);
  return def ? def.render(ctx) : null;
}
