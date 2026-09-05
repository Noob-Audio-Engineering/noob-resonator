<script setup>
/**
 * The preset view: what this device can be, one per row, grouped by object.
 *
 * The same shape as the object browser, which is the house pattern now — a
 * layer over the panel rather than a page in place of it, so the settings you
 * are choosing for stay visible behind the thing you are choosing from.
 *
 * **Loading replaces the mode table, and the view says so before you do it,
 * not after.** Presets carry your retuned partials; an empty table in a
 * preset clears yours. That is one rule with no ambiguity, and it is the kind
 * of thing a user must be told rather than discover — a preset that silently
 * dropped somebody's retuned partials would be worse than one that admits it
 * does not carry them.
 *
 * **Pairs are found and shown as pairs.** Two factory presets that differ in
 * exactly one control exist to be compared, and the browser marks them so
 * rather than leaving the comparison to be discovered: the Selection pair is
 * the whole argument of this device met by accident. They are detected
 * structurally, by diffing values, so the marking survives a rename and finds
 * any other deliberate pair too.
 *
 * Factory presets are read-only. A user's own can be renamed and deleted, and
 * saving over an existing name says it is overwriting rather than quietly
 * doing it.
 */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { findPairs, usePresets } from '../composables/usePresets.js';
import { ui } from '../composables/useResonator.js';

const presets = usePresets();

const saving = ref(false);
const name = ref('');
const description = ref('');
const withModes = ref(true);
/** The renaming target, and the text it is being renamed to. */
const renaming = ref(null);
const renameTo = ref('');

const close = () => (ui.presets = false);

/** Presets that differ in exactly one control, keyed so a row can say which. */
const pairOf = computed(() => {
  const map = new Map();
  for (const g of presets.groups) {
    for (const { a, b, on } of findPairs(g.presets)) {
      map.set(presets.keyOf(a), { with: b, on });
      map.set(presets.keyOf(b), { with: a, on });
    }
  }
  return map;
});

const overwriting = computed(() => presets.userHas(name.value));

function beginSave() {
  name.value = presets.loaded?.source === 'user' ? presets.loaded.name : '';
  description.value = presets.loaded?.source === 'user' ? presets.loaded.description : '';
  withModes.value = true;
  saving.value = true;
}
function confirmSave() {
  if (!name.value.trim()) return;
  presets.save({ name: name.value, description: description.value, withModes: withModes.value });
  saving.value = false;
}
function commitRename(p) {
  presets.rename(p, renameTo.value);
  renaming.value = null;
}
function onKey(e) {
  if (e.key !== 'Escape') return;
  e.preventDefault();
  if (renaming.value) renaming.value = null;
  else if (saving.value) saving.value = false;
  else close();
}
onMounted(() => window.addEventListener('keydown', onKey));
onBeforeUnmount(() => window.removeEventListener('keydown', onKey));
</script>

<template>
  <main class="browse" role="region" aria-label="Presets">
    <header class="browse__head">
      <div>
        <h2 class="browse__title">Presets</h2>
        <p class="browse__lede">
          What you have goes on ringing while you look, and nothing changes until you pick one.
          <b>Presets carry your retuned partials, and loading one replaces them</b> — a preset saved without
          any will clear yours.
        </p>
      </div>
      <button class="key browse__save" type="button" @click="beginSave">Save current…</button>
      <button class="browse__close" type="button" title="Escape" @click="close">
        ← Back<span class="browse__esc">Esc</span>
      </button>
    </header>

    <!-- Saving: named, described, and explicit about the partial edits. -->
    <form v-if="saving" class="save" @submit.prevent="confirmSave">
      <div class="save__row">
        <label class="save__field">
          <span>Name</span>
          <input v-model="name" type="text" autofocus placeholder="Struck Slate" />
        </label>
        <label class="save__field save__field--wide">
          <span>Description</span>
          <input v-model="description" type="text" placeholder="one line, present tense, what it is" />
        </label>
      </div>
      <label class="save__check">
        <input v-model="withModes" type="checkbox" />
        <span>
          Save my retuned partials with it
          <i>{{ withModes ? '' : '— saved empty, so loading this will clear them' }}</i>
        </span>
      </label>
      <div class="save__foot">
        <button class="key" type="submit" :disabled="!name.trim()">
          {{ overwriting ? 'Overwrite' : 'Save' }}
        </button>
        <button class="key" type="button" @click="saving = false">Cancel</button>
        <span v-if="overwriting" class="save__warn">A preset of yours already has this name.</span>
      </div>
    </form>

    <div class="browse__scroll">
      <p v-if="!presets.all.length" class="browse__empty">
        No presets yet. This build publishes none, and you have saved none — <b>Save current…</b> makes the
        first.
      </p>

      <section v-for="g in presets.groups" :key="g.label" class="browse__group">
        <div class="browse__family">
          <div class="browse__familytext">
            <span class="browse__familyname">{{ g.label }}</span>
            <span class="browse__familynote">{{ g.presets.length }} preset{{ g.presets.length === 1 ? '' : 's' }}</span>
          </div>
        </div>
        <div class="browse__list">
          <div
            v-for="p in g.presets"
            :key="presets.keyOf(p)"
            class="preset"
            :class="{ on: presets.keyOf(p) === presets.loadedKey, paired: pairOf.has(presets.keyOf(p)) }"
          >
            <button class="preset__pick" type="button" @click="presets.apply(p)">
              <span class="preset__name">
                <template v-if="renaming === presets.keyOf(p)">{{ p.name }}</template>
                <template v-else>{{ p.name }}</template>
                <span class="preset__src" :class="p.source">{{ p.source }}</span>
                <span v-if="presets.keyOf(p) === presets.loadedKey" class="browse__badge">
                  loaded{{ presets.modified ? ' · edited' : '' }}
                </span>
                <span v-if="pairOf.get(presets.keyOf(p))" class="preset__pair">
                  A/B with {{ pairOf.get(presets.keyOf(p)).with.name }} · differs only in
                  {{ pairOf.get(presets.keyOf(p)).on }}
                </span>
              </span>
              <span v-if="p.description" class="preset__desc">{{ p.description }}</span>
              <span class="preset__facts">
                {{ p.modes.length ? `${p.modes.length} partial${p.modes.length === 1 ? '' : 's'} retuned` : 'no partial edits' }}
              </span>
            </button>

            <div v-if="p.source === 'user'" class="preset__tools">
              <template v-if="renaming === presets.keyOf(p)">
                <input v-model="renameTo" class="preset__rename" type="text" @keydown.enter="commitRename(p)" />
                <button class="key" type="button" @click="commitRename(p)">Rename</button>
              </template>
              <template v-else>
                <button
                  class="key"
                  type="button"
                  @click="((renaming = presets.keyOf(p)), (renameTo = p.name))"
                >
                  Rename
                </button>
                <button class="key" type="button" @click="presets.remove(p)">Delete</button>
              </template>
            </div>
          </div>
        </div>
      </section>
    </div>
  </main>
</template>
