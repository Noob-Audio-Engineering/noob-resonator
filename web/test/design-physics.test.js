/**
 * The design-mode physics, held to its own equations.
 *
 * **This does not test the plug-in.** The mathematics that ships belongs to
 * the Rust engine, and the engine's own tests guard it. What this guards is
 * `src/dev/physics/` — the equations the page uses to fill its three streams
 * before a plug-in is running, so that the panel can be built and looked at.
 * If they drift, the thing anybody looks at while designing is wrong.
 *
 * **Two results in here are worth more than that**, and both came out of
 * writing these tests rather than out of reading a source:
 *
 * * The beam ratio quoted everywhere as 2.756 is a **truncation** of 2.75654,
 *   not a rounding of it — correctly rounded it is 2.757. Computing it rather
 *   than quoting it is the only way that stays true.
 * * An undercut bar has **no closed form**. Its ratios are a maker's tuning
 *   target, and the two published values for its third partial are a
 *   builder's choice rather than a discrepancy to average away.
 *
 * Both belong in the engine's tests, where they will guard the numbers that
 * actually ship. They are kept here until they are carried across.
 *
 * It deliberately asserts nothing about amplitudes: those are invented until
 * the engine publishes them, and a test that checked the model against itself
 * would be the bug rather than the evidence.
 *
 *   npm test
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  barRatios,
  beamEigenvalues,
  beamRatios,
  beamShape,
  besselJ,
  besselZeros,
  circleModes,
  ratiosOf,
  columnLength,
  guideRatios,
  nodeWeight,
  rectModes,
  stretch,
  BAR_SECOND,
  BAR_THIRD,
  C_AIR,
} from '../src/dev/physics/resonators.js';
import { OBJECTS } from '../src/objects.js';
import {
  ceilingHz,
  computePartials,
  dampExponent,
  resolvable,
  ringSeconds,
  selectPartials,
  loudest,
  PUBLISHED,
} from '../src/dev/physics/model.js';

const close = (a, b, eps, what) =>
  assert.ok(Math.abs(a - b) <= eps, `${what}: ${a} is not within ${eps} of ${b}`);

/** Zero crossings of `f` on [0,1], found by bisection on a fine grid. */
function zeros(f, steps = 4000) {
  const out = [];
  let prev = f(0);
  for (let i = 1; i <= steps; i++) {
    const u = i / steps;
    const cur = f(u);
    if (prev === 0) out.push((i - 1) / steps);
    else if (prev * cur < 0) {
      let lo = (i - 1) / steps;
      let hi = u;
      for (let k = 0; k < 60; k++) {
        const mid = (lo + hi) / 2;
        if (f(lo) * f(mid) <= 0) hi = mid;
        else lo = mid;
      }
      out.push((lo + hi) / 2);
    }
    prev = cur;
  }
  return out;
}

// ---------------------------------------------------------------------------

test('the beam eigenvalues really do solve cos x · cosh x = 1', () => {
  for (const x of beamEigenvalues(8)) {
    // Checked in the form that does not overflow: cos x = sech x.
    close(Math.cos(x), 1 / Math.cosh(x), 1e-12, `residual at x = ${x}`);
  }
});

test('and they are the values the literature gives', () => {
  const e = beamEigenvalues(4);
  const want = [4.730041, 7.853205, 10.995608, 14.137165];
  want.forEach((w, i) => close(e[i], w, 5e-6, `eigenvalue ${i + 1}`));
});

test('the beam ratios the panel prints come out of those eigenvalues', () => {
  // Frequencies go as the square of the eigenvalue, so these follow from the
  // roots above and nothing else.
  const r = beamRatios(4);
  [1, 2.756538507, 5.403917632, 8.932950352].forEach((w, i) => close(r[i], w, 1e-9, `beam partial ${i + 1}`));
});

test('the second partial is 2.7565, and the usual 2.756 is a truncation of it', () => {
  // Worth pinning, because "1 : 2.756 : 5.404 : 8.933" is quoted everywhere
  // and only two of those three are correctly rounded. The exact value is
  // 2.75654, which rounds to 2.757. The panel prints what the solver gives
  // rather than the quotation, which is why this test exists.
  const r = beamRatios(4);
  close(Number(r[1].toFixed(4)), 2.7565, 1e-9, 'four places');
  assert.equal(Number(r[1].toFixed(3)), 2.757, 'three places, correctly rounded');
  assert.notEqual(Number(r[1].toFixed(3)), 2.756, 'the quoted figure is not the rounded one');
});

