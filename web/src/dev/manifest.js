/**
 * Design-time manifest for Noob Resonator — **development only**, loaded from
 * a dynamic import behind `import.meta.env.DEV`, so a production build does
 * not contain a byte of this directory.
 *
 * **This mirrors `src/dsp/mod.rs`. That file is the contract; this one is a
 * copy of it.** Where the two disagree, this is what is wrong. Everything
 * below — every id, range, default, label list and stream layout — is read
 * off the Rust, not proposed to it.
 *
 * Its second job is to stand in for the engine: the generators fill the same
 * four streams the engine fills, out of the quarantined equations in
 * `physics/`, so the page renders before a plug-in is running. **The panel
 * cannot tell the difference and must not be able to.**
 *
 * That is why the generators publish *exactly* the fields the engine
 * publishes and no more, even where the equations could easily produce more.
 * They could compute where the bank runs out; the engine has no
 * `ceiling_hz` field, so they do not publish one and the panel draws no wall
 * in either mode. A design mode that can show something live mode cannot is
 * a design mode that lies about the product.
 *
 * Nothing here animates: every generator is a pure function of the parameter
 * values, so the panel sits still until a control moves.
 */
import { getClient } from '@noob-audio-engineering/noob-vst-webgui-framework/vue';
import { OBJECTS, objectAt } from '../objects.js';
import {
  PUBLISHED,
  allPartialsCounted,
  bankResponse,
  ceilingHz,
  columnFacts,
  computePartials,
  guideResponse,
  loudest,
  resolvable,
  selectPartials,
} from './physics/model.js';

/** From `dsp::engine`: `MAX_EDITS`, `MODE_FIELDS`, `INFO_LEN`, `RESPONSE_POINTS`. */
export const MAX_MODES = 4096;
export const MAX_PARTIALS = 64;
export const MODE_FIELDS = 8;
export const INFO_LEN = 13;
export const RESPONSE_POINTS = 512;
export const METER_LEN = 4;

const SAMPLE_RATE = 48000;

/** A labelled parameter with no range of its own spans its step indices. */
const stepped = (list) => list.map((p) => (p.labels && p.max == null ? { min: 0, max: p.labels.length - 1, ...p } : p));

/**
 * The voice pitch ids, in the engine's order, published as `meta.voice_ids` so
 * the page never builds `'voice' + n` by hand.
 */
export const VOICE_IDS = ['voice1', 'voice2', 'voice3', 'voice4', 'voice5', 'voice6'];

/**
 * The chord table, copied from `dsp::chord`.
 *
 * **A menu that fills the voice pitches, never a parameter.** A chord
 * parameter would be a second place a voice's pitch is decided, and the moment
 * somebody nudged one voice the two would disagree about what the chord is
 * with no way to say which is true. A menu that only ever writes has no such
 * state.
 *
 * **Voicings for a resonator rather than a keyboard**: the thirds and sevenths
 * sit an octave up so each voice keeps its own register. Six fundamentals
 * inside four semitones is one thick note rather than a chord.
 */
export const CHORDS = [
  { group: 'Triads', name: 'Major', semis: [0, 7, 16], voices: 3 },
  { group: 'Triads', name: 'Minor', semis: [0, 7, 15], voices: 3 },
  { group: 'Triads', name: 'Diminished', semis: [0, 6, 15], voices: 3 },
  { group: 'Triads', name: 'Augmented', semis: [0, 8, 16], voices: 3 },
  { group: 'Triads', name: 'Sus 2', semis: [0, 7, 14], voices: 3 },
  { group: 'Triads', name: 'Sus 4', semis: [0, 7, 17], voices: 3 },
  { group: 'Sevenths', name: 'Major 7', semis: [0, 7, 16, 23], voices: 4 },
  { group: 'Sevenths', name: 'Minor 7', semis: [0, 7, 15, 22], voices: 4 },
  { group: 'Sevenths', name: 'Dominant 7', semis: [0, 7, 16, 22], voices: 4 },
  { group: 'Sevenths', name: 'Half Diminished', semis: [0, 6, 15, 22], voices: 4 },
  { group: 'Extended', name: 'Major 9', semis: [0, 7, 16, 23, 26], voices: 5 },
  { group: 'Extended', name: 'Minor 9', semis: [0, 7, 15, 22, 26], voices: 5 },
  { group: 'Stacks', name: 'Fifths', semis: [0, 7, 12, 19, 24, 31], voices: 6 },
  { group: 'Stacks', name: 'Octaves', semis: [0, 12, 24], voices: 3 },
  { group: 'Stacks', name: 'Fourths', semis: [0, 5, 10, 15, 20, 25], voices: 6 },
  { group: 'Stacks', name: 'Unison', semis: [0], voices: 1 },
];

