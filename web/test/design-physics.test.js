/**
 * The design-mode physics, and the engine's own series table, held to
 * published values.
 *
 * **Half of this now tests the engine rather than the page, and that is the
 * point.** The series come off `benchmark --dump series` into
 * `src/dev/series-table.js`, so the ratios asserted here are the numbers the
 * audio thread runs — and they are checked against their *defining equations*
 * and against the literature, never against the code that produced them. A
 * test that asserts a model reproduces its own output is the bug rather than
 * the evidence, so the beam's table is fed back through `cos β · cosh β = 1`
 * and the drum head's through the Bessel integral, both of which are
 * independent of anything the engine did.
 *
 * The rest guards `src/dev/physics/` — the mode shapes and the two one-line
 * laws the page still applies over that table, so that a page opened with no
 * plug-in running is not quietly wrong.
 *
 * **Two results in here came out of writing these tests** rather than out of
 * reading a source:
 *
 * * The beam ratio quoted everywhere as 2.756 is a **truncation** of 2.75654,
 *   not a rounding of it — correctly rounded it is 2.757.
 * * An undercut bar has **no closed form**. Its ratios are a maker's tuning
 *   target, and the two published values for its third partial are a
 *   builder's choice rather than a discrepancy to average away.
 *
 * It deliberately asserts nothing about amplitudes: those are invented until
 * the engine publishes them.
 *
 *   npm test
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import {
  barRatios,
  beamShape,
  besselJ,
  inharmB,
  modeIndices,
  ratiosOf,
  columnLength,
  guideRatios,
  nodeWeight,
  rectModes,
  stretch,
  tableOf,
  tineShape,
  BAR_SECOND,
  BAR_THIRD,
  C_AIR,
  INHARM_B_MAX,
  SHAPELESS,
} from '../src/dev/physics/resonators.js';
import { fieldAt } from '../src/streams.js';
import { OBJECTS } from '../src/objects.js';
import {
  allPartialsCounted,
  ceilingHz,
  computePartials,
  dampExponent,
  resolvable,
  ringSeconds,
  selectPartials,
  loudest,
  PUBLISHED,
} from '../src/dev/physics/model.js';

/** The ratios of one object's series, off the engine's table. */
const series = (id) => tableOf(id).map((m) => m[2]);

/**
 * `βₙL` recovered from a bar's ratios: frequencies go as `(βₙ/β₁)²`, so the
 * eigenvalue is `β₁√rₙ` and the published first root is all that is needed.
 * This is what lets the engine's table be fed back through the equation that
 * defines it.
 */
/**
 * How far a residual may sit from zero before it means something.
 *
 * The engine's ratios reach this page through single precision, so a root
 * recovered from one is good to about seven figures and the residual it leaves
 * is around 1e-7 however exactly the engine solved. That is a property of the
 * wire rather than of the mathematics — and it is still four orders tighter
 * than a wrong root, which leaves a residual of order one.
 */
const WIRE = 1e-5;
const BEAM_BETA1 = 4.730040744862704;
const TINE_BETA1 = 1.8751040687119413;
const J01 = 2.404825557695773;

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

test('the engine’s beam ratios solve cos β · cosh β = 1', () => {
  // **This tests the engine, not the page.** The ratios come off
  // `benchmark --dump series`; turning each back into its eigenvalue and
  // substituting it into the equation that defines it is a check that owes
  // nothing to the code that produced them. Checked in the form that does not
  // overflow: cos β = sech β.
  for (const r of series('beam').slice(0, 12)) {
    const b = BEAM_BETA1 * Math.sqrt(r);
    close(Math.cos(b), 1 / Math.cosh(b), WIRE, `residual at β = ${b}`);
  }
});

test('and they are the ratios the literature gives', () => {
  // Leissa, NASA SP-160, Table 4.23, as ratios of the fundamental.
  const r = series('beam');
  [1, 2.756538507, 5.403917632, 8.932950352].forEach((w, i) => close(r[i], w, 1e-5, `beam partial ${i + 1}`));
});