test('a free–free beam mode has one more node than its index', () => {
  const e = beamEigenvalues(6);
  e.forEach((x, i) => {
    const n = zeros((u) => beamShape(x, u)).length;
    assert.equal(n, i + 2, `mode ${i + 1} should have ${i + 2} nodes, found ${n}`);
  });
});

test('the first mode’s nodes are where a marimba bar’s cord goes', () => {
  // 0.2242 and 0.7758 of the length: the two points the fundamental does not
  // move, which is why a bar hung there keeps ringing.
  const [x] = beamEigenvalues(1);
  const n = zeros((u) => beamShape(x, u));
  close(n[0], 0.2242, 5e-4, 'lower node');
  close(n[1], 0.7758, 5e-4, 'upper node');
});

test('the beam mode shape survives the high modes it is asked for', () => {
  // The naive form is the difference of two numbers around 1e19 by mode 20.
  const e = beamEigenvalues(48);
  for (const x of e) {
    for (let i = 0; i <= 32; i++) {
      const v = beamShape(x, i / 32);
      assert.ok(Number.isFinite(v), `beamShape(${x}, ${i / 32}) is ${v}`);
      assert.ok(Math.abs(v) < 4, `beamShape(${x}, ${i / 32}) = ${v} is out of range`);
    }
  }
});

test('hanging the bar on its node takes nothing from that mode', () => {
  const beam = 'beam';
  close(nodeWeight(beam, 0, 0.2242), 0, 2e-3, 'fundamental at its own node');
  close(nodeWeight(beam, 0, 0), 1, 1e-9, 'free end is an antinode');
});

// ---------------------------------------------------------------------------

test('a string is the integers, and plucking at 1/5 kills the fifth', () => {
  const s = 'string';
  ratiosOf(s, 8).forEach((r, i) => close(r, i + 1, 1e-12, `string partial ${i + 1}`));
  close(nodeWeight(s, 4, 0.2), 0, 1e-9, 'fifth partial, plucked at a fifth');
  close(nodeWeight(s, 0, 0.5), 1, 1e-9, 'fundamental, plucked at the middle');
  // The neighbours are untouched, which is what makes it a null and not a filter.
  assert.ok(nodeWeight(s, 3, 0.2) > 0.5, 'fourth partial should survive');
});

test('a tuned bar puts partials 2 and 3 where the maker chose, and is a beam above them', () => {
  const r = barRatios(6);
  close(r[0], 1, 1e-12, 'the fundamental');
  close(r[1], 4, 1e-12, 'the marimba’s second, two octaves');
  close(r[2], 9.2, 1e-12, 'the marimba’s third');
  for (let i = 1; i < r.length; i++) assert.ok(r[i] > r[i - 1], `partial ${i + 1} must be above ${i}`);
  // The whole point of the undercut: partial 2 lands on a whole ratio where
  // the untouched bar had it at 2.7565.
  assert.ok(beamRatios(2)[1] < r[1], 'the undercut raises partial 2 relative to the fundamental');
});

test('both builder’s choices are honoured rather than averaged', () => {
  // Neither pair is a constant: 4:1 is a marimba and 3:1 a xylophone, and the
  // third partial is quoted at both 9.2 and 10 because it depends on how deep
  // the arch is cut. A page that averaged them would describe a bar nobody
  // has made, so both are reachable and this checks they arrive intact.
  assert.deepEqual(BAR_SECOND, [4, 3]);
  assert.deepEqual(BAR_THIRD, [9.2, 10]);
  for (const second of BAR_SECOND) {
    for (const third of BAR_THIRD) {
      const r = barRatios(8, second, third);
      close(r[1], second, 1e-12, `second at ${second}`);
      close(r[2], third, 1e-12, `third at ${third}`);
      for (let i = 1; i < r.length; i++) assert.ok(r[i] > r[i - 1], `ordered at ${second}/${third}`);
    }
  }
  // And the choice actually reaches the object.
    close(ratiosOf('marimba', 3, { barSecond: 3, barThird: 10 })[1], 3, 1e-12, 'a xylophone through the type');
});

// ---------------------------------------------------------------------------