export const SELECT_LABELS = ['Loudest', 'Lowest', 'Log Spread'];
export const LFO_SHAPES = ['Sine', 'Square', 'Triangle', 'Ramp Up', 'Ramp Down', 'S&H', 'Random Ramp'];
export const FILTER_PLACES = ['Pre', 'Post'];
export const BAR_TUNINGS = ['Marimba 4:1', 'Xylophone 3:1'];
export const BAR_THIRDS = ['9.2x', '10x'];

/** The 41 host parameters, in `param_specs` order. `src_*` are standalone-only and not here. */
export const PARAMS = stepped([
  { id: 'type', name: 'Object', labels: OBJECTS.map((o) => o.label), default: 2, group: 'body' },
  { id: 'tune', name: 'Tune', min: 20, max: 4000, default: 220, unit: 'Hz', taper: 'log', group: 'body' },
  // `decimals: 0` on the two that are counts of whole things, mirroring the
  // engine. **A hint the manifest declares and the mirror omits is a
  // design-versus-live divergence**, and this one showed itself: the Modes knob
  // read 4.00 against the stand-in and 4 against the engine, on the same page.
  { id: 'transpose', name: 'Transpose', min: -48, max: 48, default: 0, unit: 'st', decimals: 0, group: 'body' },
  { id: 'fine', name: 'Fine', min: -50, max: 50, default: 0, unit: 'ct', group: 'body' },
  // `mode_budget`, not `modes`: the stream is called `modes`, and a name that
  // means two things on one wire is a trap for whoever comes next. Not
  // "quality" either — that is their word for a control that truncates by
  // frequency and then needs a Bleed knob to restore what it threw away, and
  // this one spends a budget by contribution. The face still says Modes.
  { id: 'mode_budget', name: 'Modes', min: 4, max: MAX_MODES, default: 1024, taper: 'log', decimals: 0, group: 'engine' },
  { id: 'select', name: 'Selection', labels: SELECT_LABELS, default: 0, group: 'engine' },

  // The voices. **A voice is a root, not a partial**: each one gets the
  // object's own series, so six voices on a Beam is six beams. That is why
  // they sit beside the object rather than replacing it, and why the
  // two-dimensional objects simply do not offer them.
  { id: 'voices', name: 'Voices', min: 1, max: 6, default: 3, decimals: 0, group: 'chord' },
  ...VOICE_IDS.map((id, i) => ({
    id,
    name: `Voice ${i + 1}`,
    min: -24,
    max: 36,
    default: [0, 7, 16, 12, 19, 24][i],
    decimals: 0,
    unit: 'st',
    group: 'chord',
  })),
  { id: 'ratio', name: 'Ratio', min: 0.2, max: 5, default: 1, taper: 'log', group: 'body' },
  { id: 'bar_tuning', name: 'Bar Tuning', labels: BAR_TUNINGS, default: 0, group: 'body' },
  { id: 'bar_third', name: 'Third Partial', labels: BAR_THIRDS, default: 0, group: 'body' },
  { id: 'radius', name: 'Radius', min: 1, max: 100, default: 20, unit: 'mm', taper: 'log', group: 'body' },
  { id: 'opening', name: 'Opening', min: 0, max: 100, default: 0, unit: '%', group: 'body' },

  { id: 'decay', name: 'Decay', min: 0.02, max: 60, default: 2, unit: 's', taper: 'log', group: 'damping' },
  { id: 'material', name: 'Material', min: -1, max: 1, default: -0.5, group: 'damping' },
  { id: 'damp_corner', name: 'Damp Corner', min: 100, max: 20000, default: 20000, unit: 'Hz', taper: 'log', group: 'damping' },
  { id: 'damp_hi', name: 'HF Slope', min: -2, max: 1, default: -1, group: 'damping' },
  { id: 'tail', name: 'Tail', toggle: true, default: 1, group: 'damping' },
  // Not flat. At exactly 0 dB/oct a mass-normalised mode set has no amplitude
  // trend, and a membrane has far more partials per octave high up — so they
  // all crowd the same maximum and "keep the loudest" collapses into "keep the
  // highest", which is the ordering Selection exists to beat. Measured at 512
  // modes on a 110 Hz membrane: at 0 dB/oct nothing survives between 1.5 and
  // 10 kHz; at −3, 341 partials do. Flat is still selectable.
  { id: 'bright', name: 'Bright', min: -6, max: 6, default: -3, unit: 'dB/oct', group: 'damping' },
  { id: 'inharm', name: 'Inharm', min: -100, max: 100, default: 0, unit: '%', group: 'damping' },

  { id: 'hit', name: 'Hit X', min: 0, max: 100, default: 20, unit: '%', group: 'contact' },
  { id: 'hit_y', name: 'Hit Y', min: 0, max: 100, default: 20, unit: '%', group: 'contact' },
  { id: 'pos_l', name: 'Pos L X', min: 0, max: 100, default: 30, unit: '%', group: 'contact' },
  { id: 'pos_l_y', name: 'Pos L Y', min: 0, max: 100, default: 30, unit: '%', group: 'contact' },
  { id: 'pos_r', name: 'Pos R X', min: 0, max: 100, default: 70, unit: '%', group: 'contact' },
  { id: 'pos_r_y', name: 'Pos R Y', min: 0, max: 100, default: 70, unit: '%', group: 'contact' },
  { id: 'spread', name: 'Spread', min: 0, max: 100, default: 0, unit: '%', group: 'contact' },
  { id: 'width', name: 'Width', min: 0, max: 100, default: 100, unit: '%', group: 'contact' },

  { id: 'filter_on', name: 'Exciter Filter', toggle: true, default: 0, group: 'exciter' },
  { id: 'filter_freq', name: 'Filter Freq', min: 50, max: 18000, default: 1000, unit: 'Hz', taper: 'log', group: 'exciter' },
  { id: 'filter_width', name: 'Filter Width', min: 0.5, max: 9, default: 4, unit: 'oct', group: 'exciter' },
  { id: 'filter_place', name: 'Filter Place', labels: FILTER_PLACES, default: 0, group: 'exciter' },

  { id: 'lfo_on', name: 'LFO', toggle: true, default: 0, group: 'lfo' },
  { id: 'lfo_shape', name: 'LFO Shape', labels: LFO_SHAPES, default: 0, group: 'lfo' },
  { id: 'lfo_rate', name: 'LFO Rate', min: 0.01, max: 20, default: 1, unit: 'Hz', taper: 'log', group: 'lfo' },
  { id: 'lfo_depth', name: 'LFO Depth', min: 0, max: 12, default: 0, unit: 'st', group: 'lfo' },
  { id: 'lfo_phase', name: 'LFO Phase', min: 0, max: 360, default: 180, unit: '°', group: 'lfo' },

  { id: 'bleed', name: 'Bleed', min: 0, max: 100, default: 0, unit: '%', group: 'out' },
  { id: 'mix', name: 'Dry/Wet', min: 0, max: 100, default: 100, unit: '%', group: 'out' },
  { id: 'gain', name: 'Gain', min: -36, max: 36, default: 0, unit: 'dB', group: 'out' },
  { id: 'limiter', name: 'Limiter', toggle: true, default: 1, group: 'out' },
  { id: 'limit_ceil', name: 'Ceiling', min: -24, max: 0, default: -0.3, unit: 'dB', group: 'out' },
  { id: 'bypass', name: 'Bypass', toggle: true, default: 0, group: 'out' },
]);

