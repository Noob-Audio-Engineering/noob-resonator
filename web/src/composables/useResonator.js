/**
 * Noob Resonator specifics on top of the generic
 * `@noob-audio-engineering/noob-vst-webgui-framework/vue` bridge.
 *
 * **The panel renders; it does not compute.** Every partial, every level and
 * every ring time on this page arrives on a stream. There is no physics in
 * this file and there must not be: the mathematics belongs to the Rust
 * engine, which owns it, tests it and ships it. What the page adds is which
 * of those numbers to show, how to draw them, and what to say about them.
 *
 * The equations that once lived here are in `dev/physics/`, loaded only in
 * development, and their only job is to fill these same streams so the page
 * renders before the plug-in is running. Nothing outside `src/dev/` imports
 * them, so a production build has no copy of them at all — and the reader
 * never has to wonder which path a number came from, because there is only
 * one path.
 *
 * **Every stream field is looked up by name, never by offset.** Each stream
 * declares a `layout` in its meta; a build that publishes fewer fields loses
 * exactly the readouts that needed them, and each of those says so, rather
 * than the page printing whatever happened to sit at that index.
 */
import { computed, reactive, ref, watch } from 'vue';
import {
  getClient,
  hasParam,
  hasStream,
  useNoobVstWebguiFramework,
  useParam,
  useStoredRef,
  useStreamFrame,
  useWindowSize,
} from '@noob-audio-engineering/noob-vst-webgui-framework/vue';
import { objectAt } from '../objects.js';
import { countAt, declares, fieldAt, isCount, parseLayout } from '../streams.js';

export { getClient, hasParam, hasStream, useParam, useNoobVstWebguiFramework, useStoredRef };
export { countAt, declares, fieldAt, isCount, parseLayout } from '../streams.js';

/** Smallest window the panel lays out in, `[width, height]` CSS pixels; the Rust side will clamp to the same. */
export const WINDOW_MIN = [900, 520];

let win = null;
/** The page's one `useWindowSize` instance, created from the root component so its listeners live as long as the page. */
export function useWindow() {
  win ??= useWindowSize({ min: WINDOW_MIN });
  return win;
}

let res = null;
/**
 * Every parameter handle the panel uses, resolved once. A handle for an id
 * the build does not publish is `null` and the control that owns it is not
 * drawn.
 */
export function useRes() {
  if (res) return res;
  const p = (id) => (hasParam(id) ? useParam(id) : null);
  res = {
    type: p('type'),
    select: p('select'),
    /**
     * How many voices sound, and where each one sits.
     *
     * **A voice is a root, not a partial.** Each one gets the object's own
     * series, so six voices on a Beam is six beams rather than something that
     * is no longer a beam — which is the whole reason chord tuning is
     * orthogonal to the object rather than being an object of its own.
     */
    voices: p('voices'),
    modes: p('mode_budget'),
    tune: p('tune'),
    transpose: p('transpose'),
    fine: p('fine'),
    ratio: p('ratio'),
    radius: p('radius'),
    opening: p('opening'),
    barTuning: p('bar_tuning'),
    barThird: p('bar_third'),
    decay: p('decay'),
    material: p('material'),
    bright: p('bright'),
    inharm: p('inharm'),
    hit: p('hit'),
    hitY: p('hit_y'),
    posL: p('pos_l'),
    posLY: p('pos_l_y'),
    posR: p('pos_r'),
    posRY: p('pos_r_y'),
    spread: p('spread'),
    width: p('width'),
    dampCorner: p('damp_corner'),
    dampHi: p('damp_hi'),
    tail: p('tail'),
    filterOn: p('filter_on'),
    filterFreq: p('filter_freq'),
    filterWidth: p('filter_width'),
    filterPlace: p('filter_place'),
    lfoOn: p('lfo_on'),
    lfoShape: p('lfo_shape'),
    lfoRate: p('lfo_rate'),
    lfoDepth: p('lfo_depth'),
    lfoPhase: p('lfo_phase'),
    bleed: p('bleed'),
    mix: p('mix'),
    gain: p('gain'),
    limiter: p('limiter'),
    limitCeil: p('limit_ceil'),
    bypass: p('bypass'),
    /**
     * The standalone's demo source, absent under a plug-in.
     *
     * A resonator supplies a body and the incoming audio supplies the strike,
     * so with no host feeding it there is nothing to excite and the panel
     * sits silent. These three are how a developer hits it. They live on the
     * bench because they are not part of the device — the same place the
     * sibling lab keeps its own.
     */
    srcKind: p('src_kind'),
    srcLevel: p('src_level'),
    srcFreq: p('src_freq'),
  };
  return res;
}

