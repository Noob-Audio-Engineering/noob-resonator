/**
 * The design-mode stand-in for the engine — **development only.**
 *
 * This turns a set of parameter values into the three streams the Rust engine
 * publishes: `modes`, `response` and `info`. It exists so the panel has
 * something to render before the plug-in is running, and for nothing else.
 *
 * **The panel never imports this and cannot tell it apart from the engine**,
 * which is the whole point of putting it here. The page draws streams; this
 * fills streams; the engine fills the same streams with better numbers. If
 * the two ever disagree about a layout, this file is the one that is wrong.
 *
 * Every level below is invented and every ring time is a stated law with the
 * engine's own units and none of its constants. The panel stamps the display
 * while these are the source, so a screenshot taken in design mode cannot be
 * read as a measurement.
 *
 * Nothing here animates. Every generator is a pure function of the parameter
 * values, so the panel is a function of its controls and sits perfectly still
 * until one of them moves.
 */
import { objectById } from '../../objects.js';
import {
  columnLength,
  loopDelay,
  nodeWeight,
  openingPhase,
  modeIndices,
  ratiosOf,
  stretch,
} from './resonators.js';

/** The floor of the level axis, dB. */
export const DB_FLOOR = -66;

/**
 * How many partials this stand-in computes. The engine goes to four thousand
 * and ninety-six; a design-mode renderer does not need to, and the panel
 * prints "at least" when this is what ran out.
 */
export const PAGE_MAX_PARTIALS = 512;

/** How many partials the `modes` stream carries. */
export const PUBLISHED = 64;

/**
 * How many partials of this object a listener resolves as separate pitches
 * before they fuse into timbre — twelve to thirty on a bar or a string, two
 * to six on a membrane, whose partials are packed close enough to sit inside
 * one another's critical bands almost at once.
 *
 * **This is the number that says "more modes" is the wrong axis.** Above it
 * the answer is a statistically matched extension rather than an exact bank,
 * because an exact membrane is tens of thousands of modes.
 */
export const resolvable = (id) => (objectById(id).twoD ? 6 : 24);

/** Material, as the exponent of the damping law: `T(f) = decay · (f/f₀)^−γ`. */
export const dampExponent = (material) => 1 - material;

/** How long a partial rings, in seconds. Decay is the fundamental's; Material tilts the rest. */
export const ringSeconds = (hz, f0, decaySec, material) =>
  Math.max(0.001, decaySec * (Math.max(1e-6, hz) / Math.max(1e-6, f0)) ** -dampExponent(material));

/** How far Spread pulls the two channels apart at full travel, in cents. */
export const SPREAD_CENTS = 35;

const db = (a) => 20 * Math.log10(Math.max(1e-7, a));
const clampDb = (v) => Math.max(DB_FLOOR, v);

/** What the series generators need out of a state, in one place so two readers cannot differ. */
const optsOf = (s) => ({ ratio: s.ratio, opening: s.opening, barSecond: s.barSecond, barThird: s.barThird });

/**
 * Whether the partial list is **all** of them, or merely all this stand-in
 * has.
 *
 * The list stops for one of two reasons and they mean opposite things. Nyquist
 * is a fact about the object at this pitch — above it there is nothing, and
 * counting what is below is a real count. Running off the end of the
 * design-mode series table is a fact about the table, and publishing that
 * length as "the partials this object has" would be the false-ceiling bug
 * again, in the one place where nobody would think to look for it.
 *
 * So the caller publishes the count only when this is true, and leaves the
 * field unset otherwise — where the page already reads "not computed".
 */
export function allPartialsCounted(s) {
  const base = ratiosOf(objectById(s.object).id, PAGE_MAX_PARTIALS, optsOf(s));
  if (!base.length) return true;
  return s.f0 * base[base.length - 1] > s.nyquist;
}

/**
 * Every partial the object has, before the mode budget decides which run.
 *
 * `i` is the **physical partial index** and it is what an override addresses
 * — not a position in a list that Select is about to reorder.
 */
