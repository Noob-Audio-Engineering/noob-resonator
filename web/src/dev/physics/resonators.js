/**
 * The equations behind each object's partial series — **development only, and
 * never the source of anything the panel shows when a plug-in is connected.**
 *
 * This whole directory is a quarantine. All of the mathematics in this
 * plug-in belongs to the Rust engine: it computes the partials, the levels
 * and the ring times, and publishes them on the `modes`, `response` and
 * `info` streams. The panel draws what arrives and derives none of it.
 *
 * What this file is for is the hour before that engine exists, and the hour
 * after somebody clones the repository and wants to look at the page. The
 * design manifest in `../manifest.js` uses it to generate the same three
 * streams the engine will, so the panel has something to render — and the
 * panel cannot tell the difference, because it only ever sees a stream.
 *
 * **Nothing outside `src/dev/` may import this**, and nothing does. It is
 * loaded from a dynamic import behind `import.meta.env.DEV`, so a production
 * build does not contain a byte of it.
 *
 * **The results here are worth keeping even so.** Two of them came out of
 * writing the tests: the beam ratio everyone quotes, 2.756, is a truncation
 * of 2.75654 and not a rounding of it — the correctly rounded figure is
 * 2.757 — and an undercut bar has no closed form at all, so its ratios are a
 * maker's target rather than a solution. Both belong in the engine's own
 * tests, where they will guard the numbers that actually ship.
 */

/** Speed of sound in air at 20 °C, m/s. Only the air columns use it, and the panel prints it beside every length it derives. */
export const C_AIR = 343;

// ---------------------------------------------------------------------------
// Free–free beam
// ---------------------------------------------------------------------------

/**
 * The eigenvalues `βL` of a free–free uniform beam: the roots of
 * `cos(x)·cosh(x) = 1`, solved here rather than tabulated.
 *
 * Written as `cos(x) − sech(x) = 0`, which is the same equation with the
 * overflow taken out — `cosh(45)` is 1.7e19 and the product form loses every
 * digit it had. The roots sit just above `(2n+1)π/2`, which is where Newton
 * starts, and `sech` vanishes fast enough that the high ones land on that
 * asymptote to machine precision.
 *
 * @param {number} n How many roots.
 * @returns {number[]} `[4.7300, 7.8532, 10.9956, 14.1372, …]`
 */
export function beamEigenvalues(n) {
  const out = [];
  for (let i = 1; i <= n; i++) {
    let x = ((2 * i + 1) * Math.PI) / 2;
    for (let k = 0; k < 40; k++) {
      const sech = 1 / Math.cosh(x);
      const f = Math.cos(x) - sech;
      const df = -Math.sin(x) + sech * Math.tanh(x);
      const dx = f / df;
      x -= dx;
      if (Math.abs(dx) < 1e-14) break;
    }
    out.push(x);
  }
  return out;
}

/**
 * Partial ratios of a free–free uniform bar. A beam's frequencies go as the
 * *square* of the eigenvalue, which is why the series climbs so fast and why
 * a glockenspiel clangs rather than sings.
 *
 * @param {number} n How many partials.
 * @returns {number[]} `[1, 2.756, 5.404, 8.933, …]`
 */
export function beamRatios(n) {
  const e = beamEigenvalues(Math.max(1, n));
  return e.map((x) => (x / e[0]) ** 2);
}

/**
 * The mode shape of a free–free beam, `X_n(u)` for `u` along the bar.
 *
 * `X(u) = cosh(xu) + cos(xu) − σ(sinh(xu) + sin(xu))`, with
 * `σ = (cosh x − cos x)/(sinh x − sin x)`.
 *
 * The growing and decaying exponentials are separated before they are added,
 * because `σ → 1` as `x` grows and `cosh(xu) − σ·sinh(xu)` is then the
 * difference of two enormous and nearly equal numbers. Writing it as
 * `((1−σ)e^{xu} + (1+σ)e^{−xu})/2` and computing `1−σ` from its own closed
 * form keeps every digit.
 *
 * This is what makes a marimba bar's suspension cord go where it does: the
 * first mode's nodes are at 0.224 and 0.776 of the length, so a bar hung
 * there is held at the two points its fundamental does not move.
 *
 * @param {number} x The eigenvalue `βL` for this mode.
 * @param {number} u Position along the bar, 0 to 1.
 * @returns {number} The shape, ±2 at the free ends.
 */