/** The object currently loaded, from the catalogue. */
export function useObject() {
  const r = useRes();
  return computed(() => objectAt(r.type ? r.type.index : 2));
}

/** True while the page is running on the design manifest, and every number on it is the page's own arithmetic. */
export function useDesignMode() {
  return computed(() => !!getClient()?.offline);
}

// ---------------------------------------------------------------------------
// Reading a stream by the names it declares
// ---------------------------------------------------------------------------

/**
 * A stream's field names, from the `layout` its meta declares.
 *
 * Reading by name is what makes every optional field on this page degrade on
 * its own: the display asks for `db_bare`, a build that does not publish it
 * returns nothing, and the ghosts are simply not drawn. Reading by offset
 * would have printed the field that happened to be there instead, which is
 * the failure this exists to make impossible.
 *
 * Split into a pure half and a client-bound half so the pure half can be
 * tested — this mechanism is what every optional readout on the panel rests
 * on, and it had no test at all.
 */
function layoutOf(id) {
  if (!hasStream(id)) return { names: [], stride: 0, index: {} };
  return parseLayout((getClient().stream(id).meta || {}).layout);
}

// ---------------------------------------------------------------------------
// Which controls an object has// ---------------------------------------------------------------------------
// Which controls an object has
// ---------------------------------------------------------------------------

/**
 * The `objects` table the engine publishes in its manifest meta, keyed by id.
 *
 * **The page reads this and derives nothing.** Which control an object has is
 * an engine fact — it is what the engine will actually read — and deriving it
 * on this side was got wrong twice from the outside. `null` when a build
 * publishes no table, in which case nothing is greyed and the bench says so,
 * rather than the page greying a control the engine is listening to.
 */
export function useObjectTable() {
  const { manifest } = useNoobVstWebguiFramework();
  return computed(() => {
    const list = manifest.value?.meta?.objects;
    return Array.isArray(list) && list.length ? list : null;
  });
}

/**
 * The published entry for the object that is loaded, or `null`.
 *
 * **Looked up by index, because `id` on the wire is the index.** A saved
 * project loads an object by its position, so that is what identifies one —
 * and keying this by the catalogue's string id instead silently matched
 * nothing, which would have greyed no control at all the moment a real
 * plug-in connected while looking perfectly fine against the design manifest.
 */
export function useObjectMeta() {
  const r = useRes();
  const table = useObjectTable();
  return computed(() => {
    const list = table.value;
    if (!list) return null;
    const i = r.type ? r.type.index : 2;
    return list.find((o) => o.id === i) || list[i] || null;
  });
}

/** What choosing this object pins, as the engine publishes it. A Tube's far end is open by definition. */
export const forcesOf = (meta) => (meta && meta.forces) || null;
/** The engine's own note about this object, where it has one. */
export const noteOf = (meta) => (meta && meta.note) || '';

/**
 * Why an object has nothing for a control to act on.
 *
 * **The engine says *whether*; this says *why*.** The `uses` list is the truth
 * and it is a bare list of ids, which is the right thing to publish and the
 * wrong thing to show a person — a greyed control with no explanation is the
 * thing this panel exists to improve on. So the sentence lives here and is
 * only ever shown for a control the published table has already omitted.
 */
