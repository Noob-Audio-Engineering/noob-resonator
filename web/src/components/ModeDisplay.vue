<script setup>
/**
 * Where the partials land, how long each one rings, where the object runs out,
 * and where the ear stops telling them apart. Click one to edit it.
 *
 * **This draws streams and computes nothing.** Every position, every height
 * and every ring time here arrives from the engine on `modes`, `response` and
 * `info`; what this component owns is the axes, the marks and what is said
 * about them. That is the larger half — a panel is where a plug-in is
 * understood — but none of it is arithmetic about vibration.
 *
 * This is the display the device needs and Ableton's Corpus has no equivalent
 * of. **A resonator's whole character is decided by where its partials sit**;
 * damping, brightness and strike position only colour a series those ratios
 * have already fixed. Switch from String to Beam and the answer to "why does
 * one sing and the other clang" is on the screen.
 *
 * **The axis is the ratio to the fundamental, not the frequency.** The ratios
 * are the whole game — 1, 2, 3, 4 sings and 1, 2.757, 5.404, 8.933 clangs —
 * and neither statement is about hertz. On a ratio axis the octave gridlines
 * are fixed furniture, so a string's partials fall on a ruler you can read
 * off, a marimba's tuned second partial lands exactly on the 4× line where
 * the maker put it, and switching objects moves the series against a scale
 * that has not moved. A hertz axis spent a third of its width below the
 * fundamental, where nothing ever happens.
 *
 * **The ceiling.** When the bank runs out before the axis does, the engine
 * says so in `ceiling_hz` and the display draws the line in the warning
 * colour. A bank that stops at partial N is complete only above the
 * fundamental where the Nth partial clears Nyquist; below that it stops dead
 * inside the audio band. Pull Tune down and watch the wall walk in.
 *
 * **The resolvability crossover.** Above `crossover_hz` the display stops
 * drawing separate lines and draws a band, because a listener resolves only
 * about twelve to thirty partials on a bar and two to six on a membrane, and
 * separate lines above that claim a distinction the ear is not making. It is
 * also what the engine does: an exact bank below the crossover and a
 * statistically matched extension above it. "More modes" is the wrong axis.
 *
 * **The strike and the pickups take their share visibly.** Striking an object
 * at a mode's node gives that mode no energy. A nulled partial draws a short
 * bar, and a short bar on its own looks like a modelling choice rather than a
 * physical fact — so the rose ghost above it is the height it would have had,
 * drawn from the engine's `db_bare`. A build that does not publish that field
 * simply has no ghosts.
 *
 * **The edits.** Nine global knobs generating thousands of modes with not one
 * of them reachable is the gap in every device of this kind. Click a partial
 * and set its pitch, its level and its ring time.
 */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import {
  hzText,
  lengthText,
  partialName,
  ratioShort,
  ratioText,
  timeText,
  useDesignMode,
  useFundamental,
  useNyquist,
  useInfo,
  useModes,
  useObject,
  useOverrides,
  useRes,
  useResponse,
} from '../composables/useResonator.js';
import ModeEditor from './ModeEditor.vue';

const r = useRes();
const object = useObject();
const modes = useModes();
const info = useInfo();
const response = useResponse();
const overrides = useOverrides();
const designMode = useDesignMode();
const fundamental = useFundamental();

const box = ref(null);
const plate = ref(null);
const W = ref(760);
const H = ref(240);
/** The whole plate's height, which is what decides whether the optional rows fit. */
const plateH = ref(400);
let ro = null;
onMounted(() => {
  ro = new ResizeObserver((entries) => {
    for (const e of entries) {
      if (e.target === box.value) {
        W.value = Math.max(240, e.contentRect.width);
        H.value = Math.max(110, e.contentRect.height);
      } else {
        plateH.value = e.contentRect.height;
      }
    }
  });
  ro.observe(box.value);
  ro.observe(plate.value);
});
onBeforeUnmount(() => ro?.disconnect());

/**
 * Which of the optional rows fit.
 *
 * **Measured against the plate, not the window.** These used to be dropped by
 * a viewport-height media query, which was the wrong variable: how much room
 * the display has depends on how tall the deck is, and the deck's height
 * depends on how many control groups an object has. So at 900 x 520 the
 * provenance line spilled out over the deck and the plate had to be given
 * `overflow: hidden` to contain it — a clip, not a fix, and one that would
 * have silently swallowed the next thing added here.
 *
 * Now the plate reports its own height and the rows that do not fit are not
 * rendered. Nothing is ever clipped, and adding a row later cannot hide one.
 */
const LEGEND_NEEDS = 128;
const PROV_NEEDS = 158;
const showLegend = computed(() => plateH.value >= LEGEND_NEEDS);
const showProv = computed(() => plateH.value >= PROV_NEEDS);

/** `t` is deep enough to clear the stamp, which sits inside the plot because it is a statement about the level axis. */
const PAD = { l: 34, r: 10, t: 21, b: 25 };
const LANE_GAP = 8;
const RING_SHARE = 0.3;
const RING_MIN = 34;
const RING_MAX = 120;
/** Below this the ring lane is dropped and the legend says so: a lane twenty pixels deep cannot show three decades. */
const RING_MIN_PLOT = 120;
/** The ring lane's axis, in seconds — Decay's own range, rounded outward. */
const RING_LO = 0.01;
const RING_HI = 60;
/** How far below the fundamental the axis starts, so the first partial is not on the frame. */
const R_MIN = 0.82;
/** The floor of the level axis. */
const DB_FLOOR = -66;

const guide = computed(() => object.value.engine === 'waveguide');
const f0 = computed(() => Math.max(1e-6, fundamental.value.hz));
const nyquist = computed(() => (response.has ? response.range[1] : 24000));

