/**
 * Reading a stream by the names it declares.
 *
 * **This is the mechanism every optional readout on the panel rests on**, and
 * until now it had no test. The page never reads a stream field by offset: it
 * asks for `db_bare` by name, and a build that does not publish it gets
 * nothing back and simply does not draw the node ghosts. Reading by offset
 * would print whichever field happened to sit at that index — a wrong number
 * structurally indistinguishable from a right one, which is the failure this
 * whole design exists to make impossible.
 *
 * The three cases that must all come back as "absent", because to a reader
 * they mean the same thing:
 *
 * * the build does not declare the field,
 * * no frame has arrived,
 * * the engine published a non-finite value for it.
 *
 * That last one is the rule the design manifest's `info` frame now follows:
 * an unset field must be **absent, not plausible**. It was zero-filled once,
 * and the level meter dutifully reported `0.0 dB GR` for a measurement
 * nothing had made.
 *
 *   npm test
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import { COUNT_MAX, countAt, fieldAt, isCount, parseLayout } from '../src/streams.js';
import { valueText } from '../src/format.js';

/** The engine's own two layouts, as `dsp::streams` declares them. */
const MODES = 'i,j,hz,db_l,db_r,t60_s';
const INFO =
  'modes_used,modes_available,crossover_hz,tail_db,limit_gr_db,inharm_b,column_m,loop_ms,open_hz,engine,build,f0_hz';

test('a layout string becomes names, a stride and an index', () => {
  const l = parseLayout(MODES);
  assert.deepEqual(l.names, ['i', 'j', 'hz', 'db_l', 'db_r', 't60_s']);
  assert.equal(l.stride, 6, 'the stride is what walks the frame');
  assert.equal(l.index.hz, 2);
  assert.equal(l.index.t60_s, 5);
  assert.equal(parseLayout(INFO).stride, 12);
});

test('whitespace and an empty layout are survivable', () => {
  assert.deepEqual(parseLayout(' a , b ,c ').names, ['a', 'b', 'c']);
  for (const bad of ['', null, undefined]) {
    const l = parseLayout(bad);
    assert.equal(l.stride, 0);
    assert.equal(fieldAt([1, 2, 3], l, 'hz'), null, 'and asking it for anything gives nothing');
  }
});

test('a field the build does not declare reads as absent', () => {
  const l = parseLayout(MODES);
  const frame = [3, 0, 440, -6, -6, 1.2];
  assert.equal(fieldAt(frame, l, 'hz'), 440);
  // The three the engine does not publish. Each darkens exactly one readout.
  for (const name of ['db_bare', 'base_hz', 'ceiling_hz']) {
    assert.equal(fieldAt(frame, l, name), null, `${name} must be absent, not guessed`);
  }
});

test('a non-finite value is "not computed", not a number', () => {
  const l = parseLayout(INFO);
  const frame = new Float32Array(12).fill(NaN);
  frame[l.index.modes_used] = 109;
  assert.equal(fieldAt(frame, l, 'modes_used'), 109);
  // The one that caught this: an uncomputed gain reduction must not read as
  // "the limiter is taking nothing off".
  assert.equal(fieldAt(frame, l, 'limit_gr_db'), null);
  assert.equal(fieldAt(frame, l, 'ceiling_hz'), null);
});

test('and a real zero survives, because zero is a measurement', () => {
  const l = parseLayout(INFO);
  const frame = new Float32Array(12).fill(NaN);
  frame[l.index.limit_gr_db] = 0;
  assert.equal(fieldAt(frame, l, 'limit_gr_db'), 0, 'the limiter really is taking nothing off');
  assert.notEqual(fieldAt(frame, l, 'limit_gr_db'), null);
});

test('no frame at all reads as absent rather than throwing', () => {
  const l = parseLayout(INFO);
  for (const f of [null, undefined]) assert.equal(fieldAt(f, l, 'modes_used'), null);
});

test('a per-partial frame is walked by stride, not by fixed offsets', () => {
  const l = parseLayout(MODES);
  // Two modes of a surface, sharing a first index — which is exactly why the
  // override key is the pair.
  const frame = [2, 1, 220, -3, -3, 2.0, 2, 3, 660, -9, -9, 0.7];
  const read = (k) => ({
    i: fieldAt(frame, l, 'i', k * l.stride),
    j: fieldAt(frame, l, 'j', k * l.stride),
    hz: fieldAt(frame, l, 'hz', k * l.stride),
  });
  assert.deepEqual(read(0), { i: 2, j: 1, hz: 220 });
  assert.deepEqual(read(1), { i: 2, j: 3, hz: 660 });
  assert.notEqual(`${read(0).i}:${read(0).j}`, `${read(1).i}:${read(1).j}`, 'the pair tells them apart');
});

test('the engine may move a field and the page still finds it', () => {
  // The point of names over offsets: this layout is the same fields in a
  // different order, and every read still lands on the right one.
  const shuffled = parseLayout('hz,t60_s,i,j,db_l,db_r');
  const frame = [440, 1.2, 3, 0, -6, -7];
  assert.equal(fieldAt(frame, shuffled, 'hz'), 440);
  assert.equal(fieldAt(frame, shuffled, 'i'), 3);
  assert.equal(fieldAt(frame, shuffled, 't60_s'), 1.2);
  assert.equal(fieldAt(frame, shuffled, 'db_r'), -7);
});