const WHY = {
  ratio: {
    short: 'no aspect to set',
    why: 'Ratio is the shape of a rectangle. A bar, a string and an air column have one dimension, and a round head is a circle — none of them has a second side to be in proportion to.',
  },
  radius: { short: 'air columns only', why: 'A bore is a property of a tube. A solid object has no inside for the air to be in.' },
  opening: {
    short: 'nothing left to open',
    why: 'Opening is the reflection at the far end of an air column, from closed to open. A solid has no far end, and a tube is already open at both — it is at one extreme of this control by definition.',
  },
  select: {
    short: 'nothing to choose between',
    why: 'Selection decides which of an object’s partials a limited bank of resonators runs. A waveguide has no such budget — every resonance under Nyquist falls out of one delay loop at one cost — so there is nothing being left out and nothing to choose.',
  },
  material: {
    short: 'the loop sets its own loss',
    why: 'Material is the exponent of a mode bank’s damping law, applied per resonator. An air column loses its highs to the walls and out of the open end instead, which the loop’s own reflection filter accounts for; Radius is the control that moves it.',
  },
  damp_corner: {
    short: 'mode-bank damping only',
    why: 'The second half of the mode bank’s two-parameter loss law: where the extra damping starts. An air column’s loss is a property of its loop, not of a per-mode law.',
  },
  damp_hi: {
    short: 'mode-bank damping only',
    why: 'How steeply the extra damping bites above its corner. Part of the same per-mode law an air column does not have.',
  },
  tail: {
    short: 'mode-bank only',
    why: 'The tail is what a bank of resonators is left ringing with. A delay loop rings on its own terms.',
  },
  mode_budget: {
    short: 'one loop gives them all',
    why: 'A waveguide’s partials are the resonances of a single delay loop, so every one under Nyquist comes out of it at the same cost whether or not anybody counts them. There is no budget here to spend.',
  },
  inharm: {
    short: 'Opening moves these',
    why: 'The loop fixes where an air column’s partials are. What moves them is the reflection at the far end, which is what Opening is.',
  },
  hit_y: {
    short: 'one dimension only',
    why: 'A strike on a bar, a string or an air column has one coordinate: how far along it you hit. The second axis needs a surface.',
  },
  pos_l_y: { short: 'one dimension only', why: 'A pickup on a one-dimensional object has one coordinate.' },
  pos_r_y: { short: 'one dimension only', why: 'A pickup on a one-dimensional object has one coordinate.' },
  bar_tuning: { short: 'a tuned bar only', why: 'The undercut that puts the second partial on a whole ratio is something a bar maker does to a bar.' },
  bar_third: { short: 'a tuned bar only', why: 'The depth of the arch decides where the third partial lands, and only a tuned bar has one.' },
};

/**
 * How this object's contact points are addressed: `line`, `xy` or `polar`.
 *
 * The engine publishes it per object. Where a build does not, the shape of
 * the object is the fallback — which is a labelling choice rather than a
 * physical claim, so it is safe to make here.
 */
export function coordsOf(object, meta) {
  return meta?.coords || (object.twoD ? 'xy' : 'line');
}

/**
 * What to call the two contact axes, and what they mean.
 *
 * **A disc is not a square, and saying so is not decoration.** Mapping a
 * round head onto X and Y wasted the corners and clamped them to the rim,
 * where every mode of a clamped membrane is exactly zero — a strike in the
 * corner of the control excited nothing at all. On a disc the first axis is
 * the distance out from the centre and the second is the angle round it, and
 * the angle chooses which orientation of a degenerate pair the strike lines
 * up with, which you can hear.
 */
export function contactAxes(coords) {
  if (coords === 'polar') {
    // "Rad" and "Ang" rather than "R" and "θ": the right pickup is already
    // called Pos R, so a radius suffixed R gave "Pos R R", which is not a
    // label anybody can read.
    return {
      x: { suffix: 'Rad', hint: 'out from the centre' },
      y: { suffix: 'Ang', hint: 'round the head' },
    };
  }
  if (coords === 'xy') {
    return { x: { suffix: 'X', hint: 'across' }, y: { suffix: 'Y', hint: 'down' } };
  }
  return { x: { suffix: '', hint: 'along it' }, y: { suffix: 'Y', hint: 'needs a surface' } };
}

/**
 * Controls the published object table names that this build does not have.
 *
 * **A list that names a control the build does not publish is stale, and a
 * stale list is not evidence about any control.** This exists because it
 * happened: `object_meta()` kept naming `modes` after the parameter was
 * renamed to `mode_budget`, so every bank object's `uses` array named a
 * control that no longer existed and did not name the one that did — which
 * would have greyed out the headline knob of the whole device, in the host
 * only, while looking perfectly correct against a design manifest that had
 * been updated. Nothing throws, nothing logs, and both sides are internally
 * consistent.
 *
 * So the page compares the two lists it has and says so out loud.
 */
const driftCache = new WeakMap();
function driftOf(meta) {
  if (!meta || !Array.isArray(meta.uses)) return [];
  let d = driftCache.get(meta);
  if (d === undefined) {
    d = meta.uses.filter((id) => !hasParam(id));
    driftCache.set(meta, d);
  }
  return d;
}

/** Every id the object table names that this build does not publish, across all objects. */
export function useMetaDrift() {
  const table = useObjectTable();
  return computed(() => {
    const list = table.value;
    if (!list) return [];
    const out = new Set();
    for (const o of list) for (const id of driftOf(o)) out.add(id);
    return [...out].sort();
  });
}