/**
 * Nyquist as the build actually states it, or `null`.
 *
 * **Not the same value as the one above, and the difference is the point.**
 * `nyquist` falls back to 24 kHz so the axis always has a top, which is fine
 * for drawing a ruler and not fine for deciding that the engine has clamped a
 * partial — that is a claim about what the engine did, and a guessed ceiling
 * would make it on no evidence. When the build says nothing, nothing is
 * marked.
 */
const statedNyquist = useNyquist();

const geom = computed(() => {
  const rMax = Math.max(4, nyquist.value / f0.value);
  const plotW = Math.max(20, W.value - PAD.l - PAD.r);
  const plotH = Math.max(36, H.value - PAD.t - PAD.b);
  const hasRing = plotH >= RING_MIN_PLOT;
  const ringH = Math.min(RING_MAX, Math.max(RING_MIN, (plotH - LANE_GAP) * RING_SHARE));
  const levelH = hasRing ? plotH - LANE_GAP - ringH : plotH;
  const span = Math.log(rMax / R_MIN);
  return {
    rMax,
    plotW,
    hasRing,
    x: (ratio) => PAD.l + (Math.log(Math.min(rMax, Math.max(R_MIN, ratio)) / R_MIN) / span) * plotW,
    levelTop: PAD.t,
    levelBottom: PAD.t + levelH,
    levelH,
    ringTop: PAD.t + levelH + LANE_GAP,
    ringBottom: PAD.t + plotH,
    ringH,
  };
});
const xHz = (hz) => geom.value.x(hz / f0.value);

/**
 * The loudest published partial, so the level lane reads as relative to it
 * however Brightness is tilted.
 *
 * Zero before the first frame arrives: an empty list reduces to −Infinity,
 * and every gridline on the plot is placed relative to this, so the whole
 * axis came out NaN for the frame between mount and the first stream.
 */
const peakDb = computed(() => {
  const v = modes.list.reduce(
    (m, p) => Math.max(m, p.dbL ?? -Infinity, p.dbR ?? -Infinity, p.bareDb ?? -Infinity),
    -Infinity,
  );
  return Number.isFinite(v) ? v : 0;
});
const yLevel = (db) => {
  const g = geom.value;
  return g.levelBottom - ((Math.max(DB_FLOOR, db - peakDb.value) - DB_FLOOR) / -DB_FLOOR) * g.levelH;
};
const yRing = (sec) => {
  const g = geom.value;
  const t = Math.log(Math.min(RING_HI, Math.max(RING_LO, sec)) / RING_LO) / Math.log(RING_HI / RING_LO);
  return g.ringBottom - t * g.ringH;
};

// --- grid -----------------------------------------------------------------

const rTicks = computed(() => {
  const g = geom.value;
  const out = [];
  for (let ratio = 1; ratio <= g.rMax; ratio *= 2) {
    out.push({ r: ratio, x: g.x(ratio), label: `${ratio}×`, hz: hzText(ratio * f0.value) });
  }
  return out;
});
const dbTicks = computed(() => {
  const step = geom.value.levelH < 110 ? -24 : -12;
  const out = [];
  for (let d = 0; d >= -60; d += step) out.push({ d, y: yLevel(peakDb.value + d) });
  return out;
});
const ringTicks = computed(() =>
  [0.01, 0.1, 1, 10].map((v) => ({ v, y: yRing(v), label: timeText(v) })),
);

// --- the series -----------------------------------------------------------

const crossX = computed(() => (info.crossoverHz > 0 ? xHz(info.crossoverHz) : null));

/** How many partials have to share one frequency before it is a stack and not a coincidence. */
const STACK_MIN = 3;

/**
 * The partials sharing the highest frequency in the drawn set, when there are
 * enough of them to be a stack rather than a coincidence.
 *
 * **Detected from what it is, not from where the ceiling is guessed to be.**
 * The first version of this tested `hz` against Nyquist and found nothing: the
 * engine clamps at 23.52 kHz on a 24 kHz band, so a threshold tight enough to
 * mean "at the ceiling" missed every one, and a threshold loose enough to catch
 * them would have been a number chosen to make the test pass. What is
 * observable is that **several distinct modes are sharing one frequency**,
 * which cannot be true of an object — two modes do not sound at one pitch,
 * except for the degenerate pairs a square has, and those come in twos and
 * fours and sit anywhere in the series rather than three-deep at its top.
 */
function heldOf(list) {
  if (list.length < STACK_MIN) return [];
  const topHz = list.reduce((a, b) => (b.hz > a ? b.hz : a), 0);
  if (!(topHz > 0)) return [];
  const held = list.filter((b) => b.hz >= topHz * 0.999);
  if (held.length < STACK_MIN) return [];
  // A stack belongs at the top of the band. Anything lower is the object's own
  // degeneracy, and the engine is not holding it anywhere.
  const ny = statedNyquist.value;
  if (ny != null && topHz < ny * 0.85) return [];
  return held;
}

