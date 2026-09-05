<script setup>
/**
 * One object's partial series, drawn small, as its row in the browse view.
 *
 * **This is what makes a browse view worth having here.** The sibling lab
 * previews a compressor with its real faceplate, so a thumbnail cannot drift
 * from the panel it represents. A resonator has no faceplate — a bar, a drum
 * head and a length of pipe all look like the same grey box — but it has
 * something better: the series *is* the difference between them. The browser
 * does not merely list eight names; it shows why a beam is not a string
 * before you have committed to anything.
 *
 * **It reads a table and computes nothing.** Every partial the panel draws
 * for the loaded object arrives on the `modes` stream, and a browser showing
 * eight objects at once cannot do that, because only one of them is loaded.
 * Solving eight beam equations in the front end to draw eight thumbnails is
 * exactly what this page's architecture forbids, so the ratios come from
 * `previews.js`, which is generated from the engine's own equations and
 * regenerated when they move.
 *
 * **Every preview shares one axis**, and it has to: a row scaled to its own
 * top partial would make a membrane that stops at 5× and a beam that reaches
 * 106× look like the same spread, which is the one comparison the browser
 * exists to make.
 *
 * **The heights are a drawing convention and mean nothing.** They fall gently
 * with frequency so the near partials read first and the row does not look
 * like a picket fence. Only the positions carry information here, which is
 * also the honest thing: an object's levels depend on damping and contact
 * points that the row knows nothing about.
 *
 * Props: `object` (an entry of `OBJECTS`).
 */
import { computed } from 'vue';
import { PREVIEW_RATIOS } from '../previews.js';

const props = defineProps({ object: { type: Object, required: true } });

const W = 300;
const H = 46;
const R_MIN = 0.82;
/** Six octaves, so the dense objects fill the left and the sparse ones run off the end. */
const R_MAX = 64;

const x = (r) => (Math.log(Math.min(R_MAX, Math.max(R_MIN, r)) / R_MIN) / Math.log(R_MAX / R_MIN)) * (W - 4) + 2;

const bars = computed(() =>
  (PREVIEW_RATIOS[props.object.id] || [])
    .filter((r) => r <= R_MAX)
    .map((r, i) => ({ i, x: x(r), h: (H - 8) * (1 - 0.5 * (Math.log(r) / Math.log(R_MAX))) })),
);

const octaves = (() => {
  const out = [];
  for (let r = 1; r <= R_MAX; r *= 2) out.push(x(r));
  return out;
})();
</script>

<template>
  <svg class="prev" :viewBox="`0 0 ${W} ${H}`" preserveAspectRatio="none" aria-hidden="true">
    <line v-for="(gx, i) in octaves" :key="`o${i}`" :x1="gx" y1="0" :x2="gx" :y2="H" class="prev__grid" :class="{ root: i === 0 }" />
    <rect
      v-for="b in bars"
      :key="b.i"
      :x="b.x - 1"
      :y="H - 4 - b.h"
      width="2"
      :height="Math.max(1, b.h)"
      class="prev__bar"
      :class="object.engine"
    />
    <line x1="0" :y1="H - 4" :x2="W" :y2="H - 4" class="prev__base" />
  </svg>
</template>

<style scoped>
.prev { display: block; width: 100%; height: 100%; }
.prev__grid { stroke: rgb(255 255 255 / 0.06); stroke-width: 1; vector-effect: non-scaling-stroke; }
.prev__grid.root { stroke: rgb(255 255 255 / 0.16); }
.prev__base { stroke: rgb(255 255 255 / 0.1); stroke-width: 1; vector-effect: non-scaling-stroke; }
.prev__bar { vector-effect: non-scaling-stroke; }
.prev__bar.modal { fill: var(--res-modal); }
.prev__bar.waveguide { fill: var(--res-guide); }
</style>