test('the engine’s tine ratios solve cos β · cosh β = −1, and are the cantilever’s', () => {
  for (const r of series('tine').slice(0, 8)) {
    const b = TINE_BETA1 * Math.sqrt(r);
    // −sech β, the same rearrangement with the sign the clamped end puts on it.
    close(Math.cos(b), -1 / Math.cosh(b), WIRE, `residual at β = ${b}`);
  }
  // Leissa, Table 4.39. A cantilever's first overtone is at 6.27 where a free
  // bar's is at 2.76, which is the whole reason a tine is not a glockenspiel.
  const r = series('tine');
  [1, 6.2669, 17.5475, 34.3861].forEach((w, i) => close(r[i], w, 1e-3, `tine partial ${i + 1}`));
});

test('the engine’s clamped disc is the published one', () => {
  // Leissa §2.1 / Rossing: a disc clamped at its rim rings at
  // 1 : 2.08 : 3.41 : 3.89 : 5.00 — far wider than the round head's
  // 1 : 1.59 : 2.14, because a stiff plate goes as λ² where a tensioned
  // membrane goes as λ.
  const r = series('plate_round');
  [1, 2.08, 3.41, 3.89, 5.0].forEach((w, i) => close(r[i], w, 5e-3, `plate_round partial ${i + 1}`));
  const head = series('membrane_round');
  assert.ok(r[1] > head[1] + 0.4, 'a stiff disc spreads wider than a tensioned one');
});

test('the second partial is 2.7565, and the usual 2.756 is a truncation of it', () => {
  // Worth pinning, because "1 : 2.756 : 5.404 : 8.933" is quoted everywhere
  // and only two of those three are correctly rounded. The exact value is
  // 2.75654, which rounds to 2.757. The panel prints what the engine solved
  // rather than the quotation, which is why this test exists.
  const r = series('beam');
  close(Number(r[1].toFixed(4)), 2.7565, 1e-9, 'four places');
  assert.equal(Number(r[1].toFixed(3)), 2.757, 'three places, correctly rounded');
  assert.notEqual(Number(r[1].toFixed(3)), 2.756, 'the quoted figure is not the rounded one');
});

test('a free–free beam mode has one more node than its index', () => {
  series('beam').slice(0, 6).forEach((r, i) => {
    const x = BEAM_BETA1 * Math.sqrt(r);
    const n = zeros((u) => beamShape(x, u)).length;
    assert.equal(n, i + 2, `mode ${i + 1} should have ${i + 2} nodes, found ${n}`);
  });
});

test('the first mode’s nodes are where a marimba bar’s cord goes', () => {
  // 0.2242 and 0.7758 of the length: the two points the fundamental does not
  // move, which is why a bar hung there keeps ringing.
  const n = zeros((u) => beamShape(BEAM_BETA1, u));
  close(n[0], 0.2242, 5e-4, 'lower node');
  close(n[1], 0.7758, 5e-4, 'upper node');
});

test('a clamped bar is held at one end and free at the other, exactly', () => {
  // Both clamped conditions fall out of the rearranged form rather than being
  // imposed, so getting them for free is the check that the rearrangement did
  // not change the function.
  for (const r of series('tine').slice(0, 6)) {
    const x = TINE_BETA1 * Math.sqrt(r);
    close(tineShape(x, 0), 0, 1e-9, `clamped end, β = ${x}`);
    // Zero *and* flat there, which is a double zero: the shape leaves the
    // clamp as u² rather than as u, so a hundredth of the way along it is
    // still four orders below the free end. Asserted as the value rather than
    // as a finite-difference slope, which only measures the step size.
    assert.ok(
      Math.abs(tineShape(x, 1e-4)) < 1e-5,
      `the clamped end should be a double zero, β = ${x}, got ${tineShape(x, 1e-4)}`,
    );
    assert.ok(Math.abs(tineShape(x, 1)) > 1, 'and the far end is free to move');
  }
  // Which is what the contact control does with it: strike the clamp and
  // nothing comes out.
  close(nodeWeight('tine', 0, 0), 0, 1e-9, 'a tine struck at its clamp');
  assert.ok(nodeWeight('tine', 0, 1) > 0.9, 'and struck at its tip');
});

