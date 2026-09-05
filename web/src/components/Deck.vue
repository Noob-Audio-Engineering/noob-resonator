<script setup>
/**
 * Every control except the headline, in four groups that follow the physical
 * story rather than the signal: what is ringing, how it stops ringing, where
 * you hit it and where you listen, and what comes out. Select and Modes are
 * not here — they are the argument, and they live above the display.
 *
 * **Several controls carry a second line, and that is a design position
 * rather than decoration.** A control the reader cannot reason about is a
 * defect, so Decay is seconds and Brightness is decibels per octave on the
 * face, and Material says what its setting is costing the top of the series —
 * a figure read off the published ring time, not derived here.
 *
 * **What the controls no longer print is which partial they are killing.**
 * That needed each contact point's weight on each mode, which is not on the
 * wire and would have to be worked out here — and the panel computes nothing.
 * The display still shows the nulls, from the engine's own `db_bare`, which
 * is the half that matters.
 *
 * **A greyed control is greyed because the engine's own table says so.** The
 * `uses` list in the manifest meta is the truth and the page derives nothing;
 * what the page adds is the sentence saying why, because a greyed control
 * with no explanation is the thing this panel exists to improve on.
 */
import { computed } from 'vue';
import { Segmented, Toggle } from '@noob-audio-engineering/noob-vst-webgui-framework/vue';
import ResKnob from './ResKnob.vue';
import { contactAxes, coordsOf, inactive, timeText, useInfo, useModes, useObject, useObjectTable, useRes } from '../composables/useResonator.js';

const r = useRes();
const object = useObject();
const modes = useModes();
const info = useInfo();
const table = useObjectTable();

const off = (id) => inactive(id, object.value, table.value);

/**
 * Material states what its exponent is costing the top of the series, read
 * off the published ring time rather than worked out here.
 *
 * A percentage on a knob says nothing about what the device is doing; "the
 * top partial rings for 18 ms" says all of it. The figure is the engine's.
 */
const materialHint = computed(() => {
  const l = modes.list;
  const top = l.length ? l[l.length - 1] : null;
  return top && top.ring != null ? `top rings ${timeText(top.ring)}` : 'damping against frequency';
});
const openingHint = computed(() => {
  if (!r.opening) return 'the far end';
  const o = r.opening.plain / 100;
  if (o < 0.02) return 'stopped · odd only';
  if (o > 0.98) return 'open · full series';
  return 'part open · sliding';
});
const barOff = computed(() => off('bar_tuning'));

/**
 * What the contact controls are called on this object.
 *
 * A bar has one coordinate, a rectangle has two, and **a disc has a radius
 * and an angle** — which is a different thing to aim, not a different label
 * for the same thing. Calling the second axis "Y" on a drum head would be
 * telling the user to think in squares about a circle, which is precisely the
 * mistake that once let a strike land in a corner that does not exist.
 */
const axes = computed(() => contactAxes(coordsOf(object.value, table.value)));
const ax = (base, which) => {
  const a = axes.value[which];
  return a.suffix ? `${base} ${a.suffix}` : base;
};
</script>