export function beamShape(x, u) {
  const sinh = Math.sinh(x);
  const sin = Math.sin(x);
  const cos = Math.cos(x);
  // 1 − σ, from (sinh x − cosh x) = −e^−x. Never the subtraction of two
  // large numbers, however big x gets.
  const oneMinusSigma = (cos - sin - Math.exp(-x)) / (sinh - sin);
  const sigma = 1 - oneMinusSigma;
  const a = x * u;
  const growing = oneMinusSigma * Math.exp(a);
  const decaying = (1 + sigma) * Math.exp(-a);
  return (growing + decaying) / 2 + Math.cos(a) - sigma * Math.sin(a);
}

// ---------------------------------------------------------------------------
// Rectangular membrane and plate
// ---------------------------------------------------------------------------

/** How far the `(a, b)` mode indices run when the eigenvalues are enumerated. Well past any partial count the panel offers. */
const RECT_ORDER = 24;

/**
 * The mode indices and eigenvalues of a rectangle of aspect `r`, sorted and
 * normalised so the first is 1.
 *
 * `f(a,b) ∝ √(a² + b²/r²)` for a tensioned membrane. Two indices run
 * independently, so the series is dense and has no common divisor — which is
 * exactly why a drum is not pitched the way a string is, and why raising the
 * partial count on a membrane fills the axis in rather than extending it.
 *
 * @param {number} n How many partials.
 * @param {number} r Aspect ratio of the rectangle.
 * @returns {{ ratio: number, a: number, b: number }[]}
 */
export function rectModes(n, r = 1) {
  const all = [];
  for (let a = 1; a <= RECT_ORDER; a++) {
    for (let b = 1; b <= RECT_ORDER; b++) all.push({ a, b, ratio: Math.sqrt(a * a + (b * b) / (r * r)) });
  }
  all.sort((x, y) => x.ratio - y.ratio);
  const f0 = all[0].ratio;
  return all.slice(0, n).map((m) => ({ ...m, ratio: m.ratio / f0 }));
}

// ---------------------------------------------------------------------------
// Circular membrane
// ---------------------------------------------------------------------------

/**
 * The Bessel function of the first kind, from its integral form:
 * `J_m(x) = (1/π) ∫₀^π cos(mθ − x sin θ) dθ`.
 *
 * The power series is the usual way and it is the wrong way here: by the
 * time `x` is past twenty its terms alternate through numbers far larger
 * than the answer and the result is noise. The integrand is bounded by one
 * for every `x`, so Simpson's rule on it cannot lose what it never had, and
 * the panel needs zeros out past forty.
 */
export function besselJ(m, x) {
  const N = 512;
  const h = Math.PI / N;
  // The two endpoints, sin θ being zero at both, then Simpson's alternating weights.
  let sum = 1 + Math.cos(m * Math.PI);
  for (let i = 1; i < N; i++) {
    const th = i * h;
    sum += (i % 2 ? 4 : 2) * Math.cos(m * th - x * Math.sin(th));
  }
  return (sum * h) / 3 / Math.PI;
}

/** `dJ_m/dx = (J_{m−1} − J_{m+1})/2`, for the Newton step onto a zero. */
const besselJPrime = (m, x) => (besselJ(m - 1, x) - besselJ(m + 1, x)) / 2;

/**
 * The first `n` positive zeros of `J_m`, solved rather than tabulated.
 *
 * McMahon's asymptotic expansion puts the starting point within a hundredth
 * of the answer even for the first zero, and Newton closes it. The same
 * discipline as the beam's eigenvalues: the panel prints what the solver
 * gives, and `test/modes.test.js` checks the solver against the definition.
 *
 * @returns {number[]} For `m = 0`: `[2.4048, 5.5201, 8.6537, …]`
 */
export function besselZeros(m, n) {
  const out = [];
  const mu = 4 * m * m;
  for (let k = 1; k <= n; k++) {
    const beta = (k + m / 2 - 0.25) * Math.PI;
    let x =
      beta -
      (mu - 1) / (8 * beta) -
      (4 * (mu - 1) * (7 * mu - 31)) / (3 * (8 * beta) ** 3);
    for (let i = 0; i < 60; i++) {
      const d = besselJ(m, x) / besselJPrime(m, x);
      x -= d;
      if (Math.abs(d) < 1e-13) break;
    }
    out.push(x);
  }
  return out;
}

