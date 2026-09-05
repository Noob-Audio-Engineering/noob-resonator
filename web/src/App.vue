<script setup>
/**
 * Noob Resonator: the root. Once the manifest is in (`ready`) the panel
 * renders; before that a short status line says what the client is doing. In
 * development the offline design manifest makes that immediate.
 *
 * Nothing may call `useParam` before `ready`, which is why the panel is a
 * child behind `v-if` rather than markup here: a handle for a parameter the
 * manifest has not described yet throws, and the whole page goes blank.
 *
 * Keyboard: Ctrl+Z / Ctrl+Shift+Z (or Ctrl+Y) undo and redo through the
 * framework's history, Ctrl+B toggles A/B.
 */
import { onBeforeUnmount, onMounted } from 'vue';
import { useNoobVstWebguiFramework } from '@noob-audio-engineering/noob-vst-webgui-framework/vue';
import PanelPage from './components/PanelPage.vue';

const { ready, connected, history } = useNoobVstWebguiFramework();

function onKey(e) {
  const t = e.target;
  if (t && (t.tagName === 'INPUT' || t.tagName === 'SELECT' || t.tagName === 'TEXTAREA')) return;
  const mod = e.ctrlKey || e.metaKey;
  const k = e.key.toLowerCase();
  if (mod && k === 'z' && !e.shiftKey) history.undo();
  else if ((mod && k === 'y') || (mod && e.shiftKey && k === 'z')) history.redo();
  else if (mod && k === 'b') history.toggleAB();
  else return;
  e.preventDefault();
}
onMounted(() => window.addEventListener('keydown', onKey));
onBeforeUnmount(() => window.removeEventListener('keydown', onKey));
</script>

<template>
  <PanelPage v-if="ready" />
  <div v-else class="res res--wait">
    <div class="res__wait">{{ connected ? 'loading the manifest' : 'connecting to the plug-in' }}</div>
  </div>
</template>