/**
 * Which controls each object has — **a copy of `dsp::object_meta()`**, not a
 * proposal to it. The page reads this and reimplements none of it.
 *
 * Three things about the shape, all of which the page depends on:
 *
 * * **`id` is the object's index, not a string.** A saved project loads by
 *   index, so that is what identifies an object on the wire.
 * * **`engine` is `"bank"` or `"waveguide"`** — the engine's own words.
 * * **`forces`** says what choosing this object pins. A Tube is a Pipe with
 *   its far end fully open, so the engine forces `opening` to 1 rather than
 *   the panel writing it: they are one loop at two settings of one
 *   termination, and both keep their own index because a user who picks Tube
 *   has asked for a tube, not for a pipe to open by hand.
 *
 * The air columns keep Bright, Hit and both pickups, which is the physics —
 * a pipe loses its highs to the walls and the open end, and injecting a wave
 * a third of the way along a delay loop cancels every third harmonic. What
 * they do not get is the mode budget, the selection, or the damping law's
 * three extra controls.
 */
const COMMON = [
  'type', 'tune', 'transpose', 'fine', 'decay', 'spread', 'width',
  'filter_on', 'filter_freq', 'filter_width', 'filter_place',
  'lfo_on', 'lfo_shape', 'lfo_rate', 'lfo_depth', 'lfo_phase',
  'bleed', 'mix', 'gain', 'limiter', 'limit_ceil', 'bypass',
];
const BANK_ONLY = [
  'mode_budget', 'select', 'material', 'damp_corner', 'damp_hi', 'tail', 'bright',
  'inharm', 'hit', 'pos_l', 'pos_r',
];