/**
 * Whether this object uses this control, and if not, why not. `null` when it
 * is live.
 *
 * **Greying stops entirely when the list is stale.** Showing a live control
 * that the engine ignores is a small wrong; hiding a control the engine reads
 * because a list forgot to be renamed is the kind of wrong that costs a user
 * the feature. Fail towards the working panel and print the disagreement.
 */
export function inactive(id, object, meta) {
  if (!meta || !Array.isArray(meta.uses)) return null;
  if (driftOf(meta).length) return null;
  if (meta.uses.includes(id)) return null;
  return WHY[id] || { short: 'not used by this object', why: `The engine does not read ${id} for a ${object.label}.` };
}

// ---------------------------------------------------------------------------
// The per-mode override table
// ---------------------------------------------------------------------------

/**
 * What an override may ask for, matching the engine's own clamps.
 *
 * **Two octaves of pitch, not one.** The engine widened it for a concrete
 * reason: putting a string's third partial onto a bell's tierce is 1,586 cents
 * down, so an octave was not enough room to build an object nobody has
 * shipped — which is the whole argument for having a per-partial table at all.
 */
export const EDIT_LIMITS = { cents: 2400, db: 60, decayMin: 0.1, decayMax: 10 };

const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
const neutral = (e) => !e || (!e.cents && !e.db && (e.decay == null || Math.abs(e.decay - 1) < 1e-9));

/**
 * The per-mode override table: frequency, gain and ring time, per partial.
 *
 * **It lives in the UI store and not in a message**, and the difference is not
 * cosmetic. A plug-in has no main loop — there is the audio thread and the
 * editor's thread and nothing else — so a message channel has nothing to pump
 * it. The message route works perfectly against a standalone dev server and
 * silently does nothing at all inside a VST3, which is the worst shape of bug
 * there is: it passes every test that can be run from this side. The store has
 * a write hook the engine picks up either way.
 *
 * It is also why the table keeps its promise: the store is serialised with the
 * plug-in state, so the overrides travel with a saved project and are
 * reapplied before the first block on load.
 *
 * **There is no reply channel and none is needed.** The `modes` stream is
 * sticky and already carries the current table, so the round trip closes
 * itself.
 *
 * `i` is the physical partial index as published in each frame's `index`
 * field — not a position in a list that Select reorders. An absent key, a
 * null, or an empty array all mean no overrides.
 */
export function useOverrides() {
  const stored = useStoredRef('modes', null);
  const edits = computed(() => {
    const v = stored.value;
    const list = v && Array.isArray(v.edits) ? v.edits : [];
    return list.filter((e) => e && Number.isFinite(e.i));
  });
  const write = (list) => {
    stored.value = list.length ? { edits: list } : null;
  };
  /**
   * A mode's key.
   *
   * **A pair, because a surface's modes need one.** Two different modes of a
   * rectangle routinely share their first index — `(2,1)` and `(2,3)` are
   * different shapes at different frequencies — so keying an override on `i`
   * alone would silently apply one edit to several resonances. On an object
   * with a single index `j` is 0 and the key degrades to the number.
   *
   * **The page never invents this identity, it copies it.** Both halves come
   * straight out of the `modes` frame and go back unchanged, so whatever the
   * engine means by them, the round trip is exact.
   */
  const keyOf = (i, j) => `${i}:${j || 0}`;
  const same = (e, i, j) => e.i === i && (e.j || 0) === (j || 0);
  return reactive({
    edits,
    count: computed(() => edits.value.length),
    byIndex: computed(() => new Map(edits.value.map((e) => [keyOf(e.i, e.j), e]))),
    keyOf,
    /** Whether this mode has an override. */
    has(i, j) {
      return edits.value.some((e) => same(e, i, j));
    },
    get(i, j) {
      return edits.value.find((e) => same(e, i, j)) || null;
    },
    /** One mode's override, merged over whatever it had. A neutral result removes the entry. */
    set(i, j, patch) {
      const next = edits.value.filter((e) => !same(e, i, j));
      const merged = { ...(edits.value.find((e) => same(e, i, j)) || {}), ...patch, i, j: j || 0 };
      if (merged.cents != null) merged.cents = clamp(merged.cents, -EDIT_LIMITS.cents, EDIT_LIMITS.cents);
      if (merged.db != null) merged.db = clamp(merged.db, -EDIT_LIMITS.db, EDIT_LIMITS.db);
      if (merged.decay != null) merged.decay = clamp(merged.decay, EDIT_LIMITS.decayMin, EDIT_LIMITS.decayMax);
      if (!neutral(merged)) next.push(merged);
      next.sort((a, b) => a.i - b.i || (a.j || 0) - (b.j || 0));
      write(next);
    },
    /**
     * Many modes at once, as one write.
     *
     * A drag across the display touches dozens of partials, and writing the
     * store once per partial would send the whole table dozens of times a
     * second. The gesture collects and commits once — which is also what
     * makes it one thing to undo by Reset rather than sixty-four.
     *
     * Each entry is `{ i, j, ...patch }`, merged over whatever that mode had,
     * and a patch that leaves a mode neutral removes it.
     */
    setMany(list) {
      if (!list.length) return;
      const byKey = new Map(edits.value.map((e) => [keyOf(e.i, e.j), { ...e }]));
      for (const { i, j, ...patch } of list) {
        const k = keyOf(i, j);
        const merged = { ...(byKey.get(k) || {}), ...patch, i, j: j || 0 };
        if (merged.cents != null) merged.cents = clamp(merged.cents, -EDIT_LIMITS.cents, EDIT_LIMITS.cents);
        if (merged.db != null) merged.db = clamp(merged.db, -EDIT_LIMITS.db, EDIT_LIMITS.db);
        if (merged.decay != null) merged.decay = clamp(merged.decay, EDIT_LIMITS.decayMin, EDIT_LIMITS.decayMax);
        if (neutral(merged)) byKey.delete(k);
        else byKey.set(k, merged);
      }
      write([...byKey.values()].sort((a, b) => a.i - b.i || (a.j || 0) - (b.j || 0)));
    },
    clear(i, j) {
      write(edits.value.filter((e) => !same(e, i, j)));
    },
    clearAll() {
      write([]);
    },
  });
}

