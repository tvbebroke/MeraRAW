// Education tab — short, accurate guides to the science of color, tied back
// to how MeraRAW actually processes your images. Pure content + SVG diagrams;
// no engine coupling. Meant to make the editor teach as you use it.
import { useState } from "react";

// ---------- small presentational helpers ----------

function Tie({ children }: { children: React.ReactNode }) {
  return (
    <div className="edu-tie">
      <span className="edu-tie-badge">In MeraRAW</span>
      <div>{children}</div>
    </div>
  );
}

function Fig({
  caption,
  children,
}: {
  caption: string;
  children: React.ReactNode;
}) {
  return (
    <figure className="edu-fig">
      {children}
      <figcaption>{caption}</figcaption>
    </figure>
  );
}

// ---------- diagrams ----------

function gaussianPath(peak: number, sigma: number, amp: number): string {
  const pts: string[] = [];
  for (let nm = 400; nm <= 700; nm += 5) {
    const x = 30 + ((nm - 400) / 300) * 290;
    const g = amp * Math.exp(-0.5 * ((nm - peak) / sigma) ** 2);
    const y = 150 - g * 120;
    pts.push(`${x.toFixed(1)},${y.toFixed(1)}`);
  }
  return "M" + pts.join(" L");
}

function ConeCurves() {
  const ticks = [400, 500, 600, 700];
  return (
    <svg viewBox="0 0 340 175" className="edu-svg" role="img" aria-label="Cone sensitivity curves">
      <line x1="30" y1="150" x2="320" y2="150" stroke="var(--border)" />
      {ticks.map((nm) => {
        const x = 30 + ((nm - 400) / 300) * 290;
        return (
          <g key={nm}>
            <line x1={x} y1="150" x2={x} y2="154" stroke="var(--text-faint)" />
            <text x={x} y="166" className="edu-svg-tick">{nm}</text>
          </g>
        );
      })}
      <text x="175" y="174" className="edu-svg-axis">wavelength (nm)</text>
      <path d={gaussianPath(440, 30, 0.9)} fill="none" stroke="#5a9bff" strokeWidth="2.5" />
      <path d={gaussianPath(540, 45, 1.0)} fill="none" stroke="#6ad07a" strokeWidth="2.5" />
      <path d={gaussianPath(570, 55, 1.0)} fill="none" stroke="#ff6b6b" strokeWidth="2.5" />
      <text x="70" y="40" className="edu-svg-lab" fill="#5a9bff">S · 440nm</text>
      <text x="150" y="24" className="edu-svg-lab" fill="#6ad07a">M · 540nm</text>
      <text x="238" y="34" className="edu-svg-lab" fill="#ff6b6b">L · 570nm</text>
    </svg>
  );
}

// approximate CIE 1931 spectral locus (illustrative, not calibrated)
const LOCUS: Array<[number, number]> = [
  [0.175, 0.005], [0.144, 0.03], [0.091, 0.113], [0.045, 0.295],
  [0.02, 0.5], [0.008, 0.65], [0.02, 0.75], [0.075, 0.833],
  [0.23, 0.754], [0.373, 0.624], [0.513, 0.487], [0.627, 0.373],
  [0.692, 0.308], [0.719, 0.281], [0.735, 0.265],
];
const CX = (x: number) => 20 + (x / 0.75) * 300;
const CY = (y: number) => 285 - (y / 0.85) * 270;