const bars = computed(() => {
  const list = modes.list.map((p) => {
    const top = Math.max(p.dbL ?? DB_FLOOR, p.dbR ?? DB_FLOOR);
    return {
      ...p,
      /**
       * The mode's own identity, and what every list here is keyed on.
       *
       * **A surface's modes routinely share a first index**, so keying on `i`
       * alone collides: (1,5) and (1,6) are two different partials at two
       * different frequencies with one key between them, and Vue then patches
       * one element where two were meant. It was warning on both discs and
       * every rectangle. The same pair that identifies an override identifies
       * these, which is the point of having the pair at all.
       */
      key: `${p.i}:${p.j || 0}`,
      ratio: p.hz / f0.value,
      x: xHz(p.hz),
      xBase: p.baseHz ? xHz(p.baseHz) : null,
      yBare: p.bareDb == null ? null : yLevel(p.bareDb),
      yL: yLevel(p.dbL ?? DB_FLOOR),
      yR: yLevel(p.dbR ?? DB_FLOOR),
      yTop: yLevel(top),
      edited: overrides.has(p.i, p.j),
      /** Nothing measurable came out of it. */
      dead: top <= DB_FLOOR + 0.5,
      /** Set in the second pass below, which needs the whole list to see a stack. */
      held: false,
      /** The engine says this partial started higher, so a node took the difference. */
      lost: p.bareDb != null && top < p.bareDb - 1.2,
      /** Above the crossover the ear fuses them, so they are drawn as a band. */
      fused: info.crossoverHz > 0 && p.hz > info.crossoverHz,
    };
  });
  for (const b of heldOf(list)) b.held = true;
  return list;
});
/** The stack, as the display draws and describes it. A plain read of `bars`. */
const ceilingStack = computed(() => {
  const held = bars.value.filter((b) => b.held);
  if (!held.length) return null;
  const flip = W.value - PAD.r - held[0].x < 130;
  return {
    count: held.length,
    x: held[0].x,
    hz: held[0].hz,
    ceiling: statedNyquist.value,
    anchor: flip ? 'end' : 'start',
    tx: flip ? held[0].x - 5 : held[0].x + 5,
  };
});

const resolved = computed(() => bars.value.filter((b) => !b.fused));
const fused = computed(() => bars.value.filter((b) => b.fused));

/**
 * The fused partials, as an envelope between a running maximum and minimum.
 *
 * Not a polyline through their tops: half of them up there are sitting in a
 * node's null, so a line joining the tops is a sawtooth, and a sawtooth reads
 * as detail — the opposite of what a band is for.
 */
const FUSE_WINDOW = 2;
function envelope(list, pick) {
  return list.map((_, i) => {
    let v = pick === 'max' ? -Infinity : Infinity;
    for (let j = Math.max(0, i - FUSE_WINDOW); j <= Math.min(list.length - 1, i + FUSE_WINDOW); j++) {
      v = pick === 'max' ? Math.max(v, list[j].yTop) : Math.min(v, list[j].yTop);
    }
    return { x: list[i].x, y: v };
  });
}
const fusedBand = computed(() => {
  const f = fused.value;
  if (f.length < 3) return '';
  const hi = envelope(f, 'min');
  const lo = envelope(f, 'max');
  const start = resolved.value.length ? resolved.value[resolved.value.length - 1] : null;
  if (start) {
    hi.unshift({ x: start.x, y: start.yTop });
    lo.unshift({ x: start.x, y: start.yTop });
  }
  const up = hi.map((p, i) => `${i ? 'L' : 'M'} ${p.x.toFixed(2)} ${p.y.toFixed(2)}`).join(' ');
  const down = lo.slice().reverse().map((p) => `L ${p.x.toFixed(2)} ${p.y.toFixed(2)}`).join(' ');
  return `${up} ${down} Z`;
});
const fusedFloor = computed(() => {
  const f = fused.value;
  if (f.length < 3) return '';
  const g = geom.value;
  const lo = envelope(f, 'max');
  const line = lo.map((p, i) => `${i ? 'L' : 'M'} ${p.x.toFixed(2)} ${p.y.toFixed(2)}`).join(' ');
  return `${line} L ${lo[lo.length - 1].x.toFixed(2)} ${g.levelBottom} L ${lo[0].x.toFixed(2)} ${g.levelBottom} Z`;
});

const barW = computed(() => {
  const b = resolved.value;
  let gap = Infinity;
  for (let i = 1; i < b.length; i++) gap = Math.min(gap, b[i].x - b[i - 1].x);
  return Math.max(1.4, Math.min(3.4, gap === Infinity ? 3.4 : gap * 0.62));
});

/** Where a partial was before Inharm and any override moved it, when the engine publishes that. */
const ghostLines = computed(() =>
  bars.value.filter((b) => b.xBase != null && Math.abs(b.xBase - b.x) > 1.5).map((b) => b.xBase),
);

/** Where the bank runs out — the engine's own figure, or nothing drawn. */
const cut = computed(() => {
  const hz = info.ceilingHz;
  if (!hz || hz <= 0) return null;
  const x = xHz(hz);
  const flip = W.value - PAD.r - x < 150;
  return {
    x,
    hz,
    anchor: flip ? 'end' : 'start',
    tx: flip ? x - 5 : x + 5,
    label: `nothing above ${hzText(hz)} · ${info.used} modes, ${r.select ? r.select.label : ''}`,
  };
});

const ringPath = computed(() =>
  bars.value
    .filter((p) => p.ring != null)
    .map((p, i) => `${i ? 'L' : 'M'} ${p.x.toFixed(2)} ${yRing(p.ring).toFixed(2)}`)
    .join(' '),
);

// --- the air column's response --------------------------------------------

/**
 * The engine's own magnitude response, for **either** engine.
 *
 * On an air column it is the primary reading — the resonances are the peaks
 * of one loop and there is no list to draw. On a mode bank it goes *behind*
 * the bars, faintly, and it is the one thing the bars cannot say: how wide
 * each resonance is. Two objects with identical partials and different ring
 * times draw identical bars and sound nothing alike, and this is the
 * difference, drawn.
 */
