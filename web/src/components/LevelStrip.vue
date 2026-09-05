<script setup>
/**
 * In, out, and what the limiter is taking off.
 *
 * **A resonator needs this more than most effects do.** A long decay and a
 * large mode budget is a bank of resonators being fed continuously; get the
 * settings wrong and the output goes on climbing after the input has stopped.
 * The limiter catches it, is on by default and is optional — so the number
 * that actually matters is how hard it is working, and a user who cannot see
 * that has no way to tell a device that is behaving from one that is only
 * being rescued.
 *
 * **It lives in the top bar and not in the deck**, which is a size decision as
 * much as a design one: the deck scrolls at small windows, and a level readout
 * that can scroll out of sight is not a level readout. This one is on screen
 * whatever else is.
 *
 * **It is deliberately outside the panel's accent scheme.** Teal, amber and
 * rose each mean one thing about the physics here; a meter means none of
 * them, so it is drawn in plain ink like the knobs are, and the only colour
 * on it is the warning yellow at the top of the scale and on the clip lamp.
 *
 * The peak hold decays, because a bar that only ever rises is unreadable. The
 * clip lamp latches, because a clip you missed is the one worth knowing
 * about; clicking it clears it.
 */
import { computed } from 'vue';
import { linToDb, useInfo, useMeter } from '../composables/useResonator.js';

const meter = useMeter();
const info = useInfo();

/** The scale, in decibels. Six above unity, because a runaway resonator goes there. */
const MIN_DB = -48;
const MAX_DB = 6;
const pos = (v) => {
  const db = linToDb(v);
  return Math.max(0, Math.min(1, (db - MIN_DB) / (MAX_DB - MIN_DB))) * 100;
};
/** Where unity sits on the scale, so the bar has a line to be over or under. */
const unity = ((0 - MIN_DB) / (MAX_DB - MIN_DB)) * 100;

const bars = computed(() => [
  { k: 'in', label: 'in', l: pos(meter.held.in_l), r: pos(meter.held.in_r) },
  { k: 'out', label: 'out', l: pos(meter.held.out_l), r: pos(meter.held.out_r) },
]);

/**
 * The limiter's gain reduction, when the engine has one to report.
 *
 * **Zero is not "working" and absent is not zero.** The engine publishes NaN
 * for this while the limiter is off, so the readout disappears rather than
 * sitting at `0.0 dB GR` — which is what it did when the field was zero-filled,
 * reporting a measurement nothing had made. Hidden rather than explained is
 * the right shape here: the strip is four characters wide and a missing bar
 * beside a live one says everything a sentence would.
 */
const gr = computed(() => (info.limitGrDb == null ? null : Math.abs(info.limitGrDb)));
</script>

<template>
  <!--
    Dimmed and still when no frames are arriving, and never faked. Design mode
    publishes exactly what the engine publishes and invents no levels, so an
    unfed meter reads as unfed rather than as silence.
  -->
  <div
    v-if="meter.has"
    class="lvl"
    :class="{ 'is-dead': !meter.live }"
    :title="meter.live ? null : 'No meter frames. The engine publishes these; the page does not invent levels.'"
  >
    <div v-for="b in bars" :key="b.k" class="lvl__pair">
      <span class="lvl__cap">{{ b.label }}</span>
      <div class="lvl__bars">
        <div class="lvl__track"><i :style="{ width: b.l + '%' }" /></div>
        <div class="lvl__track"><i :style="{ width: b.r + '%' }" /></div>
        <span class="lvl__unity" :style="{ left: unity + '%' }" />
      </div>
    </div>

    <button
      class="lvl__clip"
      :class="{ on: meter.clipped }"
      type="button"
      :title="meter.clipped ? 'Output reached full scale. Click to clear.' : 'No clip since this was last cleared'"
      @click="meter.clear()"
    >
      clip
    </button>

    <span v-if="gr != null" class="lvl__gr tabular" :class="{ working: gr > 0.1 }" title="What the limiter is taking off">
      {{ gr > 0.05 ? `−${gr.toFixed(1)}` : '0.0' }} <i>dB GR</i>
    </span>
  </div>
</template>

<style scoped>
.lvl { display: flex; align-items: center; gap: 9px; }
.lvl.is-dead { opacity: 0.4; }
.lvl__pair { display: flex; align-items: center; gap: 5px; }
.lvl__cap { font-size: 8.5px; letter-spacing: 0.1em; text-transform: uppercase; color: var(--res-faint); }
.lvl__bars { position: relative; display: flex; flex-direction: column; gap: 2px; width: 62px; }
.lvl__track { position: relative; height: 4px; border-radius: 2px; background: rgb(255 255 255 / 0.07); overflow: hidden; }
.lvl__track i {
  display: block; height: 100%; border-radius: 2px;
  /* Plain ink up the scale, and the warning colour only where it matters. */
  background: linear-gradient(90deg, rgb(220 227 236 / 0.55) 0%, rgb(220 227 236 / 0.7) 78%, var(--res-warn) 100%);
}
.lvl__unity { position: absolute; top: -1px; bottom: -1px; width: 1px; background: rgb(255 255 255 / 0.25); }

.lvl__clip {
  font: inherit; font-size: 8.5px; letter-spacing: 0.1em; text-transform: uppercase;
  padding: 3px 6px; border-radius: 3px; cursor: pointer;
  border: 1px solid var(--res-line); background: var(--res-plate); color: var(--res-faint);
}
.lvl__clip.on { background: var(--res-warn); border-color: var(--res-warn); color: var(--res-ground); font-weight: 700; }

.lvl__gr { font-size: 10px; color: var(--res-faint); white-space: nowrap; }
.lvl__gr.working { color: var(--res-ink); }
.lvl__gr i { font-style: normal; color: var(--res-faint); }
</style>