/**
 * The objects that offer voices — **the one-dimensional ones, and that is a
 * consequence rather than a preference.**
 *
 * A mode's identity is the pair `(i, j)`, and a one-dimensional object leaves
 * `j` at zero — so the voice goes there and a one-voice chord is bit-identical
 * to the encoding that already ships. Nothing re-keys, no stream field is
 * added, and every override in every saved preset stays where it was. A
 * two-dimensional object already uses both indices, so voices there need a
 * third, which is a decision to make later on evidence rather than now on
 * principle.
 */
const HAS_VOICES = ['beam', 'marimba', 'string', 'tine', 'pipe', 'tube'];

const TWO_D = ['membrane', 'plate', 'membrane_round', 'plate_round'];
const HAS_ASPECT = ['membrane', 'plate'];

/**
 * The objects whose contact controls are a radius and an angle.
 *
 * Both discs, and for a while only one of them was: `object_meta()` mapped
 * Membrane Round to `polar` while `Walk::new` and `Contacts::psi` read the
 * clamped disc as polar too, so the published meta contradicted the audio
 * thread. A panel mirroring it would have offered a user an X and a Y for an
 * object the engine reads as a radius and an angle — and the engine's own
 * comment is the argument against that: a square mapped into a circle puts
 * the control's corners on the rim, where a clamped disc's every mode is
 * exactly zero. Found by writing this table out beside theirs, and fixed on
 * their side.
 */
const POLAR = ['membrane_round', 'plate_round'];

function usesFor(o) {
  const uses = [...COMMON];
  if (o.engine === 'waveguide') {
    uses.push('radius', 'hit', 'pos_l', 'pos_r', 'bright');
    if (o.id === 'pipe') uses.push('opening');
    if (HAS_VOICES.includes(o.id)) uses.push('voices', ...VOICE_IDS);
  } else {
    uses.push(...BANK_ONLY);
    if (HAS_VOICES.includes(o.id)) uses.push('voices', ...VOICE_IDS);
    if (TWO_D.includes(o.id)) uses.push('hit_y', 'pos_l_y', 'pos_r_y');
    if (HAS_ASPECT.includes(o.id)) uses.push('ratio');
    if (o.id === 'marimba') uses.push('bar_tuning', 'bar_third');
  }
  return [...new Set(uses)].sort();
}

const NOTES = {
  tube: 'a Pipe with its far end fully open: the same loop, one reflection at its extreme',
  pipe: 'an air column with a variable far end, from fully closed to fully open',
};

export const OBJECT_TABLE = OBJECTS.map((o, i) => ({
  id: i,
  label: o.label,
  engine: o.engine === 'waveguide' ? 'waveguide' : 'bank',
  forces: o.id === 'tube' ? { opening: 1.0 } : null,
  note: NOTES[o.id] || '',
  coords: POLAR.includes(o.id) ? 'polar' : TWO_D.includes(o.id) ? 'xy' : 'line',
  uses: usesFor(o),
}));

/**
 * The stream layouts, copied from `dsp::streams`, and **read by name** on the
 * page rather than by offset.
 *
 * Reading by name is what lets a field come and go without breaking a panel.
 * Two that the display would like are not here — `db_bare`, a partial's level
 * before the strike and the pickups took their share, and `base_hz`, where it
 * sat before Inharm moved it — so the node ghosts and the Inharm marks are
 * not drawn, and the display says which field it is missing. A third,
 * `ceiling_hz`, would let it draw the wall where a mode bank runs out. All
 * three have been asked for. None is reconstructed here.
 */
