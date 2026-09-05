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
  ratioShort,
  ratioText,
  timeText,
  useDesignMode,
  useFundamental,
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
const W = ref(760);
const H = ref(240);
let ro = null;
onMounted(() => {
  ro = new ResizeObserver(([e]) => {
    W.value = Math.max(240, e.contentRect.width);
    H.value = Math.max(110, e.contentRect.height);
  });
  ro.observe(box.value);
});
onBeforeUnmount(() => ro?.disconnect());

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

const bars = computed(() => {
  const edits = overrides.byIndex;
  return modes.list.map((p) => {
    const top = Math.max(p.dbL ?? DB_FLOOR, p.dbR ?? DB_FLOOR);
    return {
      ...p,
      ratio: p.hz / f0.value,
      x: xHz(p.hz),
      xBase: p.baseHz ? xHz(p.baseHz) : null,
      yBare: p.bareDb == null ? null : yLevel(p.bareDb),
      yL: yLevel(p.dbL ?? DB_FLOOR),
      yR: yLevel(p.dbR ?? DB_FLOOR),
      yTop: yLevel(top),
      edited: edits.has(p.i),
      /** Nothing measurable came out of it. */
      dead: top <= DB_FLOOR + 0.5,
      /** The engine says this partial started higher, so a node took the difference. */
      lost: p.bareDb != null && top < p.bareDb - 1.2,
      /** Above the crossover the ear fuses them, so they are drawn as a band. */
      fused: info.crossoverHz > 0 && p.hz > info.crossoverHz,
    };
  });
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

const band = computed(() => {
  const pts = response.points;
  if (!guide.value || !pts || pts.length < 8) return null;
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
const pickedPartial = computed(() => {
  if (picked.value == null) return null;
  const live = bars.value.find((p) => p.i === picked.value);
  if (live) {
    lastSeen.value = live;
    return live;
  }
  return lastSeen.value && lastSeen.value.i === picked.value ? { ...lastSeen.value, offscreen: true } : null;
});
function pick(p) {
  picked.value = picked.value === p.i ? null : p.i;
  lastSeen.value = picked.value == null ? null : p;
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
const missing = computed(() => {
  const out = [];
  if (guide.value && !response.live) {
    out.push('no response stream in this build, so the loop’s own curve is not drawn');
  }
  if (!info.live) {
    out.push('no info stream, so the ceiling and the crossover are not marked');
  } else {
    if (info.ceilingHz == null) {
      out.push('no ceiling_hz, so where the bank runs out is not marked');
    }
    if (info.crossoverHz == null) out.push('no crossover_hz, so nothing is drawn as fused');
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
  `partial ${p.i + 1} · ${ratioText(p.ratio)}× · ${hzText(p.hz)}` +
  (p.ring != null ? ` · rings ${timeText(p.ring)}` : '') +
  (p.dead ? ' · a node took it' : '') +
  (p.edited ? ' · edited' : '') +
  ' — click to edit';
</script>

<template>
  <section class="md plate">
    <div class="md__head">
      <h2 class="md__title">Where the partials land</h2>
      <span class="md__badge" :class="object.engine">
        {{ guide ? 'waveguide · one loop' : 'mode bank · one filter each' }}
      </span>
      <div class="md__facts tabular">
        <span><b>{{ bars.length }}</b> drawn</span>
        <span>top at <b>{{ top }}</b></span>
        <span v-if="overrides.count"><b>{{ overrides.count }}</b> edited</span>
        <span v-if="info.columnM > 0">air column <b>{{ lengthText(info.columnM) }}</b></span>
        <span v-if="info.loopMs > 0">loop <b>{{ info.loopMs.toFixed(2) }} ms</b></span>
      </div>
    </div>

    <div ref="box" class="md__box">
      <svg :viewBox="`0 0 ${W} ${H}`" :width="W" :height="H">
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

        <!-- the mode bank: one bar per resonator, up to where the ear stops separating them -->
        <g v-if="!guide">
          <path v-if="fusedFloor" :d="fusedFloor" class="g-fused-floor" />
          <path v-if="fusedBand" :d="fusedBand" class="g-fused" />
          <g class="g-lost">
            <rect
              v-for="p in resolved.filter((q) => q.lost)"
              :key="`gh${p.i}`"
              :x="p.x - barW / 3"
              :y="p.yBare"
              :width="Math.max(1, barW / 1.5)"
              :height="Math.max(0, p.yTop - p.yBare)"
            />
          </g>
          <g class="g-bar">
            <rect
              v-for="p in resolved"
              :key="`b${p.i}`"
              :class="{ edited: p.edited, picked: p.i === picked }"
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
            <line v-for="p in resolved" :key="`r${p.i}`" :x1="p.x - barW" :y1="p.yR" :x2="p.x + barW" :y2="p.yR" />
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
            :key="`h${p.i}`"
            :class="{ edited: p.edited, picked: p.i === picked, guide }"
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
          <circle v-for="p in bars.filter((q) => q.dead)" :key="`x${p.i}`" :cx="p.x" :cy="geom.levelBottom - 2" r="2" />
        </g>

        <g v-if="crossX && fused.length" class="g-cross">
          <line :x1="crossX" :y1="geom.levelTop" :x2="crossX" :y2="geom.levelBottom" />
          <text :x="crossX + 5" :y="geom.levelBottom - 6">above here the ear fuses them</text>
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
          <circle v-for="p in bars.filter((q) => q.ring != null)" :key="`rd${p.i}`" :cx="p.x" :cy="yRing(p.ring)" r="1.5" />
          <text :x="W - PAD.r - 4" :y="geom.ringTop + 11" text-anchor="end" class="lane">ring time</text>
        </g>
      </svg>

      <div v-if="!modes.live" class="md__empty">
        No <code>modes</code> stream in this build. The panel draws what the engine publishes and computes none of it.
      </div>
      <div v-if="settling" class="md__settling">still building the mode table · {{ Math.round(info.build * 100) }}%</div>
      <div v-else-if="designMode" class="md__stamp">
        design mode · these partials are the page’s own arithmetic, not the engine’s
      </div>

      <ModeEditor v-if="pickedPartial" :partial="pickedPartial" @close="picked = null" />
    </div>

    <div class="md__legend">
      <span class="k"><i :style="{ background: guide ? 'var(--res-guide)' : 'var(--res-modal)' }" />{{ guide ? 'the loop’s response' : 'left channel' }}</span>
      <span v-if="!guide" class="k"><i style="background: var(--res-ink); opacity: 0.55" />right channel</span>
      <span v-if="modes.hasBare" class="k"><i style="background: var(--res-null)" />taken by a node</span>
      <span v-if="fused.length" class="k drop-short"><i style="background: color-mix(in srgb, var(--res-modal) 45%, transparent)" />fused into timbre</span>
      <span v-if="!geom.hasRing" class="k" style="color: var(--res-faint)">ring lane hidden · the window is too short</span>
      <span v-if="!fundamental.fromEngine" class="k drop-short" style="color: var(--res-faint)">axis on the Tune control</span>
      <span class="md__note">Click a partial to set its pitch, level and ring time.</span>
    </div>

    <p v-if="missing.length" class="md__prov">{{ missing.join(' · ') }}.</p>
  </section>
</template>

<style scoped>
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
.g-bar rect.picked { fill: #fff; }
.g-right line { stroke: var(--res-ink); stroke-width: 1.3; opacity: 0.55; }
.g-lost rect { fill: var(--res-null); opacity: 0.34; }
.g-dead circle { fill: var(--res-null); }

.g-fused { fill: color-mix(in srgb, var(--res-modal) 22%, transparent); stroke: none; }
.g-fused-floor { fill: color-mix(in srgb, var(--res-modal) 6%, transparent); stroke: none; }

.g-curve .fill { fill: color-mix(in srgb, var(--res-guide) 20%, transparent); }
.g-curve .line { fill: none; stroke: var(--res-guide); stroke-width: 1.3; stroke-linejoin: round; }

.g-handles line { stroke: rgb(220 227 236 / 0.3); stroke-width: 2.4; cursor: pointer; }
.g-handles line.guide { stroke: var(--res-guide); opacity: 0.8; }
.g-handles line:hover { stroke: #fff; }
.g-handles line.edited { stroke: var(--res-brass); opacity: 1; }
.g-handles line.picked { stroke: #fff; }

.g-cross line { stroke: rgb(220 227 236 / 0.28); stroke-width: 1; stroke-dasharray: 1 4; }
.g-cross text { font-size: 8.5px; fill: rgb(220 227 236 / 0.38); }

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