const band = computed(() => {
  const pts = response.points;
  if (!pts || pts.length < 8) return null;
  const g = geom.value;
  const [lo, hi] = response.range;
  const step = (hi / lo) ** (1 / (pts.length - 1));
  const at = (i) => ({ x: xHz(lo * step ** i).toFixed(2), y: yLevel(peakDb.value + pts[i]).toFixed(2) });
  const line = Array.from(pts, (_, i) => {
    const p = at(i);
    return `${i ? 'L' : 'M'} ${p.x} ${p.y}`;
  }).join(' ');
  const first = at(0);
  const last = at(pts.length - 1);
  return { line, fill: `${line} L ${last.x} ${g.levelBottom} L ${first.x} ${g.levelBottom} Z` };
});

// --- editing --------------------------------------------------------------

/**
 * The partial being edited.
 *
 * **It survives leaving the published set.** The stream carries the sixty-four
 * loudest, and turning one down by nine decibels can drop it out of them —
 * which once took the bar and the editor off the screen in the middle of the
 * edit that caused it. The page cannot put it back, because what is published
 * is the engine's decision, so it holds the last frame the partial appeared
 * in and the editor keeps working: an override addresses a physical index and
 * does not need the partial to be on screen.
 */
const picked = ref(null);
const lastSeen = ref(null);
const isPicked = (p) => picked.value != null && picked.value === overrides.keyOf(p.i, p.j);
const pickedPartial = computed(() => {
  if (picked.value == null) return null;
  const live = bars.value.find((p) => overrides.keyOf(p.i, p.j) === picked.value);
  if (live) {
    lastSeen.value = live;
    return live;
  }
  return lastSeen.value ? { ...lastSeen.value, offscreen: true } : null;
});
function pick(p) {
  const k = overrides.keyOf(p.i, p.j);
  picked.value = picked.value === k ? null : k;
  lastSeen.value = picked.value == null ? null : p;
}
function select(p) {
  picked.value = overrides.keyOf(p.i, p.j);
  lastSeen.value = p;
}

/**
 * The partials, from the keyboard.
 *
 * **Editing a mode was mouse-only**, which made the one feature no competing
 * device has unreachable without a pointer. Sixty-four tab stops would be
 * worse than none, so the plot is a single stop and the arrows walk along the
 * series — left and right by one, Home and End to the ends, Escape to let go.
 * That is how a list of sixty-four things should be traversed anyway.
 */
function onPlotKey(e) {
  const list = bars.value;
  if (!list.length) return;
  const here = picked.value == null ? -1 : list.findIndex((p) => overrides.keyOf(p.i, p.j) === picked.value);
  let next = null;
  if (e.key === 'ArrowRight' || e.key === 'ArrowDown') next = here < 0 ? 0 : Math.min(list.length - 1, here + 1);
  else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') next = here < 0 ? list.length - 1 : Math.max(0, here - 1);
  else if (e.key === 'Home') next = 0;
  else if (e.key === 'End') next = list.length - 1;
  else if (e.key === 'Escape' && picked.value != null) {
    picked.value = null;
    e.preventDefault();
    return;
  } else return;
  select(list[next]);
  e.preventDefault();
}

// --- drawing across the modes ------------------------------------------

/**
 * Shaping the whole series by drawing across it.
 *
 * **The per-partial editor is right for a correction and hopeless for
 * sixty-four of them.** The surface for the other gesture already exists —
 * the display draws every partial against a level lane and a ring lane, each
 * with its own scale — so a drag across a lane can set that quantity for
 * every partial it passes.
 *
 * **It generates into the table and the table stays editable.** The drag
 * writes ordinary per-mode overrides, exactly the ones a click would write,
 * so afterwards you can still pick one partial and correct it by hand.
 * Nothing is locked while drawing is on, and turning it off leaves the edits
 * where they are. Draw the shape, then fix the two that are wrong.
 *
 * **Level and ring only, because those are the lanes that exist.** Pitch is a
 * horizontal quantity on this display — a partial's frequency *is* its
 * position — so dragging vertically cannot mean it, and detuning stays a
 * per-partial edit rather than getting a gesture that would have to lie about
 * its own axis.
 *
 * The gesture collects as it goes and commits once on release, so a drag
 * across the whole series is one write and one thing to undo.
 */
const DRAW_TARGETS = [
  { id: 'off', label: 'Off' },
  { id: 'db', label: 'Level' },
  { id: 'decay', label: 'Ring' },
];
const drawing = ref('off');
/** Edits accumulated by the current drag, keyed by mode, shown before they are committed. */
const pending = ref(new Map());
let dragging = false;

/** How far either side of the pointer a partial is taken to be under it, in pixels. */
const REACH = 7;

function svgPoint(e) {
  const el = e.currentTarget;
  const r = el.getBoundingClientRect();
  return { x: ((e.clientX - r.left) / r.width) * W.value, y: ((e.clientY - r.top) / r.height) * H.value };
}

/** The value a y position means, for whichever lane is being drawn in. */
function valueAt(y) {
  const g = geom.value;
  if (drawing.value === 'db') {
    const t = (g.levelBottom - y) / Math.max(1, g.levelH);
    return peakDb.value + DB_FLOOR * (1 - Math.min(1, Math.max(0, t)));
  }
  const t = (g.ringBottom - y) / Math.max(1, g.ringH);
  return RING_LO * (RING_HI / RING_LO) ** Math.min(1, Math.max(0, t));
}