/**
 * The chord half of the manifest meta, as `bridge_meta` publishes it.
 *
 * **This half is ahead of the engine and says so.** The build on the wire
 * today still carries chord tuning as an eleventh *object*; the lead has ruled
 * that it is orthogonal instead — voices on the six one-dimensional objects,
 * the voice in `j`, and no `Chord` object — and res-engine is making that
 * change. Design mode mirrors the ruling so the face can be built and looked
 * at, which is the one thing this directory is for. **When the ruling reaches
 * the wire this comment goes**, and the check is the same as every other
 * contract change tonight: read it off a running bridge, not off a message.
 */
export const CHORD_META = { chords: CHORDS, voice_ids: VOICE_IDS, chord_voices: VOICE_IDS.length };

export const MODES_LAYOUT = ['i', 'j', 'hz', 'db_l', 'db_r', 't60_s', 'db_bare', 'base_hz'];
export const INFO_LAYOUT = [
  'modes_used', 'modes_available', 'crossover_hz', 'tail_db', 'limit_gr_db', 'inharm_b',
  'column_m', 'loop_ms', 'open_hz', 'engine', 'build', 'f0_hz', 'ceiling_hz',
];

export const STREAMS = [
  {
    id: 'meter',
    name: 'Meter',
    kind: 'meter',
    capacity: METER_LEN,
    channels: 2,
    meta: { layout: 'in_l,in_r,out_l,out_r', sample_rate: SAMPLE_RATE },
  },
  {
    id: 'modes',
    name: 'Partials',
    kind: 'raw',
    capacity: MAX_PARTIALS * MODE_FIELDS,
    sticky: true,
    meta: {
      layout: MODES_LAYOUT.join(','),
      fields: MODE_FIELDS,
      max_partials: MAX_PARTIALS,
      terminator: 'hz = 0',
      note: 'the loudest partials for a mode bank, the lowest resonances for an air column',
    },
  },
  { id: 'info', name: 'Readouts', kind: 'raw', capacity: INFO_LEN, meta: { layout: INFO_LAYOUT.join(',') } },
  {
    id: 'response',
    name: 'Response',
    kind: 'curve',
    capacity: RESPONSE_POINTS,
    sticky: true,
    meta: {
      hz_range: [20, SAMPLE_RATE / 2],
      points: RESPONSE_POINTS,
      unit: 'dB',
      log: true,
      note: "the engine's own response, normalised to its own peak",
    },
  },
];

// ---------------------------------------------------------------------------
// Standing in for the engine
// ---------------------------------------------------------------------------

const plain = (id, fallback = 0) => {
  try {
    const p = getClient().param(id);
    return p ? p.plain : fallback;
  } catch {
    return fallback;
  }
};

/**
 * The **step** a labelled parameter is on.
 *
 * Not `param.index`, which is that parameter's ordinal position in the
 * manifest — reading it here silently made every labelled control read as its
 * own position in the parameter list, so the object was always the first one
 * and Selection always the second. The panel showed String and the generator
 * produced a beam, and nothing threw.
 */
const step = (id, fallback = 0) => {
  try {
    const p = getClient().param(id);
    if (!p) return fallback;
    return Math.round(p.value * Math.max(1, (p.spec.steps || 1) - 1));
  } catch {
    return fallback;
  }
};

const edits = () => {
  try {
    const v = getClient().store.get('modes');
    return v && Array.isArray(v.edits) ? v.edits : [];
  } catch {
    return [];
  }
};

