/**
 * Noob Resonator entry point.
 *
 * In development the client is told about the offline design manifest, so if
 * no plug-in answers within a second the page renders against invented frames
 * and hands over the moment a real server connects. Production builds never
 * include it, and the panel marks itself DESIGN MODE while it is in use so
 * nothing invented can be mistaken for a measurement.
 */
import { createApp } from 'vue';
import { configureClient } from '@noob-audio-engineering/noob-vst-webgui-framework/vue';
import './style.css';
import App from './App.vue';

if (import.meta.env.DEV) {
  const { offline } = await import('./dev/manifest.js');
  configureClient({ offline });
}

createApp(App).mount('#app');