test('the Bessel integral gives the values it should', () => {
  close(besselJ(0, 0), 1, 1e-12, 'J₀(0)');
  close(besselJ(1, 0), 0, 1e-12, 'J₁(0)');
  close(besselJ(0, 1), 0.7651976866, 1e-9, 'J₀(1)');
  close(besselJ(1, 1), 0.4400505857, 1e-9, 'J₁(1)');
  close(besselJ(0, 10), -0.2459357644, 1e-9, 'J₀(10)');
  // Past twenty is where the power series gives up and the integral does not.
  // Checked by convergence rather than by memory: this value is stable to
  // twelve places from 256 Simpson points to 16384.
  close(besselJ(0, 30), -0.086367983581, 1e-11, 'J₀(30)');
});

test('the circular membrane’s zeros really are zeros', () => {
  for (let m = 0; m <= 4; m++) {
    for (const z of besselZeros(m, 5)) close(besselJ(m, z), 0, 1e-11, `J${m}(${z})`);
  }
});

test('and they are the values the tables give', () => {
  const j0 = besselZeros(0, 3);
  [2.404826, 5.520078, 8.653728].forEach((w, i) => close(j0[i], w, 1e-6, `j₀,${i + 1}`));
  const j1 = besselZeros(1, 2);
  [3.831706, 7.015587].forEach((w, i) => close(j1[i], w, 1e-6, `j₁,${i + 1}`));
});

test('a drum head’s series is the sorted Bessel zeros, and it is not the rectangle’s', () => {
  const got = circleModes(6).map((m) => m.ratio);
  [1, 1.5933, 2.1355, 2.2954, 2.6531, 2.9173].forEach((w, i) => close(got[i], w, 5e-5, `partial ${i + 1}`));
  // The reason the eighth object exists: a circle is not a rectangle.
  const rect = rectModes(6, 1).map((m) => m.ratio);
  assert.ok(Math.abs(got[1] - rect[1]) > 0.01, 'a round head and a square one differ from the second partial on');
});

test('every mode of a drum head is a node at the rim, and only the round ones live at the centre', () => {
  const c = 'membrane_round';
  for (let k = 0; k < 8; k++) close(nodeWeight(c, k, 1), 0, 1e-9, `partial ${k + 1} at the rim`);
  // The fundamental is circularly symmetric, so the centre is its antinode.
  close(nodeWeight(c, 0, 0), 1, 1e-9, 'the fundamental at the centre');
  // The second and third have a nodal diameter through the middle, which is
  // why striking a drum dead centre gives a duller, more pitched sound.
  close(nodeWeight(c, 1, 0), 0, 1e-9, 'the second at the centre');
  close(nodeWeight(c, 2, 0), 0, 1e-9, 'the third at the centre');
});

test('the object list is the frozen index order and is append-only', () => {
  // A saved project's object is its index, so nothing here may ever move.
  // The first seven are Corpus's own order; the eighth is ours.
  assert.deepEqual(
    OBJECTS.map((t) => t.id),
    ['beam', 'marimba', 'string', 'membrane', 'plate', 'pipe', 'tube', 'membrane_round'],
  );
  assert.deepEqual(
    OBJECTS.map((t) => t.engine),
    ['modal', 'modal', 'modal', 'modal', 'modal', 'waveguide', 'waveguide', 'modal'],
  );
});

// ---------------------------------------------------------------------------

test('a square membrane’s partials are the sorted √(a²+b²)', () => {
  // Both mode indices run independently, so (1,2) and (2,1) are the same
  // frequency and every off-diagonal mode arrives twice. Leaving the repeats
  // out would be leaving out the reason the series is as dense as it is.
  const got = rectModes(8, 1).map((m) => m.ratio);
  const want = [2, 5, 5, 8, 10, 10, 13, 13].map((v) => Math.sqrt(v / 2));
  want.forEach((w, i) => close(got[i], w, 1e-12, `membrane partial ${i + 1}`));
  // Dense and with no common divisor, which is why a drum has no pitch.
  assert.ok(got[1] / got[0] < 1.6, 'the second partial sits well inside an octave');
});

test('a plate is the membrane family squared', () => {
  const mem = rectModes(12, 1.4).map((m) => m.ratio);
  const plate = ratiosOf('plate', 12, { ratio: 1.4 });
  plate.forEach((p, i) => close(p, mem[i] ** 2, 1e-12, `plate partial ${i + 1}`));
  // So it climbs much faster: an octave of membrane is two octaves of plate.
  assert.ok(plate[11] > mem[11] * 3, 'the plate series should outrun the membrane');
});

