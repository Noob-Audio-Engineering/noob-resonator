<script setup>
/**
 * The bar across the top: what this is, whether it is talking to a plug-in,
 * and the page-level tools. None of it is a parameter; the deck owns those.
 *
 * The status dot is honest about offline design mode — `connected` stays
 * false while the client is running on the design manifest, so the dot is
 * dark and the word beside it says so. The stamp that matters is not here
 * though: it is on the display's level axis, next to the numbers it is about.
 */
import { computed } from 'vue';
import { ui, useDebug, useNoobVstWebguiFramework, useObject, useRes } from '../composables/useResonator.js';
import LevelStrip from './LevelStrip.vue';
import { usePresets } from '../composables/usePresets.js';

const { connected, history, historyState, stats } = useNoobVstWebguiFramework();
const debug = useDebug();
const object = useObject();
const r = useRes();
const presets = usePresets();
const state = computed(() => (connected.value ? 'live' : 'design mode'));
</script>

<template>
  <header class="bar plate">
    <div class="bar__brand">
      <span class="bar__dot" :class="{ on: connected }" />
      <span class="bar__name">Noob Resonator</span>
      <span class="bar__sub">the object rings; you supply the strike</span>
      <!--
        What is loaded, as a label and not a control.
        It used to be a button that opened the browse view, and it was the
        wrong thing twice over: it reads as a status — it is just the object's
        name — and pressing it replaced the entire panel, so a user who took
        it for a label got everything they were working on swept away. The way
        in is the button that says what it does.
      -->
      <span v-if="!ui.browsing" class="bar__object">{{ object.label }}</span>
      <!--
        What is loaded and whether it has been touched since. A dot rather
        than a word, because "edited" beside a name is a status a user reads
        once and then stops seeing.
      -->
      <button
        v-if="!ui.browsing && !ui.presets"
        class="key bar__preset"
        type="button"
        :title="presets.loaded ? (presets.modified ? 'Edited since it was loaded' : 'Loaded, unchanged') : 'No preset loaded'"
        @click="ui.presets = true"
      >
        <span v-if="presets.modified" class="bar__dirty" />
        {{ presets.loaded ? presets.loaded.name : 'Presets' }}
      </button>
      <!--
        Bypass, said out loud. Everything else on the panel goes on drawing
        the object it would be making if it were running, which is right —
        the display is about the object, not about the output — but a user
        looking at a full display and hearing nothing deserves to be told
        why, and told it somewhere that is always on screen.
      -->
      <button
        v-if="r.bypass && r.bypass.on"
        class="bar__bypass"
        type="button"
        title="The device is bypassed. Click to switch it back in."
        @click="r.bypass.setOn(false)"
      >
        bypassed
      </button>
    </div>

    <!--
      The level readout lives here rather than in the deck, because the deck
      scrolls at small windows and a meter that can scroll out of sight is not
      a meter. A resonator with a long decay can climb after the input stops.
    -->
    <LevelStrip class="bar__level" />

    <div class="bar__tools">
      <span class="bar__state" :class="{ design: !connected }">{{ state }}</span>
      <span v-if="connected" class="bar__state tabular">{{ Math.round(stats.rttAvgMs || 0) }} ms</span>
      <button class="key" :disabled="!historyState.canUndo" title="Ctrl+Z" @click="history.undo()">Undo</button>
      <button class="key" :disabled="!historyState.canRedo" title="Ctrl+Shift+Z" @click="history.redo()">Redo</button>
      <button class="key" :class="{ on: historyState.ab === 'B' }" title="Ctrl+B" @click="history.toggleAB()">
        {{ historyState.ab }}
      </button>
      <button class="key" :class="{ on: debug }" @click="debug = !debug">Bench</button>
    </div>
  </header>
</template>