function paint(e) {
  if (drawing.value === 'off' || !dragging) return;
  const { x, y } = svgPoint(e);
  const g = geom.value;
  const inLevel = y >= g.levelTop - 6 && y <= g.levelBottom + 6;
  const inRing = g.hasRing && y >= g.ringTop - 6 && y <= g.ringBottom + 6;
  if (drawing.value === 'db' ? !inLevel : !inRing) return;
  const target = valueAt(y);
  const next = new Map(pending.value);
  for (const p of bars.value) {
    if (Math.abs(p.x - x) > REACH) continue;
    const k = overrides.keyOf(p.i, p.j);
    if (drawing.value === 'db') {
      // The override is an offset from the level the partial would have had,
      // so the drawn shape is what you get rather than what you get plus
      // whatever the tilt was already doing.
      const bare = p.bareDb != null ? p.bareDb : Math.max(p.dbL ?? DB_FLOOR, p.dbR ?? DB_FLOOR);
      next.set(k, { i: p.i, j: p.j, db: Math.round((target - bare) * 10) / 10 });
    } else if (p.ring > 0) {
      // The published ring already includes any multiplier this mode has, so
      // the base is that divided back out before the new one is worked out.
      const had = overrides.get(p.i, p.j)?.decay ?? 1;
      const base = p.ring / had;
      next.set(k, { i: p.i, j: p.j, decay: Math.round((target / base) * 100) / 100 });
    }
  }
  pending.value = next;
}

function startDraw(e) {
  if (drawing.value === 'off') return;
  dragging = true;
  pending.value = new Map();
  e.currentTarget.setPointerCapture?.(e.pointerId);
  paint(e);
}
function endDraw() {
  if (!dragging) return;
  dragging = false;
  const list = [...pending.value.values()];
  pending.value = new Map();
  if (list.length) overrides.setMany(list);
}

/** What a partial looks like right now, including a stroke not yet committed. */
function previewOf(p) {
  return pending.value.get(overrides.keyOf(p.i, p.j)) || null;
}

const top = computed(() => {
  const l = bars.value;
  return l.length ? `${ratioShort(l[l.length - 1].ratio)}×` : '—';
});
/**
 * What this display is not drawing, and why — named per stream, the way the
 * sibling plug-in's transfer display names its own.
 *
 * **A fallback that is not visible is worse than no fallback.** Two of these
 * used to be silent: with no `response` stream an air column simply drew no
 * curve, and with no `info` stream the ceiling and the crossover quietly went
 * unmarked. Nothing was wrong on screen, which is exactly the problem — the
 * reader had no way to know the picture was short of a claim it usually makes.
 */
/**
 * The engine's own word for which engine is running, against the catalogue's.
 *
 * **Redundant on purpose, which is what makes it worth reading.** The panel
 * already knows the object is a waveguide because the catalogue says so, and
 * `info.engine` says it a second time from the other side of the wire. A
 * field that always agrees costs nothing; the one time it disagrees, the page
 * and the DSP have drifted apart about what is being synthesised, and that is
 * a fault that would otherwise be invisible — the display would draw a
 * perfectly reasonable picture of the wrong engine.
 */
const engineDisagrees = computed(() => {
  if (info.engineIx == null) return false;
  return (info.engineIx >= 0.5 ? 'waveguide' : 'modal') !== object.value.engine;
});

/**
 * What the display is *not* drawing because there is nothing to draw — which
 * is a different sentence from what it cannot draw.
 *
 * Ordinary ink, not the fault colour. "No wall" on a bank that holds every
 * partial its object has is the device working, and the readout that says so
 * is the one worth reading.
 */
/**
 * What the display is drawing that a reader would otherwise misread.
 *
 * Not a fault and not a gap: a real state that looks like something it is not,
 * which is its own category and deserves its own sentence.
 */
const readCarefully = computed(() => {
  const st = ceilingStack.value;
  if (!st) return [];
  return [
    `${st.count} partials are sharing one frequency at ${hzText(st.hz)}` +
      (st.ceiling ? `, just under the ${hzText(st.ceiling)} ceiling` : '') +
      ' — the engine holds a partial there rather than letting it alias, so those are drawn where they' +
      ' sound rather than where the object puts them',
  ];
});

const notApplicable = computed(() => {
  const out = [];
  if (!info.live) return out;
  if (!guide.value && info.declares('ceiling_hz') && info.ceilingHz == null) {
    out.push('no wall — the bank has every partial this object has');
  }
  // Only on a mode bank. The engine publishes NaN for `crossover_hz` on an air
  // column because it is a bank field that does not apply there — which is not
  // the same statement as "nothing is fused", and putting the bank's sentence
  // on a waveguide would be the panel answering a question nobody asked of it.
  if (!guide.value && info.declares('crossover_hz') && info.crossoverHz == null && modes.list.length) {
    out.push('nothing is fused — every partial drawn is one the ear can still separate');
  }
  return out;
});

const missing = computed(() => {
  const out = [];
  if (engineDisagrees.value) {
    out.push(
      `the engine reports a ${info.engineIx >= 0.5 ? 'waveguide' : 'mode bank'} and this object is a ` +
        `${object.value.engine === 'waveguide' ? 'waveguide' : 'mode bank'} — one of the two is wrong`,
    );
  }
  if (guide.value && !response.live) {
    out.push('no response stream in this build, so the loop’s own curve is not drawn');
  }
  if (!info.live) {
    out.push('no info stream, so the ceiling and the crossover are not marked');
  } else {
    // **Declared and non-finite is not missing.** The engine publishes NaN for
    // every field that does not apply, so a bank holding every partial an
    // object has leaves `ceiling_hz` unset — there is no wall because nothing
    // was thrown away, which is the best state this device reaches. Reported
    // as an absent field it read as a broken build, on the one screen a user
    // is most likely to screenshot. Only a field the layout never declares is
    // a gap.
    if (!info.declares('ceiling_hz')) {
      out.push('no ceiling_hz, so where the bank runs out is not marked');
    }
    if (!info.declares('crossover_hz')) out.push('no crossover_hz, so nothing is drawn as fused');
  }
  if (modes.live && !modes.hasBare) out.push('no db_bare, so the energy a node removed is not shown');
  return out;
});