// ---------------------------------------------------------------------------
// The streams
//
// ===========================================================================
//  THREE SETS, AND THEY ARE NOT THE SAME SET. READ THIS BEFORE CHANGING THEM.
// ===========================================================================
//
//  available  `info.modes_available` — every partial the object has under
//             Nyquist. A fact about the object. Not audible on its own; it is
//             what the bank is choosing from.
//
//  audible    `info.modes_used` — the resonators the bank is actually running,
//             which is the Modes budget spent by Select. **The top of this
//             set is a wall you can hear**, and `info.ceiling_hz` is where it
//             falls. Select chooses what is *synthesised*: the engine's own
//             `select.rs` puts all three orderings on the control "so that
//             the comparison can be heard as well as read".
//
//  drawn      the `modes` stream — the sixty-four loudest of the audible set,
//             for the picture. **A stream limit is not a wall.** Everything
//             it leaves out is still being synthesised and is still audible.
//
//  Running the last two together is a mistake this panel has already made
//  once. It printed "the object is deaf above 7 kHz" because a *display* feed
//  had run out, which is false, and is false in the worst possible way: it
//  looks exactly like a measurement, so nobody downstream can catch it. Never
//  draw a ceiling at the top of the drawn set, and never let the drawn count
//  stand in for either of the other two.
//
//  The display's own cut is therefore always *the loudest*, whatever Select
//  is set to — taking the lowest sixty-four to draw would show a cliff that
//  is not there.
// ---------------------------------------------------------------------------

/**
 * The `info` stream, read by name.
 *
 * The engine publishes eight numbers a block. The page asks for the ones it
 * understands and gets `null` for any the build does not declare, which is
 * what every optional readout on the panel branches on.
 */
/** The `info` fields that are counts of things rather than quantities. */
const COUNT_FIELDS = ['modes_used', 'modes_available'];

