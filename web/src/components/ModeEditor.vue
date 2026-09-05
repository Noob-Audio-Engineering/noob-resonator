<script setup>
/**
 * One partial's override: its pitch, its level and how long it rings.
 *
 * **The mode table is the product.** Nine global knobs generate up to four
 * thousand modes and, in every device of this kind, not one of them is
 * reachable — you get the object the knobs imply and nothing else. Exposing
 * the table costs nothing at runtime, it is native to a modal bank and to no
 * other architecture, and it is the natural end of a display that already
 * draws where every partial sits. Detune the third partial of a string by a
 * semitone and you have an object nobody has ever built.
 *
 * **The override addresses a partial by its physical index**, which is the
 * first float of each `modes` frame — not by where it sits in a list that
 * `select` reorders. Change Select and the same edit follows the same
 * partial.
 *
 * **It is written to the UI store, not sent as a message**, and that is not a
 * detail. A plug-in has no main loop, so a message channel has nothing to
 * pump it: the message route works against a standalone dev server and does
 * nothing at all inside a VST3, which is the worst shape of bug there is. The
 * store also carries the table inside the plug-in's saved state, so a project
 * reloads sounding exactly as it was saved with no editor ever opened.
 *
 * **Undo does not reach these.** The framework's history covers parameters,
 * and the override table is plug-in state rather than a parameter — which is
 * what lets it travel with a saved project and be reapplied before the first
 * block. Rather than build a second history with its own Ctrl+Z semantics,
 * the way back is Reset, and the footer says so instead of leaving a user to
 * discover it by pressing Ctrl+Z and watching nothing happen.
 *
 * Props: `partial` (one entry of the drawn series). Emits: `close`.
 */
import { computed } from 'vue';
import { EDIT_LIMITS, hzText, partialName, ratioText, timeText, useOverrides } from '../composables/useResonator.js';

const props = defineProps({ partial: { type: Object, required: true } });
defineEmits(['close']);

const overrides = useOverrides();
const edit = computed(() => overrides.get(props.partial.i, props.partial.j) || {});
const isEdited = computed(() => overrides.has(props.partial.i, props.partial.j));

const cents = computed(() => edit.value.cents ?? 0);
const gain = computed(() => edit.value.db ?? 0);
const decay = computed(() => edit.value.decay ?? 1);

const set = (patch) => overrides.set(props.partial.i, props.partial.j, patch);
const num = (e) => Number(e.target.value);
</script>

<template>
  <div class="me" role="group" :aria-label="partialName(partial)" @keydown.escape="$emit('close')">
    <div class="me__head">
      <span class="me__title">{{ partialName(partial) }}</span>
      <span class="me__at tabular">
        <template v-if="partial.offscreen">no longer in the published set</template>
        <template v-else>
          {{ ratioText(partial.ratio) }}× · {{ hzText(partial.hz) }}<template v-if="partial.ring != null"> · {{ timeText(partial.ring) }}</template>
        </template>
      </span>
      <button class="key me__x" type="button" @click="$emit('close')">Done</button>
    </div>

    <label class="me__row">
      <span class="me__k">Pitch</span>
      <input
        type="range"
        :min="-EDIT_LIMITS.cents"
        :max="EDIT_LIMITS.cents"
        step="1"
        :value="cents"
        @input="set({ cents: num($event) })"
      />
      <span class="me__v tabular">{{ cents > 0 ? '+' : '' }}{{ cents }} ct</span>
    </label>

    <label class="me__row">
      <span class="me__k">Level</span>
      <input
        type="range"
        :min="-EDIT_LIMITS.db"
        :max="EDIT_LIMITS.db"
        step="0.5"
        :value="gain"
        @input="set({ db: num($event) })"
      />
      <span class="me__v tabular">{{ gain > 0 ? '+' : '' }}{{ gain.toFixed(1) }} dB</span>
    </label>

    <label class="me__row">
      <span class="me__k">Ring</span>
      <input
        type="range"
        :min="EDIT_LIMITS.decayMin"
        :max="EDIT_LIMITS.decayMax"
        step="0.05"
        :value="decay"
        @input="set({ decay: num($event) })"
      />
      <span class="me__v tabular">×{{ decay.toFixed(2) }}</span>
    </label>

    <div class="me__foot">
      <button class="key" type="button" :disabled="!isEdited" @click="overrides.clear(partial.i, partial.j)">Reset this one</button>
      <button class="key" type="button" :disabled="!overrides.count" @click="overrides.clearAll()">
        Reset all {{ overrides.count || '' }}
      </button>
      <span class="me__note">Saved with the project. Undo does not reach these — Reset does.</span>
    </div>
  </div>
</template>