/**
 * Whether the engine is still filling the table.
 *
 * The bank spreads its mode search across blocks so no single block pays for
 * all of it, which means this display can be looking at a series that is
 * still arriving. A half-built picture that does not say it is half-built is
 * one a reader takes at face value.
 */
const settling = computed(() => info.build != null && info.build < 0.999);

const tip = (p) =>
  `${partialName(p)} · ${ratioText(p.ratio)}× · ${hzText(p.hz)}` +
  (p.ring != null ? ` · rings ${timeText(p.ring)}` : '') +
  (p.dead ? ' · a node took it' : '') +
  (p.edited ? ' · edited' : '') +
  ' — click to edit';
</script>

<template>
  <section ref="plate" class="md plate">
    <div class="md__head">
      <h2 class="md__title">Where the partials land</h2>
      <span class="md__badge" :class="object.engine">
        {{ guide ? 'waveguide · one loop' : 'mode bank · one filter each' }}
      </span>
      <!--
        Drawing shapes the whole series; clicking corrects one partial. Both
        write the same overrides, so one is never a mode you have to leave to
        use the other.
      -->
      <div class="md__draw" role="group" aria-label="Draw across the modes">
        <span class="md__drawcap">Draw</span>
        <button
          v-for="t in DRAW_TARGETS"
          :key="t.id"
          type="button"
          class="md__drawkey"
          :class="{ on: drawing === t.id }"
          :disabled="t.id === 'decay' && !geom.hasRing"
          :title="t.id === 'off' ? 'Click a partial to edit it' : `Drag across the ${t.id === 'db' ? 'level' : 'ring'} lane to shape every partial you pass`"
          @click="drawing = t.id"
        >
          {{ t.label }}
        </button>
      </div>

      <div class="md__facts tabular">
        <span><b>{{ bars.length }}</b> drawn</span>
        <span>top at <b>{{ top }}</b></span>
        <span v-if="overrides.count"><b>{{ overrides.count }}</b> edited</span>
        <span v-if="info.columnM > 0">air column <b>{{ lengthText(info.columnM) }}</b></span>
        <span v-if="info.loopMs > 0">loop <b>{{ info.loopMs.toFixed(2) }} ms</b></span>
      </div>
    </div>

    <div ref="box" class="md__box">
      <svg
        :viewBox="`0 0 ${W} ${H}`"
        :width="W"
        :height="H"
        tabindex="0"
        role="listbox"
        aria-label="The partial series. Arrow keys move between partials."
        :aria-activedescendant="picked ? `partial-${picked.replace(':', '-')}` : undefined"
        :class="{ 'is-drawing': drawing !== 'off' }"
        @keydown="onPlotKey"
        @pointerdown="startDraw"
        @pointermove="paint"
        @pointerup="endDraw"
        @pointercancel="endDraw"
        @pointerleave="endDraw"
      >
        <g class="g-grid">
          <line v-for="t in rTicks" :key="`r${t.r}`" :x1="t.x" :y1="PAD.t" :x2="t.x" :y2="geom.ringBottom" :class="{ root: t.r === 1 }" />
          <line v-for="t in dbTicks" :key="`d${t.d}`" :x1="PAD.l" :y1="t.y" :x2="W - PAD.r" :y2="t.y" :class="{ zero: t.d === 0 }" />
          <template v-if="geom.hasRing">
            <line v-for="t in ringTicks" :key="`rg${t.v}`" :x1="PAD.l" :y1="t.y" :x2="W - PAD.r" :y2="t.y" />
          </template>
        </g>
        <g class="g-axis">
          <text v-for="t in rTicks" :key="`rl${t.r}`" :x="t.x" :y="H - 13" text-anchor="middle" class="ratio">{{ t.label }}</text>
          <text v-for="t in rTicks" :key="`rh${t.r}`" :x="t.x" :y="H - 3" text-anchor="middle" class="hz">{{ t.hz }}</text>
          <text v-for="t in dbTicks" :key="`dl${t.d}`" :x="PAD.l - 4" :y="t.y + 3" text-anchor="end">{{ t.d }}</text>
          <template v-if="geom.hasRing">
            <text v-for="t in ringTicks" :key="`rt${t.v}`" :x="PAD.l - 4" :y="t.y + 3" text-anchor="end" class="soft">{{ t.label }}</text>
          </template>
        </g>

        <g class="g-ghostline">
          <line v-for="(gx, i) in ghostLines" :key="`gl${i}`" :x1="gx" :y1="geom.levelTop" :x2="gx" :y2="geom.levelBottom" />
        </g>

        <!-- the bank's own response, behind its bars: how wide each resonance is -->
        <g v-if="!guide && band" class="g-behind">
          <path :d="band.fill" class="fill" />
          <path :d="band.line" class="line" />
        </g>

        <!-- the mode bank: one bar per resonator, up to where the ear stops separating them -->
        <g v-if="!guide">
          <path v-if="fusedFloor" :d="fusedFloor" class="g-fused-floor" />
          <path v-if="fusedBand" :d="fusedBand" class="g-fused" />
          <g class="g-lost">
            <rect
              v-for="p in resolved.filter((q) => q.lost)"
              :key="`gh${p.key}`"
              :x="p.x - barW / 3"
              :y="p.yBare"
              :width="Math.max(1, barW / 1.5)"
              :height="Math.max(0, p.yTop - p.yBare)"
            />
          </g>
          <g v-if="pending.size" class="g-pending">
            <rect
              v-for="p in resolved.filter((q) => previewOf(q))"
              :key="`pd${p.key}`"
              :x="p.x - barW / 2 - 1"
              :y="geom.levelTop"
              :width="barW + 2"
              :height="Math.max(0, geom.levelBottom - geom.levelTop)"
            />
          </g>
          <g class="g-bar">
            <rect
              v-for="p in resolved"
              :key="`b${p.key}`"
              :class="{ edited: p.edited, picked: isPicked(p) }"
              :id="`partial-${p.i}-${p.j || 0}`"
              role="option"
              :aria-selected="isPicked(p)"
              :x="p.x - barW / 2"
              :y="p.yL"
              :width="barW"
              :height="Math.max(0, geom.levelBottom - p.yL)"
              @click="pick(p)"
            >
              <title>{{ tip(p) }}</title>
            </rect>
          </g>
          <g class="g-right">
            <line v-for="p in resolved" :key="`r${p.key}`" :x1="p.x - barW" :y1="p.yR" :x2="p.x + barW" :y2="p.yR" />
          </g>
        </g>

        <!-- the air column: one loop, drawn as the response the engine publishes -->
        <g v-else-if="band" class="g-curve">
          <path :d="band.fill" class="fill" />
          <path :d="band.line" class="line" />
        </g>

        <!-- every partial is reachable from the baseline, fused or not -->
        <g class="g-handles">
          <line
            v-for="p in bars"
            :key="`h${p.key}`"
            :class="{ edited: p.edited, picked: isPicked(p), guide, held: p.held }"
            :x1="p.x"
            :y1="geom.levelBottom - 5"
            :x2="p.x"
            :y2="geom.levelBottom"
            @click="pick(p)"
          >
            <title>{{ tip(p) }}</title>
          </line>
        </g>

        <g class="g-dead">
          <circle v-for="p in bars.filter((q) => q.dead)" :key="`x${p.key}`" :cx="p.x" :cy="geom.levelBottom - 2" r="2" />
        </g>

        <g v-if="crossX && fused.length" class="g-cross">
          <line :x1="crossX" :y1="geom.levelTop" :x2="crossX" :y2="geom.levelBottom" />
          <text :x="crossX + 5" :y="geom.levelBottom - 6">above here the ear fuses them</text>
        </g>

        <!--
          Partials the engine is holding at the ceiling, drawn as the stack
          they are. Neither dropped nor drawn as ordinary partials: both would
          be a false picture, one by omission and one by making twenty look
          like one.
        -->
        <g v-if="ceilingStack" class="g-held">
          <line :x1="ceilingStack.x" :y1="geom.levelTop" :x2="ceilingStack.x" :y2="geom.levelBottom" />
          <text :x="ceilingStack.tx" :y="geom.levelBottom - 4" :text-anchor="ceilingStack.anchor">
            {{ ceilingStack.count }} held here
          </text>
        </g>

        <g v-if="cut" class="g-cut select">
          <rect :x="cut.x" :y="geom.levelTop" :width="Math.max(0, W - PAD.r - cut.x)" :height="geom.levelH" />
          <line :x1="cut.x" :y1="geom.levelTop" :x2="cut.x" :y2="geom.levelBottom" />
          <text :x="cut.tx" :y="geom.levelTop + 11" :text-anchor="cut.anchor">{{ cut.label }}</text>
          <text :x="cut.tx" :y="geom.levelTop + 23" :text-anchor="cut.anchor" class="sub">nothing at all above this line</text>
        </g>

        <g v-if="geom.hasRing && ringPath" class="g-ring" :class="object.engine">
          <line :x1="PAD.l" :y1="geom.ringTop" :x2="W - PAD.r" :y2="geom.ringTop" class="edge" />
          <path :d="ringPath" class="line" />
          <circle v-for="p in bars.filter((q) => q.ring != null)" :key="`rd${p.key}`" :cx="p.x" :cy="yRing(p.ring)" r="1.5" />
          <text :x="W - PAD.r - 4" :y="geom.ringTop + 11" text-anchor="end" class="lane">ring time</text>
        </g>
      </svg>

      <div v-if="!modes.live" class="md__empty">
        No <code>modes</code> stream in this build. The panel draws what the engine publishes and computes none of it.
      </div>
      <div v-if="settling" class="md__settling">still building the mode table · {{ Math.round(info.build * 100) }}%</div>
      <!--
        The stamp names the half that is invented, and it got narrower rather
        than softer. The ratios on this axis are the engine's own table now,
        read out of `benchmark --dump series`, so calling the whole picture the
        page's arithmetic was overclaiming in the direction of caution — which
        sounds harmless and is not, because a warning that is loose about what
        it warns of is one a reader learns to discount. **Every level here is
        still invented**, and that is the sentence a screenshot has to carry.
      -->
      <div v-else-if="designMode" class="md__stamp">
        design mode · every level here is the page’s own arithmetic · the ratios are the engine’s table
      </div>

      <ModeEditor v-if="pickedPartial" :partial="pickedPartial" @close="picked = null" />
    </div>

    <div v-if="showLegend" class="md__legend">
      <span class="k"><i :style="{ background: guide ? 'var(--res-guide)' : 'var(--res-modal)' }" />{{ guide ? 'the loop’s response' : 'left channel' }}</span>
      <span v-if="!guide" class="k"><i style="background: var(--res-ink); opacity: 0.55" />right channel</span>
      <span v-if="modes.hasBare" class="k"><i style="background: var(--res-null)" />taken by a node</span>
      <span v-if="fused.length" class="k drop-short"><i style="background: color-mix(in srgb, var(--res-modal) 45%, transparent)" />fused into timbre</span>
      <span v-if="!guide && band" class="k drop-short"><i style="background: color-mix(in srgb, var(--res-modal) 30%, transparent)" />how wide each resonance is</span>
      <span v-if="!geom.hasRing" class="k" style="color: var(--res-faint)">ring lane hidden · the window is too short</span>
      <span v-if="!fundamental.fromEngine" class="k drop-short" style="color: var(--res-faint)">axis on the Tune control</span>
      <span class="md__note">Click a partial to set its pitch, level and ring time.</span>
    </div>

    <p v-if="showProv && missing.length" class="md__prov" :class="{ 'is-fault': engineDisagrees }">
      {{ missing.join(' · ') }}.
    </p>
    <p v-if="showProv && readCarefully.length" class="md__prov is-note">
      {{ readCarefully.join(' · ') }}.
    </p>
    <p v-if="showProv && !missing.length && !readCarefully.length && notApplicable.length" class="md__prov is-fine">
      {{ notApplicable.join(' · ') }}.
    </p>
  </section>
