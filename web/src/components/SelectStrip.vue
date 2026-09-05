<script setup>
/**
 * How the bank's mode budget is spent — the headline, and this device's
 * equivalent of the sibling plug-in's alias readout.
 *
 * **A bank with more partials available than resonators to run them has to
 * choose, and the obvious choice is the wrong one.** Taking the lowest is
 * what a plain mode count does implicitly, and it is what Ableton's quality
 * setting does by their own description — Applied Acoustics publish the
 * ladder as 4 / 16 / 30 / 70 modes. At a high fundamental that is fine; at a
 * low one it is a wall inside the audio band. A seventy-mode string is only
 * complete above about 286 Hz, and at 55 Hz it stops dead at 3.85 kHz with
 * nothing above it at all. Ableton document the consequence themselves and
 * the remedy their manual offers is to mix the dry input back in, which is
 * what Bleed is: a patch over a hole rather than a creative control.
 *
 * Taking the loudest keeps the partials you can actually hear, wherever they
 * sit. Both are here so the difference can be heard rather than argued.
 *
 * **Three numbers, three different things, and the strip keeps them apart.**
 * What the object *has* is a fact about the object. What the bank *runs* is
 * the budget spent by Select, and the top of that is a wall you can hear.
 * What the display *draws* is the sixty-four the stream carries, which is a
 * limit on a picture and nothing more. Running the last two together had the
 * panel announce a wall for a display feed running out, which would have been
 * false in exactly the way the rest of this page exists to avoid.
 *
 * Every figure here comes off the `info` stream. A build that does not
 * publish one leaves the strip saying so rather than guessing.
 */
import { computed } from 'vue';
import { Segmented } from '@noob-audio-engineering/noob-vst-webgui-framework/vue';
import ResKnob from './ResKnob.vue';
import { countText, hzText, inactive, useInfo, useModes, useObject, useObjectTable, useRes } from '../composables/useResonator.js';

const r = useRes();
const object = useObject();
const modes = useModes();
const info = useInfo();
const table = useObjectTable();

const guide = computed(() => object.value.engine === 'waveguide');
const drawn = computed(() => modes.list.length);
/** Whether Select is actually choosing. With room for every partial it has nothing to do. */
const cutting = computed(
  () => !guide.value && info.used != null && info.available != null && info.used < info.available,
);
const word = computed(() => {
  const l = r.select ? r.select.label : 'Loudest';
  return l === 'Lowest' ? 'lowest' : l === 'Log Spread' ? 'spread across the range' : 'loudest';
});
const wall = computed(() => (info.ceilingHz && info.ceilingHz > 0 ? info.ceilingHz : null));
/**
 * Whether the engine publishes where the bank runs out.
 *
 * It does not yet, and the strip says so rather than reporting "nothing left
 * to lose" — which would be the panel claiming an all-clear it has no way to
 * check. The figure has been asked for; it is not reconstructed here.
 */
const hasCeiling = computed(() => info.ceilingHz != null);
</script>

<template>
  <section class="sel plate">
    <div class="sel__left">
      <h2 class="cap sel__cap">Select<span class="why">how the mode budget is spent</span></h2>
      <Segmented v-if="r.select" :p="r.select" class="keys keys--sel" />
    </div>

    <ResKnob
      v-if="r.modes"
      :p="r.modes"
      label="Modes"
      :size="38"
      :off="inactive('modes', object, table)"
      hint="resonators, not partials"
    />

    <div class="sel__read tabular">
      <div class="sel__row">
        <span class="sel__k">this object has</span>
        <span class="sel__v">{{ guide ? 'one loop' : countText(info.available) }}<i v-if="!guide"> partials</i></span>
      </div>
      <div class="sel__row">
        <span class="sel__k">the bank runs</span>
        <span class="sel__v">
          {{ guide ? 'all of them' : countText(info.used) }}<i v-if="drawn && info.used > drawn">, {{ drawn }} drawn</i>
        </span>
      </div>
      <div class="sel__row" :class="{ wall: !!wall }">
        <span class="sel__k">above which there is</span>
        <span class="sel__v">
          <template v-if="wall">nothing past {{ hzText(wall) }}</template>
          <template v-else-if="!hasCeiling"><i>not published</i></template>
          <template v-else>nothing left to lose</template>
        </span>
      </div>
    </div>

    <p class="sel__say">
      <template v-if="!info.has || !info.live">
        This build publishes no <code>info</code> stream, so how many partials the object has and how many the
        bank is running are not on the wire. The panel does not guess them.
      </template>
      <template v-else-if="wall">
        <b>The object is deaf above {{ hzText(wall) }}.</b>
        It has {{ countText(info.available) }} partials, the bank runs {{ countText(info.used) }}, and this
        setting gives those to the {{ word }} — so above that line there is nothing at all. Mixing the dry
        signal back with Bleed is the usual patch, and it is a patch.
      </template>
      <template v-else-if="cutting">
        The bank runs the <b>{{ word }}</b> {{ countText(info.used) }} of {{ countText(info.available) }}
        partials. Switch to Lowest to hear what a plain mode count throws away.
        <template v-if="!hasCeiling">Where that leaves the object deaf is not on the wire yet.</template>
      </template>
      <template v-else-if="guide">
        One delay loop, so nothing is being thrown away: every resonance under Nyquist comes out of it at the
        same cost.
      </template>
      <template v-else>
        The bank has room for every partial this object has, so nothing is being thrown away and Select has
        nothing to choose between. Pull Modes down to hear what happens when it does.
      </template>
    </p>
  </section>
</template>
