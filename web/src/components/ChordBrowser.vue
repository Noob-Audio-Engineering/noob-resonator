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
import { midiName, noteName, useChords, useSlots } from '../composables/useChords.js';

const chords = useChords();
const slots = useSlots();
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
      <!--
        Six positions holding a chord each, so a voicing you built by hand is
        one click away again. They are the page's, in the same store as the mode
        table and the user presets, so they travel inside a saved project.

        **A slot holds the same shape as a published chord** — the voices and
        their semitones — which is why one row draws either, and why recalling
        one is the same write as picking Major rather than a second route to the
        same place.
      -->
      <section v-if="chords.has" class="browse__group">
        <div class="browse__family">
          <div class="browse__familytext">
            <span class="browse__familyname">Your six</span>
            <span class="browse__familynote">store what is sounding, recall it by position or by note</span>
          </div>
        </div>
        <!--
          **Two routes to the same chord, and only one of them lets go.** A
          recall writes the voice pitches, so it stands until something else
          changes them and undo reaches it. A note held on the keyboard only
          borrows the voices, and lifting your hands gives them back exactly as
          they were. That is the difference a player feels, so it is said here
          rather than left to be discovered.
        -->
        <p class="slots__how">
          Recalling one <b>writes the pitches</b> — it stays, and undo reaches it. A note
          <i>held</i> on the keyboard only borrows the voices, and they come back the moment you let go.
        </p>
        <div class="slots">
          <div
            v-for="e in slots.slots"
            :key="e.i"
            class="slot"
            :class="{ on: slots.matching && slots.matching.i === e.i, empty: e.empty }"
            :data-slot="e.i"
          >
            <button
              class="slot__pick"
              type="button"
              :disabled="e.empty"
              :title="e.empty ? 'Nothing stored here yet' : 'Tune the voices to this'"
              @click="slots.recall(e)"
            >
              <span class="slot__n">{{ e.i + 1 }}</span>
              <span class="slot__body">
                <span class="slot__name">
                  {{ e.empty ? 'empty' : `${e.voices} voice${e.voices === 1 ? '' : 's'}` }}
                  <!--
                    The note that recalls it, printed rather than hardcoded: a
                    performance affordance nobody can discover is not one.
                  -->
                  <i v-if="e.note != null" class="slot__note">{{ midiName(e.note) }}</i>
                </span>
                <span v-if="!e.empty" class="slot__semis tabular">
                  {{ e.semis.map((x) => (x > 0 ? `+${x}` : x)).join('  ') }}
                </span>
              </span>
            </button>
            <span class="slot__tools">
              <button class="key" type="button" title="Store what is sounding here" @click="slots.store(e.i)">
                Store
              </button>
              <button v-if="!e.empty" class="key" type="button" title="Empty this position" @click="slots.clear(e.i)">
                Clear
              </button>
            </span>
          </div>
        </div>
      </section>

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

.slots__how { margin: 0 2px 6px; font-size: 11px; color: var(--res-faint); max-width: 78ch; }
.slots { display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 6px; }
.slot {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 6px;
  background: rgb(255 255 255 / 0.02);
  border: 1px solid rgb(255 255 255 / 0.07);
  border-radius: 3px;
}
.slot.on { border-color: var(--res-brass); background: rgb(255 255 255 / 0.06); }
.slot.empty { opacity: 0.62; }
.slot__pick { display: flex; align-items: center; gap: 7px; flex: 1; text-align: left; color: inherit; cursor: pointer; }
.slot__pick:disabled { cursor: default; }
.slot__n {
  font-size: 10px;
  width: 15px;
  height: 15px;
  display: grid;
  place-items: center;
  border-radius: 2px;
  background: rgb(255 255 255 / 0.07);
  color: var(--res-dim);
}
.slot.on .slot__n { background: var(--res-brass); color: #14171b; }
.slot__body { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
.slot__name { font-size: 12px; display: flex; align-items: baseline; gap: 6px; }
.slot__note { font-style: normal; font-size: 9px; color: var(--res-faint); letter-spacing: 0.05em; }
.slot__semis { font-size: 9.5px; color: var(--res-faint); }
.slot__tools { display: flex; gap: 3px; }
</style>