export function useInfo() {
  const has = hasStream('info');
  const frame = has ? useStreamFrame('info') : { value: null };
  const layout = has ? layoutOf('info') : { index: {} };
  const at = (name) => computed(() => fieldAt(frame.value, layout, name));
  const count = (name) => computed(() => countAt(frame.value, layout, name));
  return reactive({
    has,
    live: computed(() => frame.value != null),
    names: layout.index,
    /**
     * Whether this build declares a field at all — which is a different
     * question from whether it has a value for it.
     *
     * **Every readout that explains an absence has to ask this one instead.**
     * A field that is declared and non-finite is the engine saying *this does
     * not apply*: an air column has no crossover, a mode bank has no bore, a
     * limiter that is off took nothing off, and a bank holding every partial
     * an object has leaves no wall to draw. Those are correct states and the
     * best ones, and reporting them as missing fields reads as a broken build.
     * Only an undeclared field is a gap.
     */
    declares: (name) => declares(layout, name),
    /**
     * Count fields the engine published that cannot be counts.
     *
     * Printed rather than swallowed. Driving the fundamental to 1.2 Hz gave
     * the object more partials under Nyquist than a count can hold, and
     * `modes_available` arrived as 1.8446744e19 — which the panel rendered,
     * faithfully and absurdly, as *this object has 18446744073709552.0 k
     * partials*. The number is refused now, and this is how the face says it
     * was refused rather than merely missing.
     */
    bogusCounts: computed(() => {
      const f = frame.value;
      return COUNT_FIELDS.filter((n) => {
        const v = fieldAt(f, layout, n);
        return v != null && !isCount(v);
      });
    }),
    used: count('modes_used'),
    available: count('modes_available'),
    crossoverHz: at('crossover_hz'),
    columnM: at('column_m'),
    loopMs: at('loop_ms'),
    openHz: at('open_hz'),
    f0Hz: at('f0_hz'),
    tailDb: at('tail_db'),
    limitGrDb: at('limit_gr_db'),
    inharmB: at('inharm_b'),
    /** 0 for the mode bank, 1 for the waveguide — the engine's own word for which one is running. */
    engineIx: at('engine'),
    /**
     * How far a pending mode search has got, 0 to 1, and 1 when it has
     * settled. Worth reading and worth showing: the bank spreads its search
     * across blocks so no single block pays for all of it, which means the
     * display can be looking at a table that is still being filled in. A
     * half-built series that says nothing about being half-built is a picture
     * a reader will take at face value.
     */
    build: at('build'),
    /**
     * `ceiling_hz` is **not** in the engine's layout, so the wall where a mode
     * bank runs out cannot be drawn. It has been asked for. It is deliberately
     * not reconstructed here: the page renders what the engine publishes, and
     * a wall computed on this side would be the panel asserting something the
     * engine never said.
     */
    ceilingHz: at('ceiling_hz'),
  });
}

/**
 * The partial series, as published.
 *
 * `i` is the physical partial number an override addresses — **the mode, not
 * the row**. An edit follows that resonance when Selection or the mode budget
 * reorders the frame; if it followed the row instead, changing either would
 * silently reassign every edit and the result would look entirely plausible.
 *
 * `base_hz` and `db_bare` are optional and the engine does not currently
 * publish either, so the display draws no node ghosts and no Inharm marks and
 * says which field it is missing.
 */
export function useModes() {
  const has = hasStream('modes');
  const frame = has ? useStreamFrame('modes') : { value: null };
  const layout = has ? layoutOf('modes') : { stride: 0, index: {} };
  const list = computed(() => {
    const f = frame.value;
    const { stride, index: at } = layout;
    if (!f || !stride || at.hz == null) return [];
    const out = [];
    for (let o = 0; o + stride <= f.length; o += stride) {
      const hz = f[o + at.hz];
      if (!(hz > 0)) break;
      const get = (name) => (at[name] == null ? null : f[o + at[name]]);
      out.push({
        /** Position in the frame — for drawing order and nothing else. */
        row: out.length,
        /** The mode's own identity, copied verbatim and written back verbatim. */
        i: at.i == null ? out.length : Math.round(f[o + at.i]),
        /**
         * The mode's second index — nodal circles on a disc, the second
         * rectangle index on a surface. Zero on an object that has only one,
         * which is how the panel knows not to print a pair.
         */
        j: at.j == null ? 0 : Math.round(f[o + at.j]),
        hz,
        baseHz: get('base_hz'),
        bareDb: get('db_bare'),
        dbL: get('db_l'),
        dbR: get('db_r'),
        ring: get('t60_s'),
      });
    }
    return out;
  });
  return reactive({
    has,
    live: computed(() => list.value.length > 0),
    list,
    /** Whether the build publishes the fields the ghosts and the Inharm marks need. */
    /** Both optional and both currently absent from the engine's layout; the display says so rather than guessing. */
    hasBare: layout.index.db_bare != null,
    hasBaseHz: layout.index.base_hz != null,
  });
}