function Chromaticity() {
  const locusPts = LOCUS.map(([x, y]) => `${CX(x).toFixed(1)},${CY(y).toFixed(1)}`).join(" ");
  const R: [number, number] = [0.64, 0.33];
  const G: [number, number] = [0.3, 0.6];
  const B: [number, number] = [0.15, 0.06];
  const W: [number, number] = [0.3127, 0.329];
  const tri = [R, G, B].map(([x, y]) => `${CX(x).toFixed(1)},${CY(y).toFixed(1)}`).join(" ");
  return (
    <svg viewBox="0 0 340 300" className="edu-svg" role="img" aria-label="CIE chromaticity diagram">
      <defs>
        <linearGradient id="spectral" x1="0" y1="1" x2="1" y2="0">
          <stop offset="0%" stopColor="#2b3bd0" />
          <stop offset="22%" stopColor="#22b7c8" />
          <stop offset="42%" stopColor="#43c94a" />
          <stop offset="62%" stopColor="#e6d43a" />
          <stop offset="82%" stopColor="#ff7b3a" />
          <stop offset="100%" stopColor="#ff4b4b" />
        </linearGradient>
      </defs>
      <polygon points={locusPts} fill="url(#spectral)" opacity="0.5"
        stroke="var(--text-dim)" strokeWidth="1" />
      <polygon points={tri} fill="none" stroke="#fff" strokeWidth="1.5" strokeDasharray="4 3" />
      {[["R", R], ["G", G], ["B", B]].map(([lab, p]) => {
        const [x, y] = p as [number, number];
        return <circle key={lab as string} cx={CX(x)} cy={CY(y)} r="3.5" fill="#fff" />;
      })}
      <circle cx={CX(W[0])} cy={CY(W[1])} r="3" fill="#111" stroke="#fff" strokeWidth="1.5" />
      <text x={CX(W[0]) + 6} y={CY(W[1]) + 3} className="edu-svg-lab" fill="#fff">D65 white</text>
      <text x={CX(R[0]) + 6} y={CY(R[1]) + 4} className="edu-svg-lab" fill="#fff">red</text>
      <text x={CX(G[0]) - 4} y={CY(G[1]) - 8} className="edu-svg-lab" fill="#fff">green</text>
      <text x={CX(B[0]) - 6} y={CY(B[1]) + 14} className="edu-svg-lab" fill="#fff">blue</text>
    </svg>
  );
}

function Pipeline() {
  const stages = [
    ["RAW / file", "sensor or image"],
    ["Linear Rec.2020", "scene-referred, 32-bit"],
    ["Your edits", "exposure → grade → curve"],
    ["View transform", "the “look”"],
    ["Display", "sRGB / HDR"],
  ];
  return (
    <svg viewBox="0 0 700 96" className="edu-svg" role="img" aria-label="MeraRAW pipeline">
      {stages.map(([t, s], i) => {
        const x = 8 + i * 138;
        return (
          <g key={t}>
            <rect x={x} y="24" width="120" height="48" rx="6"
              fill="var(--bg-elevated)" stroke="var(--border)" />
            <text x={x + 60} y="45" className="edu-svg-box">{t}</text>
            <text x={x + 60} y="60" className="edu-svg-boxsub">{s}</text>
            {i < stages.length - 1 && (
              <path d={`M${x + 120} 48 L${x + 138} 48`} stroke="var(--accent)"
                strokeWidth="2" markerEnd="url(#arrow)" />
            )}
          </g>
        );
      })}
      <defs>
        <marker id="arrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6"
          markerHeight="6" orient="auto-start-reverse">
          <path d="M0 0 L10 5 L0 10 z" fill="var(--accent)" />
        </marker>
      </defs>
    </svg>
  );
}

function MixCircles({ mode }: { mode: "add" | "sub" }) {
  const add = mode === "add";
  const cols = add ? ["#ff3b3b", "#3bff3b", "#3b6bff"] : ["#33cccc", "#cc33cc", "#cccc33"];
  const centers: Array<[number, number]> = [[85, 62], [125, 62], [105, 96]];
  return (
    <svg viewBox="0 0 210 170" className="edu-svg" role="img" aria-label={add ? "additive mixing" : "subtractive mixing"}>
      <rect x="0" y="0" width="210" height="170" rx="8" fill={add ? "#000" : "#fff"} />
      {centers.map((c, i) => (
        <circle key={i} cx={c[0]} cy={c[1]} r="42" fill={cols[i]}
          style={{ mixBlendMode: add ? "screen" : "multiply" }} />
      ))}
      <text x="105" y="150" className="edu-svg-axis" fill={add ? "#fff" : "#333"}>
        {add ? "light adds → white (RGB screens)" : "ink subtracts → black (CMY print)"}
      </text>
    </svg>
  );
}