test('Ratio changes a rectangle’s series and a square is the degenerate case', () => {
  const square = rectModes(6, 1).map((m) => m.ratio);
  const oblong = rectModes(6, 2.5).map((m) => m.ratio);
  assert.notDeepEqual(square, oblong);
  // A square has every off-diagonal mode twice over; stretching it splits them.
  close(square[1], square[2], 1e-12, 'the square’s degenerate pair');
  assert.ok(Math.abs(oblong[1] - oblong[2]) > 1e-3, 'the oblong splits that pair');
});

// ---------------------------------------------------------------------------

test('an air column stopped at one end gives the odd harmonics only', () => {
  guideRatios(6, 0).forEach((r, i) => close(r, 2 * i + 1, 1e-12, `stopped partial ${i + 1}`));
});

test('and open at both ends gives the whole series', () => {
  guideRatios(6, 1).forEach((r, i) => close(r, i + 1, 1e-12, `open partial ${i + 1}`));
});

test('Opening moves between them continuously rather than switching', () => {
  const mid = guideRatios(4, 0.5);
  const stopped = guideRatios(4, 0);
  const open = guideRatios(4, 1);
  for (let i = 1; i < 4; i++) {
    assert.ok(mid[i] > open[i] && mid[i] < stopped[i], `partial ${i + 1} should be between the two ends`);
    assert.ok(mid[i] > mid[i - 1], 'the series must stay ordered through the sweep');
  }
  // Nothing jumps: a small step in Opening is a small step in every partial.
  const a = guideRatios(4, 0.5);
  const b = guideRatios(4, 0.51);
  a.forEach((v, i) => assert.ok(Math.abs(v - b[i]) < 0.1, `partial ${i + 1} jumped`));
});

test('the octave: a stopped pipe is half the air of an open one at the same pitch', () => {
  const stopped = columnLength(220, 0, 0);
  const open = columnLength(220, 1, 0);
  close(stopped, C_AIR / (4 * 220), 1e-12, 'quarter wave');
  close(open, C_AIR / (2 * 220), 1e-12, 'half wave');
  close(open / stopped, 2, 1e-12, 'the ratio is exactly two');
});

test('the far end of a stopped pipe is a node for every partial', () => {
  const pipe = 'pipe';
  for (let k = 0; k < 6; k++) close(nodeWeight(pipe, k, 0, { opening: 0 }), 0, 1e-12, `partial ${k + 1} at the closed end`);
  // And the mouth is an antinode, which is the boundary condition the other way up.
  for (let k = 0; k < 6; k++) close(nodeWeight(pipe, k, 1, { opening: 0 }), 1, 1e-9, `partial ${k + 1} at the mouth`);
});

test('an air column takes the strike position for the same reason a string does', () => {
  const tube = 'tube';
  close(nodeWeight(tube, 2, 1 / 3, { opening: 1 }), 0, 1e-9, 'third partial, driven at a third');
  assert.ok(nodeWeight(tube, 1, 1 / 3, { opening: 1 }) > 0.5, 'the second partial survives it');
});

// ---------------------------------------------------------------------------

test('Inharm stretches the series without moving the fundamental or crossing partials', () => {
  const base = ratiosOf('string', 12);
  for (const k of [-1, -0.4, 0.4, 1]) {
    const s = stretch(base, k);
    close(s[0], 1, 1e-12, 'the fundamental stays put');
    for (let i = 1; i < s.length; i++) assert.ok(s[i] > s[i - 1], `partial ${i + 1} crossed under Inharm ${k}`);
    if (k > 0) assert.ok(s[11] > base[11], 'positive Inharm should stretch');
    if (k < 0) assert.ok(s[11] < base[11], 'negative Inharm should compress');
  }
});

// ---------------------------------------------------------------------------

test('Material is the exponent it says it is', () => {
  // The engine's own range, and Applied Acoustics' published law:
  // T(f) = decay · (f/f₀)^−γ with γ = 1 − material.
  close(dampExponent(1), 0, 1e-12, 'Material at the top rings everything alike');
  close(dampExponent(0), 1, 1e-12, 'Material in the middle');
  close(dampExponent(-1), 2, 1e-12, 'Material at the bottom kills the highs fastest');
});