/** Everything the generators need, read once per frame from the client. */
function readState() {
  const ix = step('type', 2);
  const object = objectAt(ix);
  // What the engine pins for this object. A Tube's far end is open by
  // definition, and the engine forces it rather than the panel writing it.
  const forced = OBJECT_TABLE[ix]?.forces || {};
  return {
    object: object.id,
    engine: object.engine,
    f0: plain('tune', 220) * 2 ** (plain('transpose', 0) / 12 + plain('fine', 0) / 1200),
    modes: Math.round(plain('mode_budget', 1024)),
    select: SELECT_LABELS[step('select', 0)] || 'Loudest',
    inharm: plain('inharm', 0) / 100,
    bright: plain('bright', -3),
    material: plain('material', -0.5),
    decay: plain('decay', 2),
    hit: plain('hit', 20) / 100,
    hitY: plain('hit_y', 20) / 100,
    posL: plain('pos_l', 30) / 100,
    posLY: plain('pos_l_y', 30) / 100,
    posR: plain('pos_r', 70) / 100,
    posRY: plain('pos_r_y', 70) / 100,
    spread: plain('spread', 0) / 100,
    ratio: plain('ratio', 1),
    opening: forced.opening != null ? forced.opening : plain('opening', 0) / 100,
    radius: plain('radius', 20),
    barSecond: step('bar_tuning', 0) === 1 ? 3 : 4,
    barThird: step('bar_third', 0) === 1 ? 10 : 9.2,
    nyquist: SAMPLE_RATE / 2,
    edits: edits(),
  };
}

/**
 * The streams, recomputed only when something they depend on has moved.
 *
 * Memoising on the state makes the stillness explicit: same key, same frame,
 * and nothing on the panel can drift while nobody is touching it.
 */
let cacheKey = null;
let cached = null;

function build() {
  const s = readState();
  const key = JSON.stringify(s);
  if (key === cacheKey) return cached;

  const available = computePartials(s);
  const audible =
    s.engine === 'waveguide'
      ? available
      : selectPartials(available, s.select, Math.max(1, Math.min(available.length, s.modes)));
  // Every mode the user has edited stays in the published set, as the engine
  // does it, so a partial you turned down does not vanish from the display
  // that shows you turning it down.
  const editedKeys = new Set((s.edits || []).map((e) => `${e.i}:${e.j || 0}`));
  const drawn = loudest(audible, MAX_PARTIALS, editedKeys);

  const modes = new Float32Array(MAX_PARTIALS * MODE_FIELDS);
  drawn.forEach((p, k) => {
    const at = k * MODE_FIELDS;
    modes[at] = p.mi;
    // The mode's second index: nodal circles on a disc, the second rectangle
    // index on a surface, and 0 on anything with only one.
    modes[at + 1] = p.mj;
    modes[at + 2] = p.hz;
    modes[at + 3] = p.dbL;
    modes[at + 4] = p.dbR;
    modes[at + 5] = p.ring;
    // The level with unit mode shapes at both contacts — the tilt alone — so
    // the gap between this and `db_l` is exactly the energy the strike or the
    // pickup removed.
    modes[at + 6] = p.bareDb;
    // Where this partial sat before Inharm moved it.
    modes[at + 7] = p.baseHz;
  });

  const facts = s.engine === 'waveguide' ? columnFacts(s) : { metres: 0, loopS: 0 };
  // **Not zero-filled.** A zero in `limit_gr_db` reads as "the limiter is
  // taking nothing off", which is a measurement the stand-in never made; NaN
  // reads as "not computed", and the page's field reader already turns a
  // non-finite value into `null` and hides the readout that wanted it. An
  // unset field must be absent, not plausible.
  const info = new Float32Array(INFO_LEN).fill(NaN);
  const put = (name, v) => {
    const i = INFO_LAYOUT.indexOf(name);
    if (i >= 0 && Number.isFinite(v)) info[i] = v;
  };
  put('modes_used', audible.length);
  // **Only when it is a count rather than a table length.** The stand-in walks
  // a generated series table, and where that runs out before Nyquist does the
  // number it could publish would be the size of the table rather than
  // anything about the object. Left unset, the panel reads "not computed" —
  // which is true — instead of drawing a ceiling this file invented.
  if (allPartialsCounted(s)) put('modes_available', available.length);
  // **Every field that does not apply is left unset**, which is the engine's
  // contract: NaN reads as *not applicable*, and the frame is NaN-filled
  // already. Publishing a plausible zero for a bank's bore or for a crossover
  // that nothing crossed is the same fault as the zero-filled frame, arriving
  // from the other direction.
  if (drawn.length > resolvable(s.object)) put('crossover_hz', drawn[resolvable(s.object)].hz);
  if (s.engine === 'waveguide') {
    put('column_m', facts.metres);
    put('loop_ms', facts.loopS * 1000);
  }
  put('engine', s.engine === 'waveguide' ? 1 : 0);
  // The stand-in has no incremental mode search, so its table is always settled.
  put('build', 1);
  put('f0_hz', s.f0);
  // **Unset when the bank has every partial the object has**: no wall to draw,
  // which is the best state the device reaches and not the absence of one.
  // `put` drops a non-finite value, so the field stays NaN and the panel says
  // *no wall* in ordinary ink rather than reporting a missing feed.
  put('ceiling_hz', ceilingHz(s, available, audible));

  // The engine publishes a response for both engines — `engine.rs` takes it
  // from `guides[0]` or `banks[0]` — so the stand-in does too, or the panel
  // would show something in one mode it cannot show in the other.
  const response = Float32Array.from(
    s.engine === 'waveguide' ? guideResponse(s, RESPONSE_POINTS) : bankResponse(s, audible, RESPONSE_POINTS),
  );

  cacheKey = key;
  cached = { modes, info, response };
  return cached;
}