/** How far the `(m, n)` indices run when a circular membrane's modes are enumerated. */
const CIRCLE_ORDER = 24;

/**
 * The modes of a circular membrane fixed at the rim, sorted and normalised
 * so the first is 1.
 *
 * `f(m,n) ∝ j_{m,n}`, the `n`th zero of the `m`th Bessel function — `m`
 * counting nodal diameters and `n` nodal circles. The series opens
 * 1 : 1.593 : 2.136 : 2.296 : 2.653, which shares no common divisor with
 * anything and is why a drum head has no pitch to speak of.
 *
 * **This is the one object Corpus does not have.** Their Membrane is a
 * rectangle with an aspect ratio, and a drum head is a circle; the two have
 * genuinely different series, and a circle has no aspect to set, so Ratio is
 * meaningless on it rather than merely unused.
 */
let circleCache = null;
export function circleModes(n) {
  if (!circleCache) {
    const all = [];
    for (let m = 0; m <= CIRCLE_ORDER; m++) {
      for (const z of besselZeros(m, CIRCLE_ORDER)) all.push({ m, zero: z });
    }
    all.sort((a, b) => a.zero - b.zero);
    const f0 = all[0].zero;
    circleCache = all.map((e) => ({ ...e, ratio: e.zero / f0 }));
  }
  return circleCache.slice(0, n);
}

// ---------------------------------------------------------------------------
// The air column
// ---------------------------------------------------------------------------

/**
 * The round-trip phase offset of the loop, in radians, for an `opening` of 0
 * to 1.
 *
 * A stopped pipe reflects `+1` at the closed end and `−1` at the open one, so
 * the round trip inverts: half a turn of phase, and only the odd harmonics
 * survive. A tube open at both ends reflects `−1` twice, the round trip is
 * back where it started, and the whole series is there. **That single sign is
 * the entire difference between the two objects**, and because it is a phase
 * rather than a switch it has a continuum between its ends.
 */
export const openingPhase = (opening) => Math.PI * (1 - opening);

/**
 * Partial ratios of an air column at a given opening.
 *
 * Resonance is where the round trip comes back in phase: `2πfT + ψ = 2πk`.
 * Pinning the first resonance to the fundamental fixes the loop delay at
 * `T = (1 − ψ/2π)/f₀`, and every other partial follows.
 *
 * Stopped (`ψ = π`) gives 1, 3, 5, 7 …; open (`ψ = 0`) gives 1, 2, 3, 4 …;
 * and the sweep between them slides the upper partials continuously, which is
 * what makes Opening a real control rather than a second type switch.
 *
 * @param {number} n How many partials.
 * @param {number} opening 0 stopped, 1 open at both ends.
 */
export function guideRatios(n, opening = 1) {
  const psi = openingPhase(opening) / (2 * Math.PI);
  return Array.from({ length: n }, (_, i) => (i + 1 - psi) / (1 - psi));
}

/**
 * How long the air column actually is, in metres, for a fundamental and an
 * opening — the tube the engine is standing in for.
 *
 * The column is `Λ` wavelengths long, `Λ = (1 − ψ/2π)/2`: half a wavelength
 * open at both ends, a quarter stopped. So **a stopped pipe is half the
 * length of an open one at the same pitch**, which is the same fact as "the
 * same pipe stopped sounds an octave lower", read the other way round.
 *
 * @param {number} f0 Fundamental, Hz.
 * @param {number} opening 0 to 1.
 * @param {number} radiusMm Bore radius, for the end correction.
 */
export function columnLength(f0, opening, radiusMm = 0) {
  const lambdas = (1 - openingPhase(opening) / (2 * Math.PI)) / 2;
  // An open end behaves as though the column ran on a little past it; the
  // classic unflanged figure is 0.6 of the bore radius, and there is one such
  // end for every end that is open.
  const correction = 0.6 * (radiusMm / 1000) * (1 + opening);
  return Math.max(0, (C_AIR * lambdas) / Math.max(1e-6, f0) - correction);
}

/** The loop delay, in seconds, that puts an air column's first resonance on `f0`. */
export const loopDelay = (f0, opening) => (1 - openingPhase(opening) / (2 * Math.PI)) / Math.max(1e-6, f0);

// ---------------------------------------------------------------------------
// The objects
// ---------------------------------------------------------------------------

