<script setup>
/**
 * The browse view: what this device can be, one object per row, grouped by
 * the engine that produces it. It replaces the panel while it is up, and the
 * top bar stays, so it is always clear what is loaded and where you are.
 *
 * **Browsing does not touch the audio.** The object that is loaded keeps
 * ringing with its own settings the whole time this view is open: the `type`
 * parameter is written only when a row is chosen. Nothing here previews by
 * switching, which would be audible, would push entries onto the undo history
 * and would fight automation on that parameter. Leaving by Escape, by the
 * close button or by choosing the object already loaded writes nothing at
 * all.
 *
 * **Each row draws that object's own partial series.** That is what makes a
 * browse view worth having here rather than a menu: this device's difference
 * between one object and the next is not a faceplate or a name, it is where
 * the partials sit, and the preview is the one picture that shows it.
 * Switching from String to Beam in a dropdown is a word changing; here it is
 * visible before you commit.
 *
 * **The grouping is the honest one and it teaches the panel you are about to
 * meet.** Five objects are a mode bank, where the thing itself vibrates and
 * every partial is one resonator paid for separately. Two are a waveguide,
 * where the object is only a boundary and the air inside rings, so one delay
 * loop gives every harmonic at once. Which engine you are on is why half the
 * deck is greyed when you get there.
 */
import { computed, onBeforeUnmount, onMounted } from 'vue';
import { ENGINES, OBJECTS } from '../objects.js';
import { ui, useObject, useRes } from '../composables/useResonator.js';
import SeriesPreview from './SeriesPreview.vue';
import EngineDiagram from './EngineDiagram.vue';

const r = useRes();
const object = useObject();
const current = computed(() => (r.type ? r.type.index : 2));

const groups = ENGINES.map((e) => ({
  ...e,
  objects: OBJECTS.map((o, index) => ({ ...o, index })).filter((o) => o.engine === e.id),
}));

/**
 * Take one. The only write in this component, and only when the choice is a
 * change — picking the object already loaded leaves the history alone.
 *
 * **It writes the object and nothing else.** Choosing a Tube used to set
 * Opening here as well, so that the name and the display agreed; the engine
 * does that itself now and publishes it as `forces`, which is the better
 * place for it — one authority for what an object pins, and the panel free to
 * simply say so.
 */
function choose(t) {
  if (t.index !== current.value && r.type) {
    r.type.begin();
    r.type.setIndex(t.index);
    r.type.end();
  }
  ui.browsing = false;
}
/** Leave without choosing: nothing has been written, so there is nothing to undo. */
const cancel = () => (ui.browsing = false);

function onKey(e) {
  if (e.key === 'Escape') {
    e.preventDefault();
    cancel();
  }
}
onMounted(() => window.addEventListener('keydown', onKey));
onBeforeUnmount(() => window.removeEventListener('keydown', onKey));
</script>

<template>
  <main class="browse" role="region" aria-label="Change resonator">
    <header class="browse__head">
      <div>
        <h2 class="browse__title">Change resonator</h2>
        <p class="browse__lede">
          What you have is still ringing while you look, and nothing changes until you pick one. Each row draws
          where that object’s partials fall, which is the whole difference between them — and the one thing a
          list of names cannot show you. The rows are drawn at reference settings, so they show what an object
          <em>is</em> rather than what it would sound like at yours.
        </p>
      </div>
      <!--
        The way back, and it has to look like one. This is a whole-page view,
        so the panel a user was working on is not on screen while it is up;
        a dim word in the far corner is not enough to say "nothing has
        happened yet and this is how you leave".
      -->
      <button class="browse__close" type="button" title="Escape" @click="cancel">
        ← Back to {{ object.label }}<span class="browse__esc">Esc</span>
      </button>
    </header>

    <div class="browse__scroll">
      <section v-for="g in groups" :key="g.id" class="browse__group">
        <div class="browse__family" :class="g.id">
          <EngineDiagram :engine="g.id" />
          <div class="browse__familytext">
            <span class="browse__familyname">{{ g.label }}</span>
            <span class="browse__familynote">{{ g.note }}</span>
          </div>
        </div>
        <div class="browse__list">
          <button
            v-for="t in g.objects"
            :key="t.id"
            class="browse__card"
            :class="[g.id, { on: t.index === current }]"
            type="button"
            :aria-current="t.index === current ? 'true' : undefined"
            @click="choose(t)"
          >
            <span class="browse__preview"><SeriesPreview :object="t" /></span>
            <span class="browse__meta">
              <span class="browse__name">
                {{ t.label }}
                <span class="browse__sub">{{ t.short }}</span>
                <span v-if="t.index === current" class="browse__badge">loaded</span>
              </span>
              <span class="browse__blurb">{{ t.blurb }}</span>
              <span class="browse__uses"><b>Good for</b> {{ t.uses }}</span>
              <span class="browse__src">
                <b :class="{ target: t.derivation === 'tuning target' }">{{ t.derivation }}</b> · {{ t.source }}
              </span>
            </span>
          </button>
        </div>
      </section>
    </div>
  </main>
</template>