export function computePartials(s) {
  const o = objectById(s.object);
  const guide = o.engine === 'waveguide';
  const opts = optsOf(s);
  // What the object *has* is a fact about the object: the mode budget is
  // applied afterwards, and running the two together is what once had the
  // panel name the wrong limit as a wall.
  const base = ratiosOf(o.id, PAGE_MAX_PARTIALS, opts);
  const indices = modeIndices(o.id, PAGE_MAX_PARTIALS, opts);
  const shaped = guide ? base : stretch(base, s.inharm);
  const detune = 2 ** ((SPREAD_CENTS * s.spread) / 1200);
  // Overrides are keyed by the mode's own identity, not by where it lands in
  // a list that Selection reorders — and on a surface that identity is a
  // *pair*, because two different modes routinely share a first index.
  const key = (i, j) => `${i}:${j || 0}`;
  const byIndex = new Map((s.edits || []).map((e) => [key(e.i, e.j), e]));

  const out = [];
  for (let i = 0; i < shaped.length; i++) {
    const [mi, mj] = indices[i] || [i + 1, 0];
    const edit = byIndex.get(key(mi, mj));
    const r = shaped[i] * (edit?.cents ? 2 ** (edit.cents / 1200) : 1);
    const hz = s.f0 * r;
    if (hz > s.nyquist) break;
    const bare = 10 ** ((s.bright * Math.log2(Math.max(1e-6, r)) + (edit?.db || 0)) / 20);
    const strike = weight(o, i, s.hit, s.hitY, opts);
    const pickL = weight(o, i, s.posL, s.posLY, opts);
    const pickR = weight(o, i, s.posR, s.posRY, opts);
    out.push({
      /** Position in the computed series, used for the node weights only. */
      row: i,
      /** The mode's own identity. `mj` is 0 on an object that has only one index. */
      mi,
      mj,
      hz,
      /** Where this partial sits before Inharm and before any override moved it. */
      baseHz: s.f0 * base[i],
      /** The level before the strike and the pickups took their share, so the panel can draw what was removed. */
      bareDb: clampDb(db(bare)),
      dbL: clampDb(db(bare * strike * pickL)),
      dbR: clampDb(db(bare * strike * pickR * detune ** 0)),
      // The damping law is read at the partial's own frequency, so a pitch
      // override moves its ring time too: retune one an octave up and it is
      // already ringing half as long before the multiplier is applied.
      ring: ringSeconds(hz, s.f0, s.decay, s.material) * (edit?.decay ?? 1),
      strike,
      pickL,
      pickR,
    });
  }
  return out;
}

/** A contact point's weight on one mode. A one-dimensional object ignores the second axis. */
function weight(o, k, x, y, opts) {
  const wx = nodeWeight(o.id, k, x, opts);
  return o.twoD ? wx * nodeWeight(o.id, k, y, opts) : wx;
}

/**
 * Which of an object's partials the bank actually runs.
 *
 * `Lowest` takes partials 1 to N and throws away everything above, which is
 * what a plain mode count does implicitly and what Ableton's quality setting
 * does by their own description — a wall inside the audio band at a low
 * fundamental. `Loudest` keeps the ones you can hear wherever they sit.
 * `Log Spread` keeps the shape of the whole range.
 *
 * A choice about what is **synthesised**, not about what is drawn.
 */
export function selectPartials(list, mode, n) {
  if (list.length <= n) return list;
  let kept;
  if (mode === 'Lowest') {
    kept = list.slice(0, n);
  } else if (mode === 'Log Spread') {
    const lo = Math.log(list[0].hz);
    const hi = Math.log(list[list.length - 1].hz);
    const seen = new Set();
    kept = [];
    for (let k = 0; k < n; k++) {
      const target = lo + ((hi - lo) * k) / (n - 1);
      let best = -1;
      let bestD = Infinity;
      for (let j = 0; j < list.length; j++) {
        if (seen.has(j)) continue;
        const d = Math.abs(Math.log(list[j].hz) - target);
        if (d < bestD) {
          bestD = d;
          best = j;
        }
      }
      if (best >= 0) {
        seen.add(best);
        kept.push(list[best]);
      }
    }
  } else {
    kept = list
      .slice()
      .sort((a, b) => Math.max(b.dbL, b.dbR) - Math.max(a.dbL, a.dbR))
      .slice(0, n);
  }
  return kept.slice().sort((a, b) => a.hz - b.hz);
}

/**
 * The sixty-four the `modes` stream carries: the loudest of whatever the bank
 * is running.
 *
 * **A stream limit is not a wall.** The partials this leaves out are still
 * being synthesised, so the cut is always the loudest whatever Select is —
 * taking the lowest sixty-four to publish would show the panel a cliff that
 * is not there.
 */
export function loudest(list, n = PUBLISHED, edited = null) {
  if (list.length <= n) return list;
  const ranked = list.slice().sort((a, b) => Math.max(b.dbL, b.dbR) - Math.max(a.dbL, a.dbR));
  // **An edited mode is always published**, however quiet the edit made it —
  // which is what the engine does. Without it, turning a partial down drops
  // it out of the picture that shows the turning down, and drawing a falling
  // shape across the series makes most of what you drew disappear.
  const keep = [];
  const rest = [];
  for (const p of ranked) ((edited && edited.has(`${p.mi}:${p.mj || 0}`)) ? keep : rest).push(p);
  return [...keep, ...rest.slice(0, Math.max(0, n - keep.length))].sort((a, b) => a.hz - b.hz);
}

/**
 * The top of what the bank is running, when the bank ran out before the axis
 * did — the frequency above which the object has nothing at all.
 *
 * **`NaN` when there is no wall**, which is the engine's contract and reads as
 * *not applicable*: an air column has no budget to run out of, and a bank
 * holding every partial its object has threw nothing away. It used to be zero
 * here, which was the old contract and is exactly the design-versus-live
 * divergence this directory exists not to have — a stand-in that publishes a
 * plausible number where the engine publishes an absence teaches the panel the
 * wrong lesson in the one mode where nobody can check it against anything.
 */