/**
 * The marimba's tuned partials, as the two choices a maker actually makes.
 *
 * A marimba bar is deliberately **not** a uniform bar: the maker cuts an arch
 * out of the underside, which lowers the second partial far more than the
 * first and lands it on a low whole ratio. That retuning is the whole reason
 * it reads as pitched where a glockenspiel clangs, and the whole reason this
 * is a separate model rather than the beam with a filter on it.
 *
 * **Neither number is a constant, and the panel does not pretend otherwise.**
 * The second partial is cut to 4 — two octaves — for a marimba and to 3 — a
 * twelfth — for a xylophone, and both instruments exist. The third is given
 * as about 9.2 in one source and 10 in another, and that is not a discrepancy
 * to average away: it depends on how deeply the bar is undercut, so it is a
 * builder's choice and both values are right for the bar their author was
 * holding. Averaging two sources into 9.6 would produce a bar nobody has ever
 * made. So both are on the panel, as controls.
 */
export const BAR_SECOND = [4, 3];
export const BAR_THIRD = [9.2, 10];

/**
 * A tuned bar's series: the two chosen partials, and then the beam again.
 *
 * Above the third the undercut stops controlling anything and the bar goes
 * back to behaving like a uniform beam, so that is what the series does —
 * scaled to continue from wherever the tuned third was put.
 */
export function barRatios(n, second = BAR_SECOND[0], third = BAR_THIRD[0]) {
  const tuned = [1, second, third].slice(0, n);
  if (n <= 3) return tuned;
  const beam = beamRatios(n);
  const scale = third / beam[2];
  return tuned.concat(beam.slice(3).map((r) => r * scale));
}

/**
 * The series and mode shape for each object, keyed by the id the catalogue in
 * `src/objects.js` uses. Physics only: what a thing is called, what it is for
 * and where its numbers are cited from all live in the catalogue, because
 * those are things the panel prints and these are things the engine computes.
 *
 * @type {Record<string, { ratios: (n: number, o: object) => number[], shape: (k: number, u: number, o: object) => number }>}
 */
export const SERIES = {
  beam: {
    ratios: (n) => beamRatios(n),
    shape: (k, u) => beamShapeNorm(k, u),
  },
  marimba: {
    ratios: (n, o = {}) => barRatios(n, o.barSecond ?? BAR_SECOND[0], o.barThird ?? BAR_THIRD[0]),
    shape: (k, u) => beamShapeNorm(k, u),
  },
  string: {
    ratios: (n) => Array.from({ length: n }, (_, i) => i + 1),
    shape: (k, u) => Math.sin((k + 1) * Math.PI * u),
  },
  membrane: {
    ratios: (n, o = {}) => rectModes(n, o.ratio ?? 1).map((m) => m.ratio),
    shape: (k, u, o = {}) => rectShape(k, u, o.ratio ?? 1),
  },
  plate: {
    ratios: (n, o = {}) => rectModes(n, o.ratio ?? 1).map((m) => m.ratio ** 2),
    shape: (k, u, o = {}) => rectShape(k, u, o.ratio ?? 1),
  },
  pipe: {
    ratios: (n, o = {}) => guideRatios(n, o.opening ?? 0),
    shape: (k, u, o = {}) => guideShape(k, u, o.opening ?? 0),
  },
  tube: {
    ratios: (n, o = {}) => guideRatios(n, o.opening ?? 1),
    shape: (k, u, o = {}) => guideShape(k, u, o.opening ?? 1),
  },
  membrane_round: {
    ratios: (n) => circleModes(n).map((m) => m.ratio),
    shape: (k, u) => circleShape(k, u),
  },
};

/**
 * How hard mode `k` is driven by a strike (or heard by a pickup) at `u`.
 *
 * **Striking at a node gives that mode no energy at all.** It is the same
 * reason plucking a string at a fifth of its length kills the fifth harmonic,
 * and the same reason a marimba bar hangs from a cord threaded through it at
 * 0.224 of its length: those are the two places the fundamental does not
 * move, so the cord takes nothing from it.
 */
export function nodeWeight(id, k, u, opts = {}) {
  const e = SERIES[id] || SERIES.string;
  return Math.abs(e.shape(k, Math.min(1, Math.max(0, u)), opts));
}

/** The partial ratios of an object, by id. */
export const ratiosOf = (id, n, opts = {}) => (SERIES[id] || SERIES.string).ratios(n, opts);

// ---------------------------------------------------------------------------
// Mode shapes, normalised
// ---------------------------------------------------------------------------

