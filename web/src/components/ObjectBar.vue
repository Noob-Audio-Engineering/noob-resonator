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
 * **The source line is not decoration.** Nine of the ten series are the
 * solution of a stated closed form, and the numbers on the display come off
 * the engine that solved them. The marimba is the exception and is marked as
 * one, in the warning colour, as a tuning target: an undercut bar is arrived
 * at by cutting until the partials land, and no bare equation gives them.
 */
import { computed } from 'vue';
import { forcesOf, inactive, noteOf, ui, useMetaDrift, useObject, useObjectMeta, useRes } from '../composables/useResonator.js';
import { useChords } from '../composables/useChords.js';

const r = useRes();
const object = useObject();
const meta = useObjectMeta();

/**
 * Controls the engine's object table names that this build does not publish.
 *
 * Printed rather than swallowed. While this says anything at all the panel
 * greys nothing out, because a list that names a control which does not exist
 * cannot be trusted about the controls that do — and the alternative, which
 * happened, is the device's headline knob quietly dead in the host and alive
 * everywhere it was tested.
 */
const drift = useMetaDrift();
const chords = useChords();

/**
 * How the object is tuned, said beside what it is made of.
 *
 * **Both are true and neither replaces the other.** An object answers *what is
 * it made of*; the voices answer *what is it tuned to*. A chord of beams is
 * still a beam — every voice gets this object's own series — so the bar states
 * the two facts side by side rather than letting one stand in for the other.
 *
 * `null` on an object that has no voices, which is the engine's call and not
 * this file's: the two-dimensional objects do not offer them yet, and `uses`
 * is what says so.
 */
/**
 * Whether this object has voices at all, which is the engine's call.
 *
 * **Separate from whether there is anything to say about them.** Tying the way
 * in to the statement made the key vanish at one voice along with the words,
 * and then the deck was the only route to the chord menu — a feature you can
 * only reach if you already know it is there. The bar stays silent at one
 * voice; the door stays open.
 */
const canVoice = computed(() => chords.has && !inactive('voices', object.value, meta.value));

const voicing = computed(() => {
  if (!canVoice.value) return null;
  const n = chords.sounding;
  // **Nothing at one voice.** One voice is the object at its own pitch, which
  // is what it was before voices existed — and a line reading "one voice" is
  // the panel announcing a feature rather than stating a fact about the sound.
  // A user who never turns Voices up should not be able to tell it is there.
  // The way in stays, because a feature nobody can reach is not one.
  return n > 1 ? `${n} voices · ${chords.label}` : null;
});

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
  // An object whose far end the engine pins cannot disagree with its name.
  if (forcesOf(meta.value)?.opening != null) return null;
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
        <!--
          What it is made of, and what it is tuned to. Two facts, not one
          replacing the other: every voice gets this object's own series, so a
          chord of beams is still beams.
        -->
        <span v-if="voicing" class="pick__voicing">{{ voicing }}</span>
      </div>

      <button class="key pick__change" type="button" @click="ui.browsing = true">Change resonator</button>
      <button v-if="canVoice" class="key pick__tune" type="button" @click="ui.chords = true">Tune the voices</button>

      <div class="pick__say">
        <p class="pick__blurb">{{ object.blurb }}</p>
        <p class="pick__src">
          <b :class="{ target: object.derivation === 'tuning target' }">{{ object.derivation }}</b>
          · {{ object.source }}
        </p>
        <p v-if="noteOf(meta)" class="pick__src pick__note">{{ noteOf(meta) }}</p>
        <p v-if="caveat" class="pick__src pick__caveat">{{ caveat }}</p>
        <p v-if="drift.length" class="pick__src pick__caveat">
          The engine’s object table names {{ drift.length === 1 ? 'a control' : 'controls' }} this build does not
          publish — {{ drift.join(', ') }} — so it is out of date and nothing on this panel is greyed out from it.
        </p>
      </div>
    </div>
  </section>
</template>