// ---------- guide content ----------

type Guide = {
  id: string;
  group: string;
  title: string;
  tagline: string;
  min: number;
  body: React.ReactNode;
};

const GUIDES: Guide[] = [
  {
    id: "what-is-color",
    group: "Foundations",
    title: "What Is Color, Really?",
    tagline: "Color lives in your brain, not in the light.",
    min: 2,
    body: (
      <>
        <p>
          It feels obvious that a tomato <em>is</em> red. But red isn't a property
          of the tomato or even of the light bouncing off it. The only physical
          thing there is <b>wavelength</b> — light in the ~380–780&nbsp;nm range.
          "Color" is the interpretation your brain builds from that signal.
        </p>
        <p>
          This matters for grading: two completely different mixtures of
          wavelengths can look identical to you (they're called <i>metamers</i>),
          and the same physical light can look like a different color depending on
          what surrounds it. We aren't editing wavelengths — we're editing a
          <b> perception</b>.
        </p>
        <p className="edu-key">
          Wavelength is physical. Color is psychological. Every tool in this app
          is really a tool for steering perception.
        </p>
      </>
    ),
  },
  {
    id: "cones",
    group: "Foundations",
    title: "Your Eyes Have Three Sensors",
    tagline: "Trichromacy — why color is three-dimensional.",
    min: 3,
    body: (
      <>
        <p>
          Your retina has ~6&nbsp;million cone cells in three types, each sensitive
          to a broad, overlapping band of wavelengths:
        </p>
        <ul className="edu-list">
          <li><b>S</b> ("blue") — peaks near <b>440&nbsp;nm</b>, only ~5–10% of cones</li>
          <li><b>M</b> ("green") — peaks near <b>540&nbsp;nm</b></li>
          <li><b>L</b> ("red") — peaks near <b>570&nbsp;nm</b> (actually yellow-green!)</li>
        </ul>
        <Fig caption="The three cone responses overlap heavily — 'red', 'green', 'blue' are loose nicknames.">
          <ConeCurves />
        </Fig>
        <p>
          Because there are exactly three sensors, any color you can see collapses
          to <b>three numbers</b> — the relative L, M, S responses. That's why every
          color space (RGB, XYZ, HSV) is three-dimensional, and why displays need a
          minimum of three primaries.
        </p>
        <Tie>
          MeraRAW works internally in a three-channel linear space. The whole
          engine is built on this fact: color = 3 numbers per pixel.
        </Tie>
      </>
    ),
  },
  {
    id: "luminance",
    group: "Foundations",
    title: "Why Green Looks Brightest",
    tagline: "Luminance is a weighted sum of cone responses.",
    min: 2,
    body: (
      <>
        <p>
          Add the three cone responses together and you get a curve that peaks in
          the green-yellow region (~540–570&nbsp;nm). That's why a green and a blue
          of the <i>same physical energy</i> don't look equally bright — green
          stimulates far more of your cones.
        </p>
        <p>
          Perceived brightness — <b>luminance</b> — is therefore a weighted mix,
          heavy on green. In Rec.2020 the weights are roughly{" "}
          <code>0.26·R + 0.68·G + 0.06·B</code>.
        </p>
        <Tie>
          Those exact weights (<code>0.2627, 0.6780, 0.0593</code>) live in the
          grade shader. When a saturation move needs to preserve brightness, this
          is the luminance it protects.
        </Tie>
      </>
    ),
  },
  {
    id: "xyz",
    group: "The Color Map",
    title: "CIE XYZ & the Chromaticity Diagram",
    tagline: "A universal map of every visible color.",
    min: 3,
    body: (
      <>
        <p>
          In 1931 the CIE built a standard color space, <b>XYZ</b>, as a linear
          transform of the cone responses. Its <b>Y</b> axis is luminance; X and Z
          carry the rest. Divide out brightness and you get 2D <b>chromaticity</b>:
        </p>
        <p className="edu-eq">x = X / (X+Y+Z)&nbsp;&nbsp;&nbsp;y = Y / (X+Y+Z)</p>
        <Fig caption="The horseshoe = every visible color. The dashed triangle = what an sRGB screen can show.">
          <Chromaticity />
        </Fig>
        <p>
          The curved edge is pure spectral (single-wavelength) light; the straight
          bottom is the "line of purples," which no single wavelength can make.
          Any real color is a point inside the horseshoe.
        </p>
      </>
    ),
  },
  {
    id: "gamut",
    group: "The Color Map",
    title: "Gamut: No Screen Shows Every Color",
    tagline: "Visible ⊃ your monitor ⊃ print.",
    min: 3,
    body: (
      <>
        <p>
          Pick three real primaries and the colors you can mix from them form a
          <b> triangle</b> inside the horseshoe — your <b>gamut</b>. Everything
          outside it is visible but unreproducible on that device. sRGB is a small
          triangle; Display-P3 and Rec.2020 are larger; CMYK print is smaller still.
        </p>
        <Fig caption="Anything between the dashed sRGB triangle and the horseshoe edge is a color your screen simply cannot display.">
          <Chromaticity />
        </Fig>
        <p>
          There are even colors that are mathematically valid in cone space but{" "}
          <b>physically impossible</b> — like a "hyper-green" that stimulates only M
          cones. No wavelength can do that, because anything hitting M also hits L or S.
        </p>
        <Tie>
          When a grade pushes a color past what the output can show, MeraRAW's
          grade shader runs a <b>constant-hue gamut compression</b> — it eases the
          color back onto the boundary along its hue instead of hard-clipping, so
          you don't get flat, posterized patches.
        </Tie>
      </>
    ),
  },
  {
    id: "additive-subtractive",
    group: "The Color Map",
    title: "Adding Light vs. Subtracting Ink",
    tagline: "Screens and printers are opposites.",
    min: 2,
    body: (
      <>
        <p>
          A screen starts black and <b>adds</b> red, green and blue light — all
          three at full gives white. Print starts white and <b>subtracts</b> with
          cyan, magenta, yellow ink — all three gives (muddy) black, which is why
          print adds a real black, <b>K</b>, for CMYK.
        </p>
        <div className="edu-fig-row">
          <MixCircles mode="add" />
          <MixCircles mode="sub" />
        </div>
        <p className="edu-key">
          C = 1−R, M = 1−G, Y = 1−B. Subtractive is just additive, inverted.
        </p>
      </>
    ),
  },
  {
    id: "rgb-bitdepth",
    group: "Working With Color",
    title: "RGB, Hex & Bit Depth",
    tagline: "How three numbers become 16 million colors.",
    min: 3,
    body: (
      <>
        <p>
          A pixel is three values — R, G, B. At <b>8-bit</b>, each is 0–255, giving
          256³ ≈ <b>16.7&nbsp;million</b> colors, written in hex as{" "}
          <code>#RRGGBB</code>. That's plenty for a finished image but{" "}
          <i>not</i> for editing.
        </p>
        <ul className="edu-list">
          <li><b>8-bit</b> — 256 steps/channel · standard delivery (JPEG)</li>
          <li><b>10-bit</b> — 1024 steps · 1.07&nbsp;billion colors · HDR</li>
          <li><b>12/16-bit</b> — 4096+ steps · heavy grading headroom</li>
        </ul>
        <p>
          Push contrast or exposure on an 8-bit file and the gaps between steps show
          up as <b>banding</b> in skies and gradients. More bits = smoother edits.
        </p>
        <Tie>
          MeraRAW decodes everything to <b>32-bit float</b> and does every edit at
          that precision, only quantizing to 8/16-bit at export. RAW files start
          with 12–16 real bits per channel — the badge in the status bar shows the
          true bit depth of whatever you opened.
        </Tie>
      </>
    ),
  },
  {
    id: "hsv",
    group: "Working With Color",
    title: "HSV: The Artist's Color Space",
    tagline: "Hue, Saturation, Value — how humans talk color.",
    min: 2,
    body: (
      <>
        <p>
          RGB is how machines store color; <b>HSV</b> is how people think about it:
        </p>
        <ul className="edu-list">
          <li><b>Hue</b> — the angle round the color wheel, 0–360°</li>
          <li><b>Saturation</b> — how pure vs. gray, 0–100%</li>
          <li><b>Value</b> — how bright, 0–100%</li>
        </ul>
        <div className="edu-hsv">
          <div className="edu-hsv-wheel" />
          <div className="edu-hsv-val" />
        </div>
        <p>
          It's just a cylindrical re-mapping of the RGB cube — but separating hue
          from saturation from brightness is exactly what you want when grading.
        </p>
        <Tie>
          The three <b>color wheels</b> in Develop are HSV in spirit: wheel angle
          picks a hue, distance from center sets strength, and the slider under each
          controls luminance — applied to shadows, midtones and highlights separately.
        </Tie>
      </>
    ),
  },
  {
    id: "white-balance",
    group: "Working With Color",
    title: "White Balance & Color Temperature",
    tagline: "Making white look white under any light.",
    min: 3,
    body: (
      <>
        <p>
          Hot things glow in a predictable sequence — the <b>Planckian locus</b>.
          A 3000&nbsp;K bulb is orange, noon sun ~5500–6500&nbsp;K is neutral, deep
          shade climbs past 10000&nbsp;K into blue. Cameras capture that cast; white
          balance removes it.
        </p>
        <div className="edu-temp">
          <span>3000K</span>
          <div className="edu-temp-bar" />
          <span>10000K</span>
        </div>
        <p>
          The physically correct way to re-balance is the <b>von Kries transform</b>:
          convert to cone (LMS) space and <b>multiply</b> each channel by a gain —
          exactly modeling how your eye adapts when you walk from daylight into a
          tungsten-lit room.
        </p>
        <Tie>
          The Temp and Tint sliders do a von Kries-style adaptation. And it inspired
          a whole grading model — see <b>The Three Grading Models</b>, where "Light"
          mode tints your shadows and highlights as if lit by colored lamps.
        </Tie>
      </>
    ),
  },
  {
    id: "linear-pipeline",
    group: "Inside MeraRAW",
    title: "Why We Work in Linear Rec.2020",
    tagline: "Editing light, not pixels.",
    min: 3,
    body: (
      <>
        <p>
          JPEGs store <b>gamma-encoded</b> values tuned for display, not for math.
          Blend or brighten them directly and colors go wrong — highlights get dull,
          blurs turn muddy. So MeraRAW converts everything to a <b>linear,
          scene-referred</b> space (linear Rec.2020) where pixel values are
          proportional to real light.
        </p>
        <Fig caption="Every image lands in the same wide, linear working space before a single edit runs.">
          <Pipeline />
        </Fig>
        <p>
          <b>Linear</b> means doubling a value doubles the light, so exposure and
          blending behave physically. <b>Rec.2020</b> is a very wide gamut, so
          saturated colors survive editing instead of getting clipped early.
          <b> Scene-referred</b> keeps values above 1.0 — real highlight headroom —
          until a final <b>view transform</b> maps the scene down to your display.
        </p>
        <Tie>
          This is why RAW and JPEG of the same shot behave differently here: the RAW
          carries the full linear scene, while a JPEG is already display-baked, so
          MeraRAW skips the view transform on it to avoid double-processing.
        </Tie>
      </>
    ),
  },
  {
    id: "grading-models",
    group: "Inside MeraRAW",
    title: "The Three Grading Models",
    tagline: "Same wheels, three kinds of color science.",
    min: 4,
    body: (
      <>
        <p>
          Most editors give you one color-wheel algorithm. MeraRAW gives you three,
          switchable at the top of the <b>Color Grade</b> panel. The wheels stay the
          same — what changes is the <i>math</i> that injects the color, and each
          has a genuinely different feel.
        </p>
        <div className="edu-models">
          <div className="edu-model">
            <h4>Perceptual</h4>
            <p className="edu-model-sci">Oklab, constant-hue</p>
            <p>
              Moves color in a perceptually-uniform space and holds the hue fixed as
              it saturates. The cleanest, most modern look — no surprise hue shifts.
              Great for skin and precise, controlled grades.
            </p>
          </div>
          <div className="edu-model">
            <h4>Classic</h4>
            <p className="edu-model-sci">RGB offset wheels</p>
            <p>
              Adds a colored offset directly in linear RGB — the way lift/gamma/gain
              wheels have always worked. Channels cross-talk, so hues shift a little
              as you push. Punchy, filmic-vintage, a bit unpredictable in the best way.
            </p>
          </div>
          <div className="edu-model">
            <h4>Light</h4>
            <p className="edu-model-sci">LMS von&nbsp;Kries</p>
            <p>
              Multiplies in cone space, exactly like the white-balance math — so a
              tint behaves like <b>colored light</b> falling on the scene. Bright
              neutrals resist the tint (whites stay clean), highlights glow. The most
              natural, "lit" result.
            </p>
          </div>
        </div>
        <p className="edu-key">
          Try the same teal-shadow / warm-highlight grade in all three and watch the
          shadows and whites — Perceptual is boldest, Classic is coolest, Light is
          the most natural.
        </p>
        <Tie>
          Every model shares the same skin-protected saturation and constant-hue
          gamut compression on output, so no matter which you pick, colors stay in
          range and faces stay believable.
        </Tie>
      </>
    ),
  },
  {
    id: "sky",
    group: "Inside MeraRAW",
    title: "Bonus: Why Skies Are Blue",
    tagline: "The physics behind golden hour.",
    min: 2,
    body: (
      <>
        <p>
          Air scatters short (blue) wavelengths far more than long (red) ones —{" "}
          <b>Rayleigh scattering</b>. Overhead, you see that scattered blue. At
          sunset, light skims a long path through the atmosphere; the blue is
          scattered away before it reaches you, leaving the reds and oranges of
          golden hour.
        </p>
        <p>
          It's the same reason grades feel "true": warm highlights + cool shadows
          echo how real daylight actually falls. You're not inventing a look so much
          as leaning into physics your eye already trusts.
        </p>
      </>
    ),
  },
];

const GROUPS = ["Foundations", "The Color Map", "Working With Color", "Inside MeraRAW"];

export function Education() {
  const [activeId, setActiveId] = useState(GUIDES[0].id);
  const active = GUIDES.find((g) => g.id === activeId) ?? GUIDES[0];
  const idx = GUIDES.findIndex((g) => g.id === active.id);
  const prev = GUIDES[idx - 1];
  const next = GUIDES[idx + 1];

  return (
    <div className="edu">
      <nav className="edu-nav">
        <div className="edu-nav-head">
          <h2>Learn Color</h2>
          <p>The science behind the sliders.</p>
        </div>
        {GROUPS.map((group) => (
          <div key={group} className="edu-nav-group">
            <div className="edu-nav-grouptitle">{group}</div>
            {GUIDES.filter((g) => g.group === group).map((g) => (
              <button
                key={g.id}
                className={`edu-nav-item ${g.id === activeId ? "active" : ""}`}
                onClick={() => setActiveId(g.id)}
              >
                {g.title}
              </button>
            ))}
          </div>
        ))}
      </nav>

      <main className="edu-main">
        <article className="edu-article" key={active.id}>
          <div className="edu-article-kicker">{active.group} · {active.min} min read</div>
          <h1>{active.title}</h1>
          <p className="edu-article-tagline">{active.tagline}</p>
          <div className="edu-article-body">{active.body}</div>

          <div className="edu-article-nav">
            {prev ? (
              <button className="edu-pager" onClick={() => setActiveId(prev.id)}>
                <span>← Previous</span>
                <b>{prev.title}</b>
              </button>
            ) : <span />}
            {next ? (
              <button className="edu-pager edu-pager-next" onClick={() => setActiveId(next.id)}>
                <span>Next →</span>
                <b>{next.title}</b>
              </button>
            ) : <span />}
          </div>
        </article>
      </main>
    </div>
  );
}