test('Decay is the fundamental’s ring time, in seconds, exactly', () => {
  // Not a percentage and not a relative figure: the number on the knob is the
  // number the fundamental rings for.
  close(ringSeconds(220, 220, 3.5, 0), 3.5, 1e-12, 'the fundamental');
  // At the top of Material every partial rings for as long as the fundamental.
  close(ringSeconds(7040, 220, 3.5, 1), 3.5, 1e-12, 'metal');
  // In the middle it falls as 1/f, so five octaves up is a thirty-second of it.
  close(ringSeconds(7040, 220, 3.2, 0), 3.2 / 32, 1e-12, 'wood');
  assert.ok(ringSeconds(7040, 220, 3.2, -1) < ringSeconds(7040, 220, 3.2, 0), 'softer still is shorter still');
});

test('what an object has is a fact about the object, not about the mode count', () => {
  const s = {
    object: 'string',
    f0: 220, modes: 16, select: 'Loudest', inharm: 0, bright: -2, material: 0, decay: 2,
    hit: 0.5, hitY: 0.5, posL: 0.3, posLY: 0.5, posR: 0.7, posRY: 0.5, spread: 0,
    ratio: 1, opening: 1, radius: 20, barSecond: 4, barThird: 9.2, nyquist: 24000, edits: [],
  };
  const all = computePartials(s);
  // A 220 Hz string has a hundred and nine partials under Nyquist whether the
  // bank is asked for sixteen resonators or four thousand. Letting `modes`
  // shorten this list is what made the ceiling name the wrong limit.
  assert.equal(all.length, 109, `109 partials under Nyquist, got ${all.length}`);
  assert.deepEqual(computePartials({ ...s, modes: 4096 }).length, all.length, 'and the count does not change it');
  assert.ok(all.every((q) => q.hz <= 24000), 'nothing above Nyquist');
  assert.ok(partialsAreOrdered(all), 'and in frequency order');
  // Raise the fundamental and the axis is what shortens it.
  assert.ok(computePartials({ ...s, f0: 4000 }).length < 10, 'the axis runs out');
  // The mode count is applied afterwards, and that is what the bank runs.
  assert.equal(selectPartials(all, 'Lowest', 16).length, 16, 'sixteen resonators');
});

test('an override moves the mode it names and leaves the rest alone', () => {
  const s = {
    object: 'string',
    f0: 220, modes: 12, select: 'Loudest', inharm: 0, bright: 0, material: 0, decay: 2,
    hit: 0.5, hitY: 0.5, posL: 0.5, posLY: 0.5, posR: 0.5, posRY: 0.5, spread: 0,
    ratio: 1, opening: 1, radius: 20, barSecond: 4, barThird: 9.2, nyquist: 24000, edits: [],
  };
  const plain = computePartials(s);
  // A string's third mode is `i = 3`, `j = 0` — the mode's own number, not a
  // row in a list.
  const edited = computePartials({ ...s, edits: [{ i: 3, j: 0, cents: 1200, db: -6, decay: 2 }] });
  const at = (list, mi) => list.find((q) => q.mi === mi);
  close(at(edited, 3).hz, at(plain, 3).hz * 2, 1e-9, 'mode 3, an octave up');
  close(at(edited, 3).dbL, at(plain, 3).dbL - 6, 1e-9, 'and six decibels down');
  // The damping law is read at the partial's own frequency, so a pitch
  // override moves its ring time too: an octave up is already half as long
  // before the multiplier is applied.
  close(at(edited, 3).ring, ringSeconds(at(edited, 3).hz, s.f0, s.decay, s.material) * 2, 1e-12, 'the law at its new frequency, doubled');
  for (const mi of [1, 2, 4, 5]) close(at(edited, mi).hz, at(plain, mi).hz, 1e-9, `mode ${mi} untouched`);
});

