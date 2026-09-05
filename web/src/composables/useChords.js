/**
 * Chord tuning: a second way to fill the mode table.
 *
 * **Every object answers "what is it made of". A chord answers "what is it
 * tuned to".** An object model is safe about timbre and dangerous about pitch —
 * it always sounds like a thing, but tune it wrong against the track and it
 * fights. A set of tuned voices is the reverse: safe about pitch and
 * indifferent to timbre, because you told it the chord. Those are two different
 * instruments, and a table that can be filled either way is one plug-in that is
 * both.
 *
 * **The voices do not replace the object.** Each voice is a root, and each root
 * gets the object's own series — a chord of beams, a chord of pipes, six tines
 * a fifth apart. That is why this file sets pitches and nothing else: the
 * series is still the object's, and the mode table derives from it exactly as
 * it always has.
 *
 * ## The chord is a menu that fills the pitches, never a parameter
 *
 * The engine publishes the chords in `meta.chords` and this applies one by
 * writing `voices` and the voice pitches through the ordinary edit path, the
 * same way a preset is applied. **That is structural rather than a convention.**
 * A chord parameter would be a second place a voice's pitch is decided, and the
 * moment somebody nudged one voice the two would disagree about what the chord
 * is with no way to say which is true. A menu that only ever *writes* has no
 * such state: generate, then edit, and never generate instead of edit.
 *
 * It also keeps the arithmetic in Rust. `semis` arrives from the engine, so
 * nothing here computes an interval.
 *
 * ## Which chord is loaded is derived, not remembered
 *
 * There is no "loaded chord" to go stale. The face compares the live pitches
 * against the published chords and reports the one that matches, or **custom**
 * when none does. Nudge a voice after picking Major and it says custom, which
 * is *generate then edit* made visible at no cost — and it cannot disagree with
 * the parameters, because it is derived from them.
 */
import { computed, reactive } from 'vue';
import { getClient, hasParam, useNoobVstWebguiFramework, useParam } from './useResonator.js';

/** How close two semitone values count as the same. They are integers on the wire; this is belt and braces. */
const EPS = 1e-6;

let chords = null;

export function useChords() {
  if (chords) return chords;
  const { manifest } = useNoobVstWebguiFramework();

  /**
   * The voice pitch parameters, in the engine's own order.
   *
   * From `meta.voice_ids` rather than built as `'voice' + n`, so the page never
   * invents an id the build might not publish.
   */
  const ids = computed(() => {
    const list = manifest.value?.meta?.voice_ids;
    return Array.isArray(list) ? list.filter((id) => hasParam(id)) : [];
  });

  const count = hasParam('voices') ? useParam('voices') : null;
  const pitches = computed(() => ids.value.map((id) => useParam(id)));

  /** How many voices are sounding, which is a parameter and not the length of anything. */
  const sounding = computed(() => (count ? Math.round(count.plain) : ids.value.length ? 1 : 0));

  /** The published chords, grouped in the engine's own order. */
  const all = computed(() => {
    const list = manifest.value?.meta?.chords;
    if (!Array.isArray(list)) return [];
    return list
      .filter((c) => c && Array.isArray(c.semis))
      .map((c) => ({
        name: String(c.name || ''),
        group: String(c.group || ''),
        semis: c.semis.map(Number),
        voices: Number.isFinite(c.voices) ? Math.round(c.voices) : c.semis.length,
      }));
  });

  const groups = computed(() => {
    const by = new Map();
    for (const c of all.value) {
      if (!by.has(c.group)) by.set(c.group, { label: c.group, chords: [] });
      by.get(c.group).chords.push(c);
    }
    return [...by.values()];
  });

  /** The semitones the voices are actually at, as many as are sounding. */
  const live = computed(() => pitches.value.slice(0, sounding.value).map((p) => p.plain));

  /**
   * The chord the voices are currently at, or `null` for a voicing of the
   * user's own.
   *
   * **Only the sounding voices are compared.** A three-voice chord says nothing
   * about where voices four to six sit, so holding those against it would
   * report *custom* for a state that is exactly Major.
   */
  const matching = computed(() => {
    const now = live.value;
    if (!now.length) return null;
    return (
      all.value.find(
        (c) => c.voices === now.length && c.semis.every((s, i) => Math.abs(s - now[i]) < EPS),
      ) || null
    );
  });

  return (chords = reactive({
    has: computed(() => all.value.length > 0 && ids.value.length > 0),
    all,
    groups,
    ids,
    count,
    pitches,
    sounding,
    matching,
    /** What to call the current tuning on the face. */
    label: computed(() => (matching.value ? matching.value.name : 'custom')),

    /**
     * Apply a chord: how many voices, and where each one sits.
     *
     * Every write is its own bracketed gesture, so the host records them and
     * undo reaches them — the same path a knob uses, because this is the same
     * kind of change. **Voices the chord does not define are left where they
     * are**: a three-voice chord is a statement about three voices, and moving
     * the others would be the menu inventing pitches it was never given.
     */
    apply(c) {
      if (!c || !Array.isArray(c.semis)) return;
      const client = getClient();
      const write = (id, v) => {
        if (!hasParam(id) || !Number.isFinite(v)) return;
        try {
          const p = client.param(id);
          p.beginEdit();
          p.setPlain(v);
          p.endEdit();
        } catch {
          /* an id this build does not publish is ignored, not an error */
        }
      };
      write('voices', c.voices);
      c.semis.forEach((s, i) => write(ids.value[i], s));
    },
  }));
}

/**
 * A voice's pitch as a note name, for a root in hertz.
 *
 * Printed beside the semitone offset because "+16 st" is what the parameter is
 * and "E5" is what a musician hears. Both, rather than either: the offset is
 * the thing automation moves, and the note is the thing the chord means.
 */
const NOTES = ['C', 'C♯', 'D', 'D♯', 'E', 'F', 'F♯', 'G', 'G♯', 'A', 'A♯', 'B'];
export function noteName(rootHz, semis) {
  if (!Number.isFinite(rootHz) || rootHz <= 0 || !Number.isFinite(semis)) return '';
  // MIDI 69 is A440, and the note number need not be an integer if the root is
  // not tuned to a semitone — which is why it rounds rather than assuming.
  const midi = Math.round(69 + 12 * Math.log2(rootHz / 440) + semis);
  return `${NOTES[((midi % 12) + 12) % 12]}${Math.floor(midi / 12) - 1}`;
}
