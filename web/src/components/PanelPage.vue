<script setup>
/**
 * The panel: the top bar, what is loaded, the headline, the display, the
 * deck, and the bench when it is up — or the browse view in place of all of
 * it except the bar.
 *
 * **The display is never behind a tab** and takes every pixel the window can
 * spare. Everything else is fixed height and the display is what grows.
 *
 * The order down the page is the physical story rather than the signal path:
 * what is ringing, which of its partials survive, what that leaves, and only
 * then the controls that shape them. A resonator has no signal path worth
 * arranging a panel around — the incoming audio is a strike and the object
 * does the rest.
 *
 * The browse view floats over the panel rather than replacing it, so the
 * thing you are choosing for stays visible behind the thing you are choosing
 * from.
 */
import { ResizeGrip } from '@noob-audio-engineering/noob-vst-webgui-framework/vue';
import TopBar from './TopBar.vue';
import ObjectBar from './ObjectBar.vue';
import SelectStrip from './SelectStrip.vue';
import ModeDisplay from './ModeDisplay.vue';
import Deck from './Deck.vue';
import DevPanel from './DevPanel.vue';
import TypeBrowser from './TypeBrowser.vue';
import PresetBrowser from './PresetBrowser.vue';
import { WINDOW_MIN, ui, useDebug, useWindow } from '../composables/useResonator.js';

const debug = useDebug();
useWindow();
</script>

<template>
  <div class="res">
    <TopBar />
    <main class="res__body">
      <ObjectBar />
      <SelectStrip />
      <ModeDisplay />
      <Deck />
      <DevPanel v-if="debug" @close="debug = false" />
    </main>
    <!--
      The browser is a layer over the panel, not a page in place of it.
      Replacing the whole panel to choose an object took away the working
      context you are choosing *for* — and seeing what you are leaving is most
      of the value of browsing. The panel stays visible and dimmed behind it.
    -->
    <TypeBrowser v-if="ui.browsing" />
    <PresetBrowser v-else-if="ui.presets" />
    <ResizeGrip class="res__grip" :min="WINDOW_MIN" />
  </div>
</template>