export function ceilingHz(s, available, audible) {
  if (!audible.length) return NaN;
  if (objectById(s.object).engine === 'waveguide') return NaN;
  if (audible.length >= available.length) return NaN;
  const top = audible[audible.length - 1].hz;
  return top > s.nyquist * 0.92 ? NaN : top;
}

/**
 * The air column's magnitude response: the actual delay loop, evaluated.
 *
 * `H(f) = 1/(1 − ρ(f)·e^{−j(2πfT + ψ)})` with `ρ` derived from the same ring
 * time as everything else, so Decay and Material act on a pipe through the
 * law they act on a bar through. The excitation and pickup combs multiply in,
 * because injecting a wave a third of the way along a delay loop cancels
 * every third harmonic.
 *
 * Returned as `points` log-spaced samples normalised to the peak, which is
 * the layout the engine's `response` stream declares.
 */
export function guideResponse(s, points) {
  const psi = openingPhase(s.opening);
  const T = loopDelay(s.f0, s.opening);
  const lambdas = (1 - psi / (2 * Math.PI)) / 2;
  const comb = (f, u) => Math.abs(Math.sin(2 * Math.PI * (f / Math.max(1e-6, s.f0)) * lambdas * u));
  const at = (f) => {
    const rho = Math.min(0.99995, 10 ** ((-3 * T) / ringSeconds(f, s.f0, s.decay, s.material)));
    const th = 2 * Math.PI * f * T + psi;
    const denom = Math.sqrt(Math.max(1e-12, 1 - 2 * rho * Math.cos(th) + rho * rho));
    const tilted = 10 ** ((s.bright * Math.log2(Math.max(1e-6, f / s.f0))) / 20);
    return (tilted / denom) * comb(f, s.hit) * comb(f, s.posL);
  };
  const fLo = 20;
  const step = (s.nyquist / fLo) ** (1 / (points - 1));
  // Each published point is the largest the response reaches around it, so a
  // comb with more peaks than points is summarised rather than aliased.
  const SUB = 12;
  const raw = new Array(points);
  let peak = 1e-9;
  for (let i = 0; i < points; i++) {
    const f0 = fLo * step ** i;
    let hi = 0;
    for (let k = 0; k < SUB; k++) hi = Math.max(hi, at(f0 * step ** (k / (SUB - 1))));
    raw[i] = hi;
    if (hi > peak) peak = hi;
  }
  return raw.map((v) => clampDb(db(v / peak)));
}

/**
 * A mode bank's own magnitude response — the thing the bars cannot show.
 *
 * Where the partials sit is one half of what a resonator is; **how wide each
 * resonance is** is the other, and that is what Decay and Material are doing.
 * Two objects with identical partials and different ring times draw identical
 * bars and sound nothing alike, so the curve goes behind them.
 *
 * Each mode is one two-pole resonator, so its magnitude near its own
 * frequency is a Lorentzian whose half-width is the decay rate over 2π: an
 * amplitude falling as `e^−σt` reaches −60 dB at `T60`, so `σ = 3 ln10 / T60`.
 * Summed in power, because the partials are incoherent for this purpose, and
 * normalised to the peak — which is the layout the engine's `response`
 * stream declares.
 */
export function bankResponse(s, list, points) {
  const fLo = 20;
  const step = (s.nyquist / fLo) ** (1 / (points - 1));
  const peaks = list.map((p) => ({
    hz: p.hz,
    // Power at the peak, from the level the strike and the pickups left it.
    a: 10 ** (Math.max(p.dbL, p.dbR) / 10),
    // Half-width in hertz. Floored so a very long ring stays drawable at
    // this resolution rather than becoming an invisible spike.
    w: Math.max(0.5, (3 * Math.LN10) / Math.max(1e-4, p.ring) / (2 * Math.PI)),
  }));
  const out = new Array(points);
  let peak = 1e-12;
  for (let i = 0; i < points; i++) {
    const f = fLo * step ** i;
    let acc = 0;
    for (const q of peaks) {
      const d = f - q.hz;
      // Skip the ones too far away to matter; a Lorentzian is negligible
      // past a few hundred half-widths and this loop is 512 by 64.
      if (Math.abs(d) > q.w * 400 && Math.abs(d) > f * 0.5) continue;
      acc += (q.a * q.w * q.w) / (d * d + q.w * q.w);
    }
    out[i] = acc;
    if (acc > peak) peak = acc;
  }
  return out.map((v) => clampDb(db(Math.sqrt(v / peak))));
}

/** How long the air column is, and how long its round trip takes. */
export function columnFacts(s) {
  return { metres: columnLength(s.f0, s.opening, s.radius), loopS: loopDelay(s.f0, s.opening) };
}