/**
 * Beam mode shapes, normalised so an antinode reads 1.
 *
 * The peak is found on a grid once per mode and cached, because the panel
 * asks for these on every frame and the eigenvalue solve underneath is not
 * free.
 */
const beamPeak = new Map();
function beamShapeNorm(k, u) {
  let e = beamPeak.get(k);
  if (e === undefined) {
    const x = beamEigenvalues(k + 1)[k];
    let peak = 0;
    for (let i = 0; i <= 256; i++) peak = Math.max(peak, Math.abs(beamShape(x, i / 256)));
    e = { x, peak: peak || 1 };
    beamPeak.set(k, e);
  }
  return beamShape(e.x, u) / e.peak;
}

/**
 * A rectangle's mode shape, read along the diagonal.
 *
 * `X(u,v) = sin(aπu)·sin(bπv)` over the two dimensions, and Hit and Pos are
 * one number each, so the panel walks the diagonal: `u = v`. **That is a
 * choice the panel is making, not something the physics forced** — a real
 * strike lands somewhere in a square and has two coordinates. The diagonal is
 * the path that meets every mode's nodes in both indices, so it is the one
 * that shows the effect the control exists for.
 */
const rectCache = new Map();
function rectShape(k, u, r) {
  const key = `${r}`;
  let list = rectCache.get(key);
  if (!list) {
    list = rectModes(RECT_ORDER * RECT_ORDER, r);
    rectCache.set(key, list);
  }
  const m = list[Math.min(k, list.length - 1)];
  return Math.sin(m.a * Math.PI * u) * Math.sin(m.b * Math.PI * u);
}

/**
 * A circular head's mode shape along a radius: `J_m(j_{mn}·u)`, with `u`
 * running from the centre to the rim.
 *
 * Its nodes are **circles**, not points, which is a genuinely different thing
 * to strike than a bar or a string: every mode is zero at the rim, so hitting
 * a drum on its edge takes almost nothing from anything, and the nodal
 * circles of the higher modes march inward. The panel walks the radius,
 * because Hit and Pos are one number each.
 */
const circlePeak = new Map();
function circleShape(k, u) {
  const modes = circleModes(k + 1);
  const m = modes[Math.min(k, modes.length - 1)];
  let peak = circlePeak.get(k);
  if (peak === undefined) {
    peak = 0;
    for (let i = 0; i <= 128; i++) peak = Math.max(peak, Math.abs(besselJ(m.m, m.zero * (i / 128))));
    circlePeak.set(k, peak || 1);
  }
  return besselJ(m.m, m.zero * u) / peak;
}

/**
 * The standing wave of an air column at partial `k`, at a fraction `u` of the
 * way from the far end to the mouth.
 *
 * The column is `Λ = (1 − ψ/2π)/2` wavelengths of the fundamental long, so
 * partial `k` turns `2π·(fₖ/f₀)·Λ` radians of phase across it. Stopped, that
 * puts a node hard against the closed end; open, an antinode at both. One
 * expression covers the whole sweep, which is the point: **the excitation
 * point works on a waveguide for the same reason it works on a string**, and
 * the panel does not need a second story for it.
 */
function guideShape(k, u, opening) {
  const psi = openingPhase(opening) / (2 * Math.PI);
  const lambdas = (1 - psi) / 2;
  const rk = (k + 1 - psi) / (1 - psi);
  return Math.sin(2 * Math.PI * rk * lambdas * u);
}


// ---------------------------------------------------------------------------
// Inharmonicity
// ---------------------------------------------------------------------------

/**
 * Inharm, applied to a series.
 *
 * A stretch on the log-frequency axis: `r → r^(1+k)`, which leaves the
 * fundamental where it is and pushes everything above it out (or pulls it
 * in), keeping the order and never folding two partials through each other.
 *
 * **This is not the stiff-string law and the panel does not claim it is.**
 * A real string's stiffness gives `fₙ = n·f₁·√(1 + Bn²)`, which is one
 * object's physics; this control has to do something sensible on a plate and
 * a drum head as well, so it is a stretch rather than a stiffness. What it
 * does have in common with the real thing is the direction and the shape:
 * upper partials sharp, and progressively more so.
 */
export const STRETCH_RANGE = 0.35;
export const stretch = (ratios, inharm) =>
  inharm === 0 ? ratios : ratios.map((r) => r ** (1 + inharm * STRETCH_RANGE));