<template>
  <div class="deck">
    <section class="deck__group plate">
      <h3 class="cap deck__head">Body<span class="why">what is ringing</span></h3>
      <div class="deck__row">
        <ResKnob v-if="r.tune" :p="r.tune" label="Tune" :size="56" />
        <ResKnob v-if="r.transpose" :p="r.transpose" label="Transpose" :size="44" hint="semitones" />
        <ResKnob v-if="r.fine" :p="r.fine" label="Fine" :size="42" />
        <ResKnob v-if="r.ratio" :p="r.ratio" label="Ratio" :size="44" :off="off('ratio')" hint="rectangle aspect" />
        <ResKnob v-if="r.radius" :p="r.radius" label="Radius" :size="44" :off="off('radius')" hint="the bore" />
        <ResKnob v-if="r.opening" :p="r.opening" label="Opening" :size="48" :off="off('opening')" :hint="openingHint" />
        <!--
          The two decisions a bar maker actually makes. The second partial is
          cut to 4 for a marimba and 3 for a xylophone; the third is quoted at
          both 9.2 and 10 because it depends on how deep the arch is. Neither
          is a discrepancy to average away, so both are here.
        -->
        <div v-if="r.barTuning || r.barThird" class="deck__stack" :class="{ 'is-off': !!barOff }" :title="barOff ? barOff.why : 'How the bar was cut'">
          <span class="deck__cap">Bar</span>
          <Segmented v-if="r.barTuning" :p="r.barTuning" class="keys keys--tiny" />
          <Segmented v-if="r.barThird" :p="r.barThird" class="keys keys--tiny" />
          <span class="deck__hint">{{ barOff ? barOff.short : 'the maker’s two choices' }}</span>
        </div>
      </div>
    </section>

    <section class="deck__group plate">
      <h3 class="cap deck__head">Damping<span class="why">how it stops</span></h3>
      <div class="deck__row">
        <ResKnob v-if="r.decay" :p="r.decay" label="Decay" :size="52" hint="the fundamental’s ring time" />
        <ResKnob v-if="r.material" :p="r.material" label="Material" :size="52" :off="off('material')" :hint="materialHint" />
        <ResKnob v-if="r.bright" :p="r.bright" label="Bright" :size="44" :off="off('bright')" hint="tilt about the fundamental" />
        <ResKnob v-if="r.inharm" :p="r.inharm" label="Inharm" :size="44" :off="off('inharm')" hint="stretches the series" />
        <!--
          Material on its own is a one-parameter loss law. These two are the
          other half of it: where the extra damping starts, and how steeply it
          bites above that. Ableton's device has one knob here and no way to
          say which of the two things it is doing.
        -->
        <ResKnob v-if="r.dampCorner" :p="r.dampCorner" label="Damp Corner" :size="44" :off="off('damp_corner')" hint="where extra loss starts" />
        <ResKnob v-if="r.dampHi" :p="r.dampHi" label="HF Slope" :size="42" :off="off('damp_hi')" hint="how hard it bites above it" />
        <div v-if="r.tail" class="deck__stack">
          <span class="deck__cap">Tail</span>
          <Toggle :p="r.tail" variant="button" class="keys keys--lamp">on</Toggle>
          <span class="deck__hint">let it ring out</span>
        </div>
      </div>
    </section>

    <!--
      The exciter filter. It shapes the strike rather than the object, which
      is why it is its own group and not part of Damping — and Filter Place
      says which side of the resonator it sits on, because Ableton document
      theirs in one place and put it in another.
    -->
    <section class="deck__group plate">
      <h3 class="cap deck__head">Exciter<span class="why">what goes in</span></h3>
      <div class="deck__row">
        <div v-if="r.filterOn" class="deck__stack">
          <span class="deck__cap">Filter</span>
          <Toggle :p="r.filterOn" variant="button" class="keys keys--lamp">on</Toggle>
          <Segmented v-if="r.filterPlace" :p="r.filterPlace" class="keys keys--tiny" />
        </div>
        <ResKnob v-if="r.filterFreq" :p="r.filterFreq" label="Freq" :size="46" :off="off('filter_freq')" />
        <ResKnob v-if="r.filterWidth" :p="r.filterWidth" label="Width" :size="44" :off="off('filter_width')" hint="octaves, stated" />
      </div>
    </section>

    <section class="deck__group plate">
      <h3 class="cap deck__head">LFO<span class="why">what moves the pitch</span></h3>
      <div class="deck__row">
        <div v-if="r.lfoOn" class="deck__stack">
          <span class="deck__cap">LFO</span>
          <Toggle :p="r.lfoOn" variant="button" class="keys keys--lamp">on</Toggle>
          <span class="deck__hint">{{ r.lfoShape ? r.lfoShape.label : '' }}</span>
        </div>
        <ResKnob v-if="r.lfoRate" :p="r.lfoRate" label="Rate" :size="44" :off="off('lfo_rate')" />
        <ResKnob v-if="r.lfoDepth" :p="r.lfoDepth" label="Depth" :size="44" :off="off('lfo_depth')" hint="semitones, not an amount" />
        <ResKnob v-if="r.lfoPhase" :p="r.lfoPhase" label="Phase" :size="42" :off="off('lfo_phase')" hint="between the channels" />
      </div>
    </section>

    <section class="deck__group plate">
      <h3 class="cap deck__head">Contact<span class="why">where you hit it, where you listen</span></h3>
      <div class="deck__row">
        <ResKnob v-if="r.hit" :p="r.hit" :label="ax('Hit', 'x')" :size="46" :hint="axes.x.hint" />
        <ResKnob v-if="r.hitY" :p="r.hitY" :label="ax('Hit', 'y')" :size="42" :off="off('hit_y')" :hint="axes.y.hint" />
        <ResKnob v-if="r.posL" :p="r.posL" :label="ax('Pos L', 'x')" :size="44" hint="left pickup" />
        <ResKnob v-if="r.posLY" :p="r.posLY" :label="ax('Pos L', 'y')" :size="42" :off="off('pos_l_y')" :hint="axes.y.hint" />
        <ResKnob v-if="r.posR" :p="r.posR" :label="ax('Pos R', 'x')" :size="44" hint="right pickup" />
        <ResKnob v-if="r.posRY" :p="r.posRY" :label="ax('Pos R', 'y')" :size="42" :off="off('pos_r_y')" :hint="axes.y.hint" />
        <ResKnob v-if="r.spread" :p="r.spread" label="Spread" :size="42" hint="detunes the channels" />
        <ResKnob v-if="r.width" :p="r.width" label="Width" :size="42" hint="pans the pickups" />
      </div>
    </section>

    <section class="deck__group plate">
      <h3 class="cap deck__head">Out<span class="why">what leaves</span></h3>
      <div class="deck__row">
        <!--
          Bleed is the one control on this panel with an argument attached.
          Ableton's own manual offers the dry input as the remedy for a bank
          that has run out of modes, which makes it a patch over a hole rather
          than a creative control. Ours says so on its face.
        -->
        <ResKnob v-if="r.bleed" :p="r.bleed" label="Bleed" :size="46" hint="dry back, for what the bank lost" />
        <ResKnob v-if="r.mix" :p="r.mix" label="Dry/Wet" :size="48" />
        <ResKnob v-if="r.gain" :p="r.gain" label="Gain" :size="44" />
        <!--
          Optional and zero-latency, so it is a choice rather than a thing
          done to you. The bench shows what it is actually taking off.
        -->
        <div v-if="r.limiter" class="deck__stack">
          <span class="deck__cap">Limiter</span>
          <Toggle :p="r.limiter" variant="button" class="keys keys--lamp">on</Toggle>
          <span class="deck__hint">zero latency</span>
        </div>
        <ResKnob v-if="r.limitCeil" :p="r.limitCeil" label="Ceiling" :size="42" :off="off('limit_ceil')" />
        <div v-if="r.bypass" class="deck__stack">
          <span class="deck__cap">Bypass</span>
          <Toggle :p="r.bypass" variant="button" class="keys keys--lamp">on</Toggle>
        </div>
      </div>
    </section>
  </div>
</template>
