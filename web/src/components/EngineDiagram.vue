<script setup>
/**
 * The two engines, drawn.
 *
 * **The prose was not working.** This panel explained the mode bank and the
 * waveguide in words, in several places, at length — and a reader who had
 * read all of it still could not say what the relationship between them was.
 * That is a failure of the explanation, not of the reader: "the object itself
 * vibrates" and "the object is only a boundary" are true sentences that do
 * not tell you what the *machine* is doing differently.
 *
 * So it is a picture, and it makes exactly one point: **these are two ways of
 * building the same thing, and they cost differently.**
 *
 * A mode bank is a stack of resonators, one per partial, each one paid for.
 * Ask for more partials and you buy more filters — which is why Modes is a
 * control on a mode bank and why the object can run out.
 *
 * A waveguide is one delay loop with a reflection at each end. Its partials
 * are not built, they *fall out* of the round trip, all of them, for the same
 * price — which is why Modes is greyed on an air column and why it can never
 * run out.
 *
 * Props: `engine` (`'modal'` or `'waveguide'`).
 */
defineProps({ engine: { type: String, required: true } });

/** Three resonators read as a stack at this size; four ran together into a smear. The ellipsis carries the rest. */
const ROWS = [0, 1, 2];
</script>

<template>
  <svg class="ed" viewBox="0 0 158 50" role="img" :aria-label="engine === 'modal' ? 'a stack of resonators, one per partial' : 'one delay loop with a reflection at each end'">
    <template v-if="engine === 'modal'">
      <!-- in -->
      <line class="ed__wire" x1="4" y1="21" x2="20" y2="21" />
      <!-- one resonator per partial, each its own box -->
      <g class="ed__box">
        <rect v-for="r in ROWS" :key="r" x="24" :y="3 + r * 13" width="34" height="9" rx="2" />
      </g>
      <g class="ed__wire">
        <line v-for="r in ROWS" :key="`a${r}`" x1="20" y1="21" x2="24" :y2="7.5 + r * 13" />
        <line v-for="r in ROWS" :key="`b${r}`" x1="58" :y1="7.5 + r * 13" x2="66" y2="21" />
      </g>
      <text class="ed__dots" x="41" y="49">· · ·</text>
      <line class="ed__wire" x1="66" y1="21" x2="80" y2="21" />
      <text class="ed__cost" x="86" y="18">one filter</text>
      <text class="ed__cost" x="86" y="28">per partial</text>
    </template>

    <template v-else>
      <line class="ed__wire" x1="4" y1="23" x2="22" y2="23" />
      <!-- the loop: two delay lines, a reflection at each end -->
      <path class="ed__loop" d="M 26 14 L 74 14 M 26 32 L 74 32" />
      <path class="ed__arrow" d="M 46 11 l 6 3 l -6 3 Z" />
      <path class="ed__arrow" d="M 54 29 l -6 3 l 6 3 Z" />
      <g class="ed__mirror">
        <line x1="24" y1="8" x2="24" y2="38" />
        <line x1="76" y1="8" x2="76" y2="38" />
      </g>
      <line class="ed__wire" x1="76" y1="23" x2="80" y2="23" />
      <text class="ed__cost" x="86" y="20">one loop,</text>
      <text class="ed__cost" x="86" y="30">every partial</text>
    </template>
  </svg>
</template>

<style scoped>
.ed { display: block; width: 158px; height: 50px; flex: 0 0 auto; }
.ed__wire { stroke: currentColor; stroke-width: 1; opacity: 0.55; }
.ed__box rect { fill: none; stroke: currentColor; stroke-width: 1.2; }
.ed__loop { stroke: currentColor; stroke-width: 1.4; fill: none; }
.ed__arrow { fill: currentColor; }
.ed__mirror line { stroke: currentColor; stroke-width: 2.4; }
.ed__dots { fill: currentColor; font-size: 8px; opacity: 0.5; text-anchor: middle; }
.ed__cost { fill: currentColor; font-size: 8.5px; opacity: 0.75; }
</style>