test('a longer layout does not break a shorter reader', () => {
  // If the engine adds the three fields that have been asked for, nothing on
  // the page needs to change to keep working — and the readouts that wanted
  // them light up on their own.
  const grown = parseLayout('i,j,hz,db_l,db_r,t60_s,db_bare,base_hz');
  const frame = [3, 0, 440, -6, -6, 1.2, -1.5, 430];
  assert.equal(fieldAt(frame, grown, 'hz'), 440, 'the old fields are still right');
  assert.equal(fieldAt(frame, grown, 'db_bare'), -1.5, 'and the new ones simply appear');
  assert.equal(fieldAt(frame, grown, 'base_hz'), 430);
});

// ---------------------------------------------------------------------------

test('a number that cannot be a count is not read as one', () => {
  // **The real frame this came from.** With the fundamental driven to 1.2 Hz a
  // membrane genuinely has of the order of a hundred million partials under
  // Nyquist, and `modes_available` arrived as 1.8446744e19 — `u64::MAX`, from
  // an unsigned cast of an infinity rather than from a subtraction running past
  // zero. The panel printed it faithfully as "this object has
  // 18446744073709552.0 k partials", which is the same class of failure as the
  // zero-filled frame: a number arriving in the right field, in the right
  // units, that nothing measured.
  const layout = parseLayout('modes_used,modes_available');
  assert.equal(countAt([12, 1.8446744073709552e19], layout, 'modes_available'), null, 'the impossible count');
  assert.equal(countAt([12, 1.8446744073709552e19], layout, 'modes_used'), 12, 'and the field beside it survives');

  assert.equal(isCount(0), true, 'zero is a count, and a real one');
  assert.equal(isCount(4096), true);
  assert.equal(isCount(COUNT_MAX), true, 'the last integer a float carries exactly');
  assert.equal(isCount(COUNT_MAX * 2), false, 'past which it is no longer spaced one apart');
  assert.equal(isCount(-1), false, 'nothing has a negative number of partials');
  assert.equal(isCount(12.5), false, 'nor half of one');
  assert.equal(isCount(NaN), false);
  assert.equal(isCount(Infinity), false);
  assert.equal(isCount(null), false);
});

test('and refusing it is not the same as hiding it', () => {
  // The reader hands back nothing; the caller is expected to notice that a
  // value arrived and was refused, so the panel can say so rather than showing
  // a dash that reads as "this build does not publish it".
  const layout = parseLayout('modes_used,modes_available');
  const frame = [12, -1];
  assert.equal(countAt(frame, layout, 'modes_available'), null, 'refused');
  assert.equal(fieldAt(frame, layout, 'modes_available'), -1, 'but still there to be complained about');
});

// ---------------------------------------------------------------------------

test('a parameter that says it is whole prints whole, at every value', () => {
  // **The values that could not fail are the ones this was checked at.** The
  // rounding was removed when the manifest gained a `decimals` hint, verified
  // live at the mode budget's default of 1024 — where the client's own fallback
  // prints a clean integer whatever the hint says, because it is a magnitude
  // rule and 1024 is over a thousand. Below a hundred that rule prints one
  // decimal and below ten it prints two, so the knob read 32.0 and 4.00: the
  // exact false precision the hint exists to remove, in the exact range a mode
  // budget is actually used.
  const count = { spec: { decimals: 0, min: 4, max: 4096 }, plain: 4, text: '4.00' };
  assert.equal(valueText(count), '4');
  assert.equal(valueText({ ...count, plain: 32, text: '32.0' }), '32');
  assert.equal(valueText({ ...count, plain: 1024, text: '1024' }), '1024');

  const semis = { spec: { decimals: 0, unit: 'st', min: -24, max: 36 }, plain: 0, text: '0.00 st' };
  assert.equal(valueText(semis), '0 st', 'the unit comes with it');
  assert.equal(valueText({ ...semis, plain: -12, text: '-12.0 st' }), '-12 st');
});

test('and a parameter that says nothing keeps the client’s own formatting', () => {
  // The page is filling in for a client that does not implement the hint yet.
  // Where there is no hint there is nothing to fill in, and where the client's
  // rendering is not about decimals at all — an enumeration, a toggle — it is
  // left alone entirely.
  assert.equal(valueText({ spec: { unit: 'Hz' }, plain: 440, text: '440 Hz' }), '440 Hz');
  assert.equal(valueText({ spec: { decimals: 0, labels: ['Loudest', 'Lowest'], steps: 2 }, plain: 0, text: 'Loudest' }), 'Loudest');
  assert.equal(valueText({ spec: { decimals: 0, steps: 2 }, plain: 1, text: 'On' }), 'On');
  assert.equal(valueText({ spec: { decimals: 0 }, plain: NaN, text: '—' }), '—', 'a value that is not a number is the client’s problem, not this one');
  assert.equal(valueText(null), '—');
});
