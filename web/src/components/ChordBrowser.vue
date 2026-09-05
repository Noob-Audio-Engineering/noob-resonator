<script setup>
/**
 * The chord menu: a second way to fill the mode table, one row per chord.
 *
 * **It writes the voice pitches and nothing else.** The series stays the
 * object's own, so picking Major on a Beam gives three beams a fifth and a
 * tenth apart rather than replacing the beam with something else. That is the
 * whole feature: an object is safe about timbre and dangerous about pitch, a
 * set of tuned voices is the reverse, and this is how one plug-in is both.
 *
 * **Nothing here computes an interval.** The semitones arrive from the engine
 * in `meta.chords`; this draws them and writes them through the ordinary edit
 * path, so the host records real gestures and undo reaches them.
 *
 * **Which chord is showing as loaded is derived from the live pitches**, not
 * remembered — so nudging a voice afterwards reads as *custom* immediately,
 * with no state to go stale. That is generate-then-edit made visible for free.
 */
import { onBeforeUnmount, onMounted } from 'vue';
import { hzText, ui, useObject, useRes } from '../composables/useResonator.js';
import { noteName, useChords } from '../composables/useChords.js';

const chords = useChords();
const object = useObject();
const r = useRes();

/** The root every voice is an offset from: the object's own fundamental. */
const rootHz = () => (r.tune ? r.tune.plain * 2 ** ((r.transpose?.plain || 0) / 12 + (r.fine?.plain || 0) / 1200) : 0);

const close = () => (ui.chords = false);

function onKey(e) {
  if (e.key !== 'Escape') return;
  e.preventDefault();
  close();
}
onMounted(() => window.addEventListener('keydown', onKey));
onBeforeUnmount(() => window.removeEventListener('keydown', onKey));
</script>

<template>
  <main class="browse" role="region" aria-label="Chord">
    <header class="browse__head">
      <div>
        <h2 class="browse__title">Tune the voices</h2>
        <p class="browse__lede">
          Each voice is a root, and every root gets <b>{{ object.label }}</b
          >’s own series — so this tunes the object rather than replacing it. Picking a chord writes the
          pitches and stops there: <b>every voice stays editable afterwards</b>, and the moment you move one
          this reads as your own voicing rather than a chord.
        </p>
      </div>
      <button class="browse__close" type="button" title="Escape" @click="close">
        ← Back<span class="browse__esc">Esc</span>
      </button>
    </header>

    <div class="browse__scroll">
      <p v-if="!chords.has" class="browse__empty">
        This build publishes no chords, so there is nothing to choose from. The voice pitches are still
        yours to set by hand on the deck.
      </p>

      <section v-for="g in chords.groups" :key="g.label" class="browse__group">
        <div class="browse__family">
          <div class="browse__familytext">
            <span class="browse__familyname">{{ g.label }}</span>
            <span class="browse__familynote">{{ g.chords.length }} chord{{ g.chords.length === 1 ? '' : 's' }}</span>
          </div>
        </div>
        <div class="browse__list">
          <button
            v-for="c in g.chords"
            :key="c.group + c.name"
            class="chord"
            :class="{ on: chords.matching && chords.matching.name === c.name && chords.matching.group === c.group }"
            :data-chord="`${c.group}:${c.name}`"
            type="button"
            @click="chords.apply(c)"
          >
            <span class="chord__name">
              {{ c.name }}
              <span class="chord__voices">{{ c.voices }} voice{{ c.voices === 1 ? '' : 's' }}</span>
            </span>
            <!--
              The semitones and the notes, because they are two different
              things a reader wants: the offset is what automation moves, and
              the note is what the chord means against the track.
            -->
            <span class="chord__semis tabular">
              <i v-for="(s, k) in c.semis" :key="k">
                {{ s > 0 ? '+' : '' }}{{ s }}<em>{{ noteName(rootHz(), s) }}</em>
              </i>
            </span>
          </button>
        </div>
      </section>

      <p v-if="chords.has" class="browse__foot">
        Voicings for a resonator rather than a keyboard: the thirds and sevenths sit an octave up so each
        voice keeps its own register. Six fundamentals inside four semitones is one thick note, not a chord.
        The root is Tune, currently <b>{{ hzText(rootHz()) }}</b>.
      </p>
    </div>
  </main>
</template>

<style scoped>
.chord {
  display: flex;
  flex-direction: column;
  gap: 3px;
  align-items: flex-start;
  padding: 7px 9px;
  text-align: left;
  background: rgb(255 255 255 / 0.02);
  border: 1px solid rgb(255 255 255 / 0.07);
  border-radius: 3px;
  cursor: pointer;
  color: inherit;
}
.chord:hover { background: rgb(255 255 255 / 0.05); }
.chord.on { border-color: var(--res-brass); background: rgb(255 255 255 / 0.06); }
.chord__name { font-size: 13px; display: flex; align-items: baseline; gap: 7px; }
.chord__voices { font-size: 9px; letter-spacing: 0.06em; text-transform: uppercase; color: var(--res-faint); }
.chord__semis { display: flex; flex-wrap: wrap; gap: 7px; font-size: 10px; color: var(--res-dim); }
.chord__semis i { font-style: normal; }
.chord__semis em { font-style: normal; color: var(--res-faint); margin-left: 3px; }
.browse__foot { margin: 10px 2px 0; font-size: 11px; color: var(--res-faint); max-width: 70ch; }
</style>