test('on a surface the override needs both indices, or it hits several modes', () => {
  // **The reason the key is a pair.** A rectangle's modes routinely share a
  // first index: (2,1) and (2,3) are different shapes at different
  // frequencies. Keyed on `i` alone, one edit would land on both, and the
  // display would look entirely reasonable while it happened.
  const s = {
    object: 'membrane',
    f0: 110, modes: 64, select: 'Loudest', inharm: 0, bright: 0, material: 0, decay: 2,
    hit: 0.5, hitY: 0.5, posL: 0.5, posLY: 0.5, posR: 0.5, posRY: 0.5, spread: 0,
    ratio: 1, opening: 1, radius: 20, barSecond: 4, barThird: 9.2, nyquist: 24000, edits: [],
  };
  const plain = computePartials(s);
  const sharing = plain.filter((q) => q.mi === 2);
  assert.ok(sharing.length > 1, `a rectangle should have several modes with i = 2, found ${sharing.length}`);

  const j = sharing[0].mj;
  const edited = computePartials({ ...s, edits: [{ i: 2, j, cents: 600 }] });
  const moved = edited.filter((q, k) => Math.abs(q.hz - plain[k].hz) > 1e-6);
  assert.equal(moved.length, 1, `exactly one mode should move, ${moved.length} did`);
  assert.equal(moved[0].mi, 2);
  assert.equal(moved[0].mj, j);
});

test('a mode carries its own identity, and only a surface has two indices', () => {
  const line = computePartials({
    object: 'string', f0: 220, modes: 8, select: 'Loudest', inharm: 0, bright: 0, material: 0,
    decay: 2, hit: 0.5, hitY: 0.5, posL: 0.5, posLY: 0.5, posR: 0.5, posRY: 0.5, spread: 0,
    ratio: 1, opening: 1, radius: 20, barSecond: 4, barThird: 9.2, nyquist: 24000, edits: [],
  });
  line.slice(0, 8).forEach((q, k) => {
    assert.equal(q.mi, k + 1, 'a one-dimensional mode is numbered from one');
    assert.equal(q.mj, 0, 'and has no second index');
  });
  const disc = computePartials({
    object: 'membrane_round', f0: 110, modes: 32, select: 'Loudest', inharm: 0, bright: 0,
    material: 0, decay: 2, hit: 0.5, hitY: 0.5, posL: 0.5, posLY: 0.5, posR: 0.5, posRY: 0.5,
    spread: 0, ratio: 1, opening: 1, radius: 20, barSecond: 4, barThird: 9.2, nyquist: 24000, edits: [],
  });
  // The fundamental of a drum head is circularly symmetric: no nodal
  // diameters, one nodal circle — the rim.
  assert.equal(disc[0].mi, 0);
  assert.equal(disc[0].mj, 1);
  assert.ok(disc.every((q) => q.mj >= 1), 'every mode of a disc has at least one nodal circle');
  // And the pair is unique, which is what makes it usable as a key.
  const keys = new Set(disc.map((q) => `${q.mi}:${q.mj}`));
  assert.equal(keys.size, disc.length, 'mode identities must be unique');
});

// ---------------------------------------------------------------------------

/** A hundred and twenty partials, enough that `select` has something to throw away. */
function bigSeries() {
  return Array.from({ length: 120 }, (_, i) => ({
    i,
    hz: 220 * (i + 1),
    // A shape with its loudest partials well up the series, so "loudest" and
    // "lowest" cannot accidentally agree.
    dbL: -Math.abs(i - 90) / 2,
    dbR: -Math.abs(i - 90) / 2,
  }));
}
const partialsAreOrdered = (l) => l.every((p, i) => i === 0 || p.hz >= l[i - 1].hz);

test('Lowest keeps the bottom of the series and throws away everything above', () => {
  const kept = selectPartials(bigSeries(), 'Lowest', 64);
  assert.equal(kept.length, 64);
  assert.deepEqual(kept.map((p) => p.i), Array.from({ length: 64 }, (_, i) => i));
  // This is the wall: with sixty-four resonators and Lowest, there is nothing
  // above partial 64 at all.
  assert.equal(kept[kept.length - 1].i, 63);
});

test('Loudest keeps the audible ones wherever they sit', () => {
  const kept = selectPartials(bigSeries(), 'Loudest', 64);
  assert.equal(kept.length, 64);
  assert.ok(partialsAreOrdered(kept), 'and hands them back in frequency order');
  // The loudest of that series are around partial 90, which Lowest never sees.
  assert.ok(kept.some((p) => p.i > 80), 'the loud upper partials survive');
  assert.ok(kept[kept.length - 1].i > 64, 'so the series reaches past where Lowest stops');
});

test('Log Spread keeps the shape of the whole range', () => {
  const kept = selectPartials(bigSeries(), 'Log Spread', 64);
  assert.equal(kept.length, 64);
  assert.ok(partialsAreOrdered(kept), 'in frequency order');
  assert.equal(kept[0].i, 0, 'from the bottom');
  assert.equal(kept[kept.length - 1].i, 119, 'to the top');
  assert.equal(new Set(kept.map((p) => p.i)).size, 64, 'each partial taken once');
});