/**
 * Design-mode presets — **stand-ins, exactly like the stream generators.**
 *
 * Factory presets are the engine's: they come out of `Settings` structs where
 * they cannot fall outside a range or contradict an object, and they arrive
 * in `meta.presets` in this same shape. These exist so the preset view has
 * something to render before that, and the engine's own replace them wholesale
 * the moment a plug-in answers.
 *
 * **The last two are a deliberate pair** — the same string at the same budget
 * with Selection on Loudest and on Lowest — because that comparison is the
 * argument this device is built around, and the browser finds pairs by
 * diffing values rather than by name, so it marks them without being told.
 */
const PRESET_STANDINS = [
  {
    v: 1, name: 'Struck Slate', group: 'Plate',
    description: 'a sheet of metal hit with something soft: dense, spread and slow to settle',
    values: { type: 4, tune: 110, decay: 6, material: -0.2, bright: -4, ratio: 2.1, mode_budget: 1024, select: 0, hit: 32, pos_l: 24, pos_r: 76 },
    modes: [],
  },
  {
    v: 1, name: 'Timpani Head', group: 'Membrane Round',
    description: 'struck away from the centre, where the diameters live',
    values: { type: 7, tune: 82, decay: 3.4, material: -0.4, bright: -3, mode_budget: 512, select: 0, hit: 62, hit_y: 25, pos_l: 40, pos_r: 70 },
    modes: [],
  },
  {
    v: 1, name: 'Tuned Bar', group: 'Marimba',
    description: 'the arch cut to four to one, struck off its node',
    values: { type: 1, tune: 220, decay: 1.6, material: -0.6, bright: -5, bar_tuning: 0, bar_third: 0, mode_budget: 256, select: 0, hit: 50 },
    modes: [{ i: 2, j: 0, cents: -8 }],
  },
  {
    v: 1, name: 'Stopped Pipe', group: 'Pipe',
    description: 'odd harmonics only, an octave below the length you would guess',
    values: { type: 5, tune: 146, decay: 2.2, radius: 34, opening: 0, bright: -2, hit: 18, pos_l: 30, pos_r: 70 },
    modes: [],
  },
  {
    v: 1, name: 'String · Loudest', group: 'String',
    description: 'twenty-four resonators spent on the partials you can hear',
    values: { type: 2, tune: 55, decay: 4, material: -0.5, bright: -3, mode_budget: 24, select: 0, hit: 20, pos_l: 30, pos_r: 70 },
    modes: [],
  },
  {
    v: 1, name: 'String · Lowest', group: 'String',
    description: 'the same twenty-four spent from the bottom up, which is where the object goes deaf',
    values: { type: 2, tune: 55, decay: 4, material: -0.5, bright: -3, mode_budget: 24, select: 1, hit: 20, pos_l: 30, pos_r: 70 },
    modes: [],
  },
];

export const offline = {
  name: 'noob-resonator',
  meta: {
    vendor: 'Noob Audio Engineering',
    version: 'dev',
    sample_rate: SAMPLE_RATE,
    standalone: true,
    /** The greying-out truth. `composables/useResonator.js` reads this and derives nothing. */
    objects: OBJECT_TABLE,
    /** Stand-ins; the engine's own factory presets arrive here and replace them. */
    presets: PRESET_STANDINS,
    ...CHORD_META,
  },
  params: PARAMS,
  streams: STREAMS,
  frameRate: 10,
  frames: {
    modes: () => build().modes,
    info: () => build().info,
    response: () => build().response,
  },
};