</template>

<style scoped>
/*
 * Everything that is drawn *about* the series rather than being the series
 * takes no pointer events. The crossover rule sits exactly on top of a
 * partial by construction — it is drawn at one — and it was swallowing the
 * click meant for that bar, so the partials nearest the one line the eye is
 * drawn to were the ones that could not be selected.
 */
.g-grid, .g-axis, .g-ghostline, .g-cross, .g-cut, .g-held, .g-ring, .g-dead,
.g-fused, .g-fused-floor, .g-lost, .g-right, .g-curve { pointer-events: none; }

.g-grid line { stroke: rgb(255 255 255 / 0.055); stroke-width: 1; }
.g-grid line.zero { stroke: rgb(255 255 255 / 0.12); }
.g-grid line.root { stroke: rgb(255 255 255 / 0.16); }
.g-axis text { font-size: 8.5px; fill: rgb(220 227 236 / 0.34); font-variant-numeric: tabular-nums; }
.g-axis text.ratio { fill: rgb(220 227 236 / 0.5); }
.g-axis text.hz, .g-axis text.soft { fill: rgb(220 227 236 / 0.24); font-size: 8px; }

.g-ghostline line { stroke: rgb(220 227 236 / 0.18); stroke-width: 1; stroke-dasharray: 2 3; }