test('every mode of select keeps the physical index the override addresses', () => {
  for (const mode of ['Loudest', 'Lowest', 'Log Spread']) {
    const kept = selectPartials(bigSeries(), mode, 64);
    for (const p of kept) close(p.hz, 220 * (p.i + 1), 1e-9, `${mode}: partial ${p.i} kept its own frequency`);
  }
});

test('a bank with room for everything chooses nothing', () => {
  const few = bigSeries().slice(0, 12);
  for (const mode of ['Loudest', 'Lowest', 'Log Spread']) {
    assert.deepEqual(selectPartials(few, mode, 64), few, mode);
  }
});

test('the display feed is the loudest, whatever Select is, and is never a wall', () => {
  // **A stream limit is not a wall.** The partials the picture leaves out are
  // still being synthesised, so the display's own cut is always the loudest —
  // taking the lowest sixty-four to draw would show a cliff that is not there.
  const audible = selectPartials(bigSeries(), 'Lowest', 100);
  const drawn = loudest(audible, PUBLISHED);
  assert.equal(drawn.length, PUBLISHED);
  assert.ok(partialsAreOrdered(drawn), 'in frequency order');
  // The loudest of the first hundred are up around partial 90, so the drawn
  // set reaches the top of what is audible rather than stopping at 64.
  assert.ok(drawn[drawn.length - 1].i > 80, 'the picture reaches the top of what is running');
  // And a short list is left alone.
  assert.deepEqual(loudest(bigSeries().slice(0, 10), PUBLISHED).length, 10);
});

// ---------------------------------------------------------------------------

test('the ceiling names the limit that produced it, and only a real one', () => {
  const s = {
    object: 'string',
    f0: 55, modes: 64, select: 'Lowest', inharm: 0, bright: 0, material: 0, decay: 2,
    hit: 0.5, hitY: 0.5, posL: 0.3, posLY: 0.5, posR: 0.7, posRY: 0.5, spread: 0,
    ratio: 1, opening: 1, radius: 20, barSecond: 4, barThird: 9.2, nyquist: 24000, edits: [],
  };
  const available = computePartials(s);
  const audible = selectPartials(available, 'Lowest', 64);
  const hz = ceilingHz(s, available, audible);
  // The finding this display exists to draw: at 55 Hz a sixty-four mode bank
  // set to Lowest runs out at 3.5 kHz and there is nothing at all above it.
  close(hz, 55 * 64, 1e-9, 'the wall is at the sixty-fourth partial');
  assert.ok(hz < 4000, `and it is inside the audio band: ${hz} Hz`);

  // Give the bank room for every partial and the wall goes, because there is
  // nothing above the top of the series to be missing.
  const roomy = selectPartials(available, 'Lowest', available.length);
  assert.equal(ceilingHz(s, available, roomy), 0, 'no wall when nothing was thrown away');

  // Raise the fundamental far enough and the same setting has no wall either,
  // because the sixty-fourth partial has cleared Nyquist.
  const high = { ...s, f0: 400 };
  const hAvail = computePartials(high);
  assert.equal(ceilingHz(high, hAvail, selectPartials(hAvail, 'Lowest', 64)), 0);
});

test('a waveguide never has a ceiling, because one loop gives every resonance', () => {
  const s = {
    object: 'tube',
    f0: 220, modes: 4, select: 'Lowest', inharm: 0, bright: 0, material: 0, decay: 2,
    hit: 0.5, hitY: 0.5, posL: 0.3, posLY: 0.5, posR: 0.7, posRY: 0.5, spread: 0,
    ratio: 1, opening: 1, radius: 20, barSecond: 4, barThird: 9.2, nyquist: 24000, edits: [],
  };
  const all = computePartials(s);
  assert.equal(ceilingHz(s, all, all), 0, 'a low mode count does nothing to an air column');
});

test('a two-dimensional object fuses far sooner than a bar does', () => {
  assert.ok(resolvable('membrane') < resolvable('string'), 'a drum head is dense');
  assert.ok(resolvable('membrane_round') <= 6, 'two to six is the published range');
  assert.ok(resolvable('beam') >= 12, 'twelve to thirty for the rest');
});
