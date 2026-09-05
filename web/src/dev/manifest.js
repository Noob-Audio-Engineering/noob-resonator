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
export const MODE_FIELDS = 6;
export const INFO_LEN = 12;
export const RESPONSE_POINTS = 512;
export const METER_LEN = 4;

const SAMPLE_RATE = 48000;

/** A labelled parameter with no range of its own spans its step indices. */
const stepped = (list) => list.map((p) => (p.labels && p.max == null ? { min: 0, max: p.labels.length - 1, ...p } : p));

export const SELECT_LABELS = ['Loudest', 'Lowest', 'Log Spread'];
export const LFO_SHAPES = ['Sine', 'Square', 'Triangle', 'Ramp Up', 'Ramp Down', 'S&H', 'Random Ramp'];
export const FILTER_PLACES = ['Pre', 'Post'];
export const BAR_TUNINGS = ['Marimba 4:1', 'Xylophone 3:1'];
export const BAR_THIRDS = ['9.2x', '10x'];

/** The 41 host parameters, in `param_specs` order. `src_*` are standalone-only and not here. */
export const PARAMS = stepped([
  { id: 'type', name: 'Object', labels: OBJECTS.map((o) => o.label), default: 2, group: 'body' },
  { id: 'tune', name: 'Tune', min: 20, max: 4000, default: 220, unit: 'Hz', taper: 'log', group: 'body' },
  { id: 'transpose', name: 'Transpose', min: -48, max: 48, default: 0, unit: 'st', group: 'body' },
  { id: 'fine', name: 'Fine', min: -50, max: 50, default: 0, unit: 'ct', group: 'body' },
  { id: 'modes', name: 'Modes', min: 4, max: MAX_MODES, default: 1024, taper: 'log', group: 'engine' },
  { id: 'select', name: 'Selection', labels: SELECT_LABELS, default: 0, group: 'engine' },
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
 * Which controls each object has — the greying-out truth, published rather
 * than derived. The page reads this and reimplements none of it.
 *
 * **Still this file's proposal, and marked as one.** `dsp::object_meta()` is
 * the engine's own version and wins the moment the plug-in is connected.
 * Three rows here disagree with Ableton's Push table on purpose: Material,
 * Bright, Hit and the two pickups stay live on the air columns, because the
 * physics does not go away — a pipe loses its highs to the walls and the open
 * end exactly as a bar loses them to internal friction, and injecting a wave
 * a third of the way along a delay loop cancels every third harmonic. What
 * genuinely does not apply to a waveguide is the mode budget, because one
 * loop gives every resonance under Nyquist at one cost.
 *
 * `bleed` is listed everywhere, which is a deliberate non-answer: their table
 * lists it unconditionally and their manual says it is deactivated for the
 * air columns, and the table proves nothing by its silence.
 */
const COMMON = [
  'type', 'tune', 'transpose', 'fine', 'select', 'decay', 'material', 'damp_corner', 'damp_hi', 'tail',
  'bright', 'spread', 'width', 'filter_on', 'filter_freq', 'filter_width', 'filter_place',
  'lfo_on', 'lfo_shape', 'lfo_rate', 'lfo_depth', 'lfo_phase',
  'bleed', 'mix', 'gain', 'limiter', 'limit_ceil', 'bypass',
];
const CONTACT_1D = ['hit', 'pos_l', 'pos_r'];
const CONTACT_2D = [...CONTACT_1D, 'hit_y', 'pos_l_y', 'pos_r_y'];

const USES = {
  beam: [...COMMON, ...CONTACT_1D, 'modes', 'inharm'],
  marimba: [...COMMON, ...CONTACT_1D, 'modes', 'inharm', 'bar_tuning', 'bar_third'],
  string: [...COMMON, ...CONTACT_1D, 'modes', 'inharm'],
  membrane: [...COMMON, ...CONTACT_2D, 'modes', 'inharm', 'ratio'],
  plate: [...COMMON, ...CONTACT_2D, 'modes', 'inharm', 'ratio'],
  pipe: [...COMMON, ...CONTACT_1D, 'radius', 'opening'],
  tube: [...COMMON, ...CONTACT_1D, 'radius'],
  membrane_round: [...COMMON, ...CONTACT_2D, 'modes', 'inharm'],
};

/**
 * How each object's three contact points are addressed.
 *
 * `line` is one coordinate along a bar, a string or an air column. `xy` is a
 * rectangle. **`polar` is a disc**, and it is not a cosmetic difference: a
 * round head mapped as a square wastes the corners and clamps them to the
 * rim, where every mode of a clamped membrane is exactly zero — so a strike
 * in the corner of the control excited nothing at all. On a disc, X is the
 * distance from the centre and Y is the angle round it, and the angle decides
 * which orientation of a degenerate pair the strike aligns with, which is
 * audible.
 */
const COORDS = {
  beam: 'line', marimba: 'line', string: 'line',
  membrane: 'xy', plate: 'xy',
  pipe: 'line', tube: 'line',
  membrane_round: 'polar',
};

export const OBJECT_TABLE = OBJECTS.map((o) => ({
  id: o.id,
  label: o.label,
  engine: o.engine,
  coords: COORDS[o.id],
  uses: USES[o.id],
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
export const MODES_LAYOUT = ['i', 'j', 'hz', 'db_l', 'db_r', 't60_s'];
export const INFO_LAYOUT = [
  'modes_used', 'modes_available', 'crossover_hz', 'tail_db', 'limit_gr_db', 'inharm_b',
  'column_m', 'loop_ms', 'open_hz', 'engine', 'build', 'f0_hz',
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
  const object = objectAt(step('type', 2));
  return {
    object: object.id,
    engine: object.engine,
    f0: plain('tune', 220) * 2 ** (plain('transpose', 0) / 12 + plain('fine', 0) / 1200),
    modes: Math.round(plain('modes', 1024)),
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
    opening: plain('opening', 0) / 100,
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
  const drawn = loudest(audible, MAX_PARTIALS);

  const modes = new Float32Array(MAX_PARTIALS * MODE_FIELDS);
  drawn.forEach((p, k) => {
    const at = k * MODE_FIELDS;
    modes[at] = p.i;
    // The mode's second index. The stand-in does not track one, and the panel
    // does not read it; the engine fills it for the two-dimensional objects.
    modes[at + 1] = 0;
    modes[at + 2] = p.hz;
    modes[at + 3] = p.dbL;
    modes[at + 4] = p.dbR;
    modes[at + 5] = p.ring;
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
    if (i >= 0) info[i] = v;
  };
  put('modes_used', audible.length);
  put('modes_available', available.length);
  put('crossover_hz', drawn.length > resolvable(s.object) ? drawn[resolvable(s.object)].hz : 0);
  put('column_m', facts.metres);
  put('loop_ms', facts.loopS * 1000);
  put('engine', s.engine === 'waveguide' ? 1 : 0);
  // The stand-in has no incremental mode search, so its table is always settled.
  put('build', 1);
  put('f0_hz', s.f0);

  const response = s.engine === 'waveguide' ? Float32Array.from(guideResponse(s, RESPONSE_POINTS)) : null;

  cacheKey = key;
  cached = { modes, info, response };
  return cached;
}

export const offline = {
  name: 'noob-resonator',
  meta: {
    vendor: 'Noob Audio Engineering',
    version: 'dev',
    sample_rate: SAMPLE_RATE,
    standalone: true,
    /** The greying-out truth. `composables/useResonator.js` reads this and derives nothing. */
    objects: OBJECT_TABLE,
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