/**
 * The input and output levels, and whether either has clipped.
 *
 * The stream is four **linear** peaks in one frame, `1.0 = 0 dBFS` — not one
 * value per channel in decibels, which is what the framework's own meter
 * component expects, so this reads the frame itself and the panel draws it.
 *
 * **A resonator needs a meter more than most effects do.** A long decay and a
 * high mode count is a bank of resonators being fed continuously; get it
 * wrong and the output climbs long after the input stopped. The limiter is on
 * by default and optional, so the one number that matters is how much it is
 * having to take off — and that is on `info` as `limit_gr_db`.
 *
 * The peak hold decays rather than latching, because a bar that only ever
 * rises is unreadable; **the clip lamp latches**, because a clip you missed is
 * exactly the one worth knowing about. Clicking it clears it.
 */
export function useMeter() {
  const has = hasStream('meter');
  const frame = has ? useStreamFrame('meter') : { value: null };
  const layout = has ? layoutOf('meter') : { index: {} };
  const held = reactive({ in_l: 0, in_r: 0, out_l: 0, out_r: 0 });
  const clipped = ref(false);

  /** How much of the held peak survives each frame. Fast enough to follow a decay, slow enough to read. */
  const HOLD = 0.86;
  watch(
    () => frame.value,
    (f) => {
      if (!f) return;
      for (const name of ['in_l', 'in_r', 'out_l', 'out_r']) {
        const i = layout.index[name];
        const v = i == null || !Number.isFinite(f[i]) ? 0 : Math.abs(f[i]);
        held[name] = Math.max(v, held[name] * HOLD);
        if (name.startsWith('out') && v >= 1) clipped.value = true;
      }
    },
  );

  return reactive({
    has,
    live: computed(() => frame.value != null),
    held,
    clipped,
    clear: () => (clipped.value = false),
  });
}

/**
 * The stiff-string coefficient `B`, in the form it is published in.
 *
 * Inharm as a percentage says nothing about what the device is doing. `B` is
 * the quantity Fletcher's stiff-string relation is actually written in and
 * the one a reader can look up, so the face prints that instead — the same
 * rule that put Decay in seconds and Bright in decibels per octave.
 */
export function stiffnessText(b) {
  if (b == null || !Number.isFinite(b) || b === 0) return null;
  const e = Math.floor(Math.log10(Math.abs(b)));
  const m = b / 10 ** e;
  return `B ${m.toFixed(1)}e${e}`;
}

/** A linear peak as decibels, floored so silence does not go to negative infinity. */
export const linToDb = (v) => (v > 0 ? 20 * Math.log10(v) : -120);

/** The engine's own magnitude response: dB against a log frequency axis over the range its meta declares. */
export function useResponse() {
  const has = hasStream('response');
  const frame = has ? useStreamFrame('response') : { value: null };
  const meta = has ? getClient().stream('response').meta || {} : {};
  const range = Array.isArray(meta.hz_range) && meta.hz_range.length === 2 ? meta.hz_range : [20, 24000];
  return reactive({ has, range, points: computed(() => frame.value), live: computed(() => frame.value != null) });
}

/**
 * The fundamental every ratio on the display is measured against.
 *
 * The engine's own when it publishes one. Otherwise the Tune control, which
 * is the best the page can do without assuming how the engine folds Fine in —
 * so `fromEngine` says which, and the display's axis caption says so too.
 */
/**
 * Half the sample rate, from the manifest, or `null` when the build does not
 * publish one.
 *
 * The panel needs it for one thing only: **a partial the engine has held at
 * the ceiling is not where the object's ratios put it**, and the only way to
 * recognise one is to know where the ceiling is. Read from the manifest rather
 * than assumed, so a build at 96 kHz is not measured against a 48 kHz line.
 */
export function useNyquist() {
  const { manifest } = useNoobVstWebguiFramework();
  return computed(() => {
    const sr = manifest.value?.meta?.sample_rate;
    return Number.isFinite(sr) && sr > 0 ? sr / 2 : null;
  });
}

export function useFundamental() {
  const r = useRes();
  const info = useInfo();
  return computed(() => {
    if (info.f0Hz && info.f0Hz > 0) return { hz: info.f0Hz, fromEngine: true };
    return { hz: r.tune ? r.tune.plain : 220, fromEngine: false };
  });
}

/**
 * Page state that is not a parameter: whether the browse view is up.
 *
 * **Browsing must not touch the instance.** The resonator that is loaded goes
 * on ringing with its own settings the whole time the browser is open, so the
 * `type` parameter is written only when a row is chosen — never to preview,
 * which would be audible, would push entries onto the undo history and would
 * fight automation on that parameter.
 */