.g-bar rect { fill: var(--res-modal); cursor: pointer; }
.g-bar rect:hover { fill: #fff; }
.g-bar rect.edited { fill: var(--res-brass); }
/* Where the stroke has been, before it is committed on release. */
.g-pending rect { fill: color-mix(in srgb, var(--res-brass) 14%, transparent); }
svg.is-drawing { cursor: crosshair; }
svg.is-drawing .g-bar rect, svg.is-drawing .g-handles line { cursor: crosshair; }
.g-bar rect.picked { fill: #fff; }
.g-right line { stroke: var(--res-ink); stroke-width: 1.3; opacity: 0.55; }
.g-lost rect { fill: var(--res-null); opacity: 0.34; }
.g-dead circle { fill: var(--res-null); }

.g-fused { fill: color-mix(in srgb, var(--res-modal) 22%, transparent); stroke: none; }
.g-fused-floor { fill: color-mix(in srgb, var(--res-modal) 6%, transparent); stroke: none; }

.g-curve .fill { fill: color-mix(in srgb, var(--res-guide) 20%, transparent); }
/*
 * The bank's response sits behind its own bars and must stay behind them:
 * the bars are where the partials are, which is the argument, and this is the
 * supporting detail of how sharp each one is.
 */
.g-behind { pointer-events: none; }
.g-behind .fill { fill: color-mix(in srgb, var(--res-modal) 10%, transparent); }
.g-behind .line { fill: none; stroke: color-mix(in srgb, var(--res-modal) 34%, transparent); stroke-width: 1; stroke-linejoin: round; }
.g-curve .line { fill: none; stroke: var(--res-guide); stroke-width: 1.3; stroke-linejoin: round; }

.g-handles line { stroke: rgb(220 227 236 / 0.3); stroke-width: 2.4; cursor: pointer; }
.g-handles line.guide { stroke: var(--res-guide); opacity: 0.8; }
.g-handles line:hover { stroke: #fff; }
.g-handles line.edited { stroke: var(--res-brass); opacity: 1; }
.g-handles line.picked { stroke: #fff; }

.g-cross line { stroke: rgb(220 227 236 / 0.28); stroke-width: 1; stroke-dasharray: 1 4; }
.g-cross text { font-size: 8.5px; fill: rgb(220 227 236 / 0.38); }

/*
 * Partials the engine holds at the ceiling. Several land on one pixel and read
 * as a single loud partial at the top of the series, which is the opposite of
 * what they are — so the stack is dashed, counted, and the bars themselves say
 * they are held rather than drawing as ordinary partials.
 */
.g-held line { stroke: var(--res-guide); stroke-width: 1.2; stroke-dasharray: 2 3; }
.g-held text { font-size: 9px; fill: var(--res-guide); }
.g-bars line.held, .g-handles line.held { stroke-dasharray: 2 2; opacity: 0.5; }

.g-cut rect { fill: rgb(11 14 18 / 0.66); }
.g-cut line { stroke: var(--res-warn); stroke-width: 1; }
.g-cut text { font-size: 8.5px; fill: var(--res-warn); letter-spacing: 0.04em; }
.g-cut text.sub { fill: color-mix(in srgb, var(--res-warn) 62%, transparent); }

.g-ring .edge { stroke: rgb(255 255 255 / 0.07); stroke-width: 1; }
.g-ring .line { fill: none; stroke-width: 1.4; stroke-linejoin: round; }
.g-ring.modal .line { stroke: var(--res-modal); }
.g-ring.waveguide .line { stroke: var(--res-guide); }
.g-ring circle { fill: var(--res-ink); opacity: 0.4; }
.g-ring text.lane { font-size: 8px; fill: rgb(220 227 236 / 0.3); letter-spacing: 0.1em; text-transform: uppercase; }
</style>
