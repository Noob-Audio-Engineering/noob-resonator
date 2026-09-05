<script setup>
/**
 * What is loaded, and the way in to changing it.
 *
 * The seven keys that used to live here are gone: choosing an object is a
 * view now (`TypeBrowser.vue`), because this device's difference between one
 * object and the next is a partial series rather than a name, and a row of
 * keys could not show that. What is left is the statement — this is what you
 * have, this is which engine it is, and this is where its numbers come from.
 *
 * **The source line is not decoration.** Seven of the eight series are the
 * solution of a stated closed form and `test/modes.test.js` solves each one
 * from scratch. The marimba is the exception and is marked as one, in the
 * warning colour, as a tuning target: an undercut bar is arrived at by
 * cutting until the partials land, and no bare equation gives them.
 */
import { computed } from 'vue';
import { ui, useObject, useRes } from '../composables/useResonator.js';

const r = useRes();
const object = useObject();

/**
 * What the cited equation does not currently describe.
 *
 * The two air-column equations are the two ends of Opening and are exactly
 * right only there. Leaving "fₖ = (2k−1)·f₁" on the panel while the control
 * sits in the middle would be citing a source for a series that is not on the
 * screen, which is the one thing this page is not allowed to do.
 */
/**
 * When the far end is not where the name says it is.
 *
 * Choosing Pipe or Tube puts Opening at the end of its travel that makes the
 * name true, because a label the display contradicts is worse than a
 * parameter write. But the control stays a control: move it afterwards and
 * the name must not quietly flip to the other one — the object did not
 * change, its far end did. So the name stays put and this says where the far
 * end actually is, in the warning colour, so the panel and the display cannot
 * disagree in silence.
 */
const farEnd = computed(() => {
  if (object.value.engine !== 'waveguide' || !r.opening) return null;
  const o = r.opening.plain / 100;
  const stopped = object.value.id === 'pipe';
  if (stopped && o < 0.02) return null;
  if (!stopped && o > 0.98) return null;
  if (o < 0.02) return 'stopped · which is a Pipe';
  if (o > 0.98) return 'open at both ends · which is a Tube';
  return `part open · ${r.opening.text}`;
});

const caveat = computed(() => {
  if (object.value.caveat) return object.value.caveat;
  if (object.value.engine !== 'waveguide' || !r.opening) return null;
  const o = r.opening.plain / 100;
  if (o < 0.02 || o > 0.98) return null;
  return `Opening is at ${r.opening.text}, between the two, so the series on the display is neither of the closed forms — it is sliding between them.`;
});
</script>

<template>
  <section class="pick plate">
    <div class="pick__row">
      <div class="pick__who">
        <span class="cap pick__engine" :class="object.engine">
          <i class="dotmark" />{{ object.engine === 'modal' ? 'Mode bank' : 'Waveguide' }}
        </span>
        <span class="pick__name">{{ object.label }}</span>
        <span class="pick__short">{{ object.short }}</span>
        <span v-if="farEnd" class="pick__state">{{ farEnd }}</span>
      </div>

      <button class="key pick__change" type="button" @click="ui.browsing = true">Change resonator</button>

      <div class="pick__say">
        <p class="pick__blurb">{{ object.blurb }}</p>
        <p class="pick__src">
          <b :class="{ target: object.derivation === 'tuning target' }">{{ object.derivation }}</b>
          · {{ object.source }}
        </p>
        <p v-if="caveat" class="pick__src pick__caveat">{{ caveat }}</p>
      </div>
    </div>
  </section>
</template>