export const ui = reactive({
  /** The object browser is up. */
  browsing: false,
  /** The preset browser is up. Only ever one of the two: they are both whole-page layers. */
  presets: false,
  /** The chord menu is up. A third layer, and the same rule: only ever one. */
  chords: false,
});

/** Whether the bench panel is shown. Off by default: everything this panel has to say is already on its face. */
export function useDebug() {
  const stored = useStoredRef('debug.shown', false);
  return computed({ get: () => !!stored.value, set: (v) => (stored.value = !!v) });
}

// ---------------------------------------------------------------------------
// Printing
// ---------------------------------------------------------------------------

/** `15 kHz`, `1.5 kHz`, `440 Hz` — trailing zeros trimmed, because `1.00 kHz` on a panel reads as a false precision. */
export function hzText(hz) {
  if (hz == null || !Number.isFinite(hz)) return '—';
  if (hz < 1000) return `${hz < 100 ? hz.toFixed(1) : Math.round(hz)} Hz`;
  return `${(Math.round((hz / 1000) * 100) / 100).toString()} kHz`;
}

/** `78 cm`, `39 cm`, `1.4 m` — an air column's length, in the unit a person would say it in. */
export function lengthText(m) {
  if (m == null || !Number.isFinite(m)) return '—';
  if (m < 0.01) return `${(m * 1000).toFixed(0)} mm`;
  if (m < 1) return `${(m * 100).toFixed(1)} cm`;
  return `${m.toFixed(2)} m`;
}

/**
 * A mode's own name, when it has two indices: `(3, 2)` is three nodal
 * diameters and two nodal circles on a disc, or the two rectangle indices on
 * a surface. `null` on an object whose partials are just numbered.
 *
 * It is worth printing because on a dense two-dimensional series "partial 14"
 * says nothing and "(3, 2)" says exactly which shape is ringing.
 */
export const modeName = (p) => (p && p.j > 0 ? `(${p.i}, ${p.j})` : null);

/**
 * How to refer to one partial in prose.
 *
 * **The label follows what the index *means* on this object, never its
 * numeric value.** `j` is a lattice coordinate on a surface and a voice on a
 * line, and reading it as "not zero, so it is a pair" put `Partial 1` and
 * `Mode (1, 1)` in one list about two things of identical kind — and told a
 * reader that a beam has a second dimension, which it does not.
 *
 * So the caller says which world it is in, and `Mode (i, j)` is reserved for
 * the surfaces where the pair genuinely is a coordinate.
 *
 * **At one voice this is identical to what shipped before voices existed.**
 * Every row has `j = 0`, `voiced` is false, and the label is `Partial n` — a
 * user who never turns Voices up cannot tell the feature is there.
 *
 * @param {{ i: number, j: number }} p
 * @param {boolean} [voiced] Whether more than one voice is sounding on a line.
 */
export const partialName = (p, voiced = false) => {
  if (!p) return '';
  if (voiced) return `Voice ${(p.j || 0) + 1} · partial ${p.i}`;
  return p.j > 0 ? `Mode ${modeName(p)}` : `Partial ${p.i}`;
};

/** `4.2 s`, `250 ms` — a ring time in the unit it is comfortable in. */
export function timeText(s) {
  if (s == null || !Number.isFinite(s)) return '—';
  if (s < 1) return `${Math.round(s * 1000)} ms`;
  return `${s < 10 ? s.toFixed(2) : s.toFixed(1)} s`;
}

/**
 * A partial's ratio to the fundamental. Three decimals with trailing zeros
 * trimmed: the beam's second partial is 2.757 and needs every one of them,
 * and the string's fourth is 4 and would be a false precision as 4.000.
 */
export const ratioText = (r) => (r >= 100 ? r.toFixed(1) : r.toFixed(3)).replace(/\.?0+$/, '');

/** The same, for a headline. Three decimals is the precision of a partial, not of a summary. */
export const ratioShort = (r) => (r >= 10 ? r.toFixed(1) : r.toFixed(3)).replace(/\.?0+$/, '');

/** `1024`, `4.1 k` — a mode count, where thousands are the point. */
export const countText = (n) =>
  n == null || !Number.isFinite(n) ? '—' : n >= 10000 ? `${(n / 1000).toFixed(1)} k` : `${Math.round(n)}`;