test('the beam mode shape survives the high modes it is asked for', () => {
  // The naive form is the difference of two numbers around 1e19 by mode 20.
  const e = series('beam').map((r) => BEAM_BETA1 * Math.sqrt(r));
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
  assert.ok(series('beam')[1] < r[1], 'the undercut raises partial 2 relative to the fundamental');
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

test('every ratio the engine gives for a drum head really is a Bessel zero', () => {
  // **The engine again, checked against the definition.** A round head's
  // ratios are `j_{mn}/j₀₁`, so multiplying each by `j₀₁` must land on a zero
  // of the Bessel function whose order the table names — evaluated here by
  // Simpson on the integral form, which shares no code with whatever the
  // engine did.
  for (const [m, , r] of tableOf('membrane_round').slice(0, 24)) {
    close(besselJ(m, J01 * r), 0, WIRE, `J${m} at ratio ${r}`);
  }
});

test('a drum head’s series is the published one, and it is not the rectangle’s', () => {
  const got = series('membrane_round');
  [1, 1.5933, 2.1355, 2.2954, 2.6531, 2.9173].forEach((w, i) => close(got[i], w, 5e-5, `partial ${i + 1}`));
  // The reason the round head exists as its own object: a circle is not a
  // rectangle, and no aspect ratio turns one into the other.
  const rect = rectModes(6, 1).map((m) => m.ratio);
  assert.ok(Math.abs(got[1] - rect[1]) > 0.01, 'a round head and a square one differ from the second partial on');
});

test('the page’s rectangle and the engine’s are the same rectangle', () => {
  // The one series still solved on the page, because Ratio is a control and
  // the engine's table is one aspect. At aspect 1 the two must agree exactly,
  // and this is what would catch them drifting apart.
  //
  // **This is the test that caught the dump.** It compared the whole membrane
  // table and failed at partial seventeen, because `--dump series` took its
  // rows in index order rather than in frequency order: the walk spends five
  // hundred rows on i = 1 before it reaches i = 2, so the cap kept three
  // families and dropped every mode with i >= 5, both halves of two
  // degenerate pairs among them. The engine sorts before it takes now, and
  // this compares all of it again.
  const mem = series('membrane');
  rectModes(mem.length, 1).forEach((m, i) => close(m.ratio, mem[i], 2e-5, `membrane partial ${i + 1}`));
  const plate = series('plate');
  rectModes(plate.length, 1).forEach((m, i) => close(m.ratio ** 2, plate[i], 2e-3, `plate partial ${i + 1}`));
});

test('the page’s air column is the engine’s delay loop, to within the loop’s own dispersion', () => {
  // The page keeps the closed form because Opening has to sweep and the dump
  // is one setting each. This is what holds the two together — and what
  // records the gap, which is real: a delay loop with a filtered reflection is
  // dispersive and an ideal pipe is not, so the engine's upper partials sit
  // progressively sharp. A fraction of a cent at the bottom, a few cents by
  // the fiftieth.
  const cents = (a, b) => Math.abs(1200 * Math.log2(a / b));
  for (const [id, opening] of [['pipe', 0], ['tube', 1]]) {
    const engine = series(id);
    const page = guideRatios(engine.length, opening);
    engine.slice(0, 8).forEach((r, i) => {
      assert.ok(cents(r, page[i]) < 1, `${id} partial ${i + 1}: ${cents(r, page[i]).toFixed(3)} cents apart`);
    });
    const top = engine.length - 1;
    assert.ok(cents(engine[top], page[top]) < 10, `${id} stays within ten cents to the top of the dump`);
  }
});

test('the mode indices come off the engine with the ratios', () => {
  // An override addresses a mode by the name the audio thread calls it, so a
  // page that numbered its own would edit the wrong partial and look right.
  for (const id of ['beam', 'string', 'tine', 'membrane_round', 'plate_round']) {
    const ix = modeIndices(id, 12);
    const rows = tableOf(id).slice(0, 12);
    ix.forEach((pair, k) => assert.deepEqual(pair, [rows[k][0], rows[k][1]], `${id} mode ${k}`));
  }
});

test('the clamped disc has no mode shape here, and says so rather than pretending', () => {
  // Its shape needs a modified Bessel function, which is the machinery this
  // page stopped carrying. A dead control that looks alive is worse than a
  // dead control that is labelled, so design mode weights every mode alike
  // and SHAPELESS is what the panel prints from.
  assert.ok(SHAPELESS.has('plate_round'));
  for (const u of [0, 0.25, 0.5, 1]) close(nodeWeight('plate_round', 3, u), 1, 1e-12, `uniform at ${u}`);
  // And no other object is quietly in that set.
  for (const o of OBJECTS) {
    if (o.id === 'plate_round') continue;
    assert.ok(!SHAPELESS.has(o.id), `${o.id} should have a modelled shape`);
  }
});

test('every mode of a drum head is a node at the rim, and only the round ones live at the centre', () => {
  const c = 'membrane_round';
  for (let k = 0; k < 8; k++) close(nodeWeight(c, k, 1), 0, 1e-6, `partial ${k + 1} at the rim`);
  // The fundamental is circularly symmetric, so the centre is its antinode.
  close(nodeWeight(c, 0, 0), 1, 1e-9, 'the fundamental at the centre');
  // The second and third have a nodal diameter through the middle, which is
  // why striking a drum dead centre gives a duller, more pitched sound.
  close(nodeWeight(c, 1, 0), 0, 1e-6, 'the second at the centre');
  close(nodeWeight(c, 2, 0), 0, 1e-6, 'the third at the centre');
});

test('the object list is the frozen index order and is append-only', () => {
  // A saved project's object is its index, so nothing here may ever move.
  // The first seven are Corpus's own order; everything from the eighth is
  // ours, and each was appended.
  assert.deepEqual(
    OBJECTS.map((t) => t.id),
    ['beam', 'marimba', 'string', 'membrane', 'plate', 'pipe', 'tube', 'membrane_round', 'tine', 'plate_round'],
  );
  assert.deepEqual(
    OBJECTS.map((t) => t.engine),
    ['modal', 'modal', 'modal', 'modal', 'modal', 'waveguide', 'waveguide', 'modal', 'modal', 'modal'],
  );
});

test('every object in the catalogue has a series, and every series an object', () => {
  // The check that went missing: the engine appended two objects and this
  // catalogue had eight, so `objectAt` clamped and the face would have printed
  // "Membrane Round" over a different object's partials.
  for (const o of OBJECTS) {
    assert.ok(tableOf(o.id).length > 0, `${o.id} has no series`);
    assert.ok(o.blurb && o.source && o.uses, `${o.id} is missing its prose`);
  }
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

test('Inharm is the stiff-string law, the engine’s, and it does not cross partials', () => {
  const base = ratiosOf('string', 12);
  for (const k of [-1, -0.4, 0.4, 1]) {
    const s = stretch(base, k);
    for (let i = 1; i < s.length; i++) assert.ok(s[i] > s[i - 1], `partial ${i + 1} crossed under Inharm ${k}`);
    if (k > 0) assert.ok(s[11] > base[11], 'positive Inharm should stretch');
    if (k < 0) assert.ok(s[11] < base[11], 'negative Inharm should compress');
  }
});

test('and the fundamental moves with it, which is what the law says', () => {
  // `fₙ = n·f₁·√(1 + Bn²)` scales the first partial too, by `√(1 + B)`. This
  // page used to apply a log-axis stretch that pinned the fundamental instead
  // and said plainly that it was not the stiff-string law. It was honest and
  // it was still the wrong shape, because the engine's law is the real one.
  const top = stretch([1], 1)[0];
  close(top, Math.sqrt(1 + INHARM_B_MAX), 1e-12, 'the fundamental at full stretch');
  const cents = 1200 * Math.log2(top);
  assert.ok(cents > 2 && cents < 3, `about two and a half cents at the extreme, got ${cents}`);
});

test('the Inharm control is quadratic and signed, as the engine has it', () => {
  // So the region a real string lives in — B around 3e-4 for a piano C4 — is
  // where a knob can be put on it rather than in the first pixel.
  close(inharmB(0), 0, 1e-15, 'the middle');
  close(inharmB(1), INHARM_B_MAX, 1e-15, 'the top');
  close(inharmB(-1), -INHARM_B_MAX, 1e-15, 'the bottom');
  close(inharmB(0.5), INHARM_B_MAX * 0.25, 1e-15, 'quadratic in the control');
  close(inharmB(2), INHARM_B_MAX, 1e-15, 'clamped past the end');
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
  //
  // **Not zero: unset.** A zero would be a frequency, and the panel would have
  // to guess that this particular frequency means "no wall" — which is the
  // zero-filled-frame fault in miniature. The engine publishes NaN for every
  // field that does not apply, so this does too, and the display says *no
  // wall* in ordinary ink instead of reporting a missing feed.
  const roomy = selectPartials(available, 'Lowest', available.length);
  assert.ok(Number.isNaN(ceilingHz(s, available, roomy)), 'no wall when nothing was thrown away');

  // Raise the fundamental far enough and the same setting has no wall either,
  // because the sixty-fourth partial has cleared Nyquist.
  const high = { ...s, f0: 400 };
  const hAvail = computePartials(high);
  assert.ok(Number.isNaN(ceilingHz(high, hAvail, selectPartials(hAvail, 'Lowest', 64))));
});

test('a waveguide never has a ceiling, because one loop gives every resonance', () => {
  const s = {
    object: 'tube',
    f0: 220, modes: 4, select: 'Lowest', inharm: 0, bright: 0, material: 0, decay: 2,
    hit: 0.5, hitY: 0.5, posL: 0.3, posLY: 0.5, posR: 0.7, posRY: 0.5, spread: 0,
    ratio: 1, opening: 1, radius: 20, barSecond: 4, barThird: 9.2, nyquist: 24000, edits: [],
  };
  const all = computePartials(s);
  assert.ok(Number.isNaN(ceilingHz(s, all, all)), 'a low mode count does nothing to an air column');
});

test('an unset info field stays unset, and a real zero still gets through', () => {
  // The stand-in's `put` drops a non-finite value rather than writing it, so
  // the NaN-filled frame keeps its NaN and the page reads *not applicable*.
  // The half that matters is the other one: a genuine zero is a measurement
  // and has to survive, which is what stopped this being a one-line rule.
  const frame = new Float32Array(3).fill(NaN);
  const layout = { index: { a: 0, b: 1, c: 2 } };
  const put = (i, v) => {
    if (Number.isFinite(v)) frame[i] = v;
  };
  put(0, NaN);
  put(1, 0);
  put(2, 12.5);
  assert.equal(fieldAt(frame, layout, 'a'), null, 'unset');
  assert.equal(fieldAt(frame, layout, 'b'), 0, 'a real zero survives, because zero is a measurement');
  assert.equal(fieldAt(frame, layout, 'c'), 12.5);
});

test('a two-dimensional object fuses far sooner than a bar does', () => {
  assert.ok(resolvable('membrane') < resolvable('string'), 'a drum head is dense');
  assert.ok(resolvable('membrane_round') <= 6, 'two to six is the published range');
  assert.ok(resolvable('beam') >= 12, 'twelve to thirty for the rest');
});
