<script setup>
/**
 * The bench: what the engine is publishing, the partial table as numbers, and
 * where each column came from.
 *
 * Off by default, including under the standalone: everything this device has
 * to say is already on its face. It floats over the deck rather than sitting
 * under it, because a second plate in the flow pushed the display out through
 * the bottom of the window at the 900 × 520 minimum and the page began to
 * scroll — a development affordance does not get to break the smallest window
 * the plug-in supports.
 *
 * **The provenance card is the point of this panel, not the table.** The
 * table is a convenience; the card is the sentence a reader needs before they
 * copy a figure out of a screenshot, and the sentence it now gets to say is a
 * short one: every number on this page came off a stream, and in design mode
 * those streams are filled by the page's own quarantined arithmetic rather
 * than by an engine.
 */
import { computed } from 'vue';
import { Segmented } from '@noob-audio-engineering/noob-vst-webgui-framework/vue';
import ResKnob from './ResKnob.vue';
import {
  countText,
  hzText,
  ratioText,
  timeText,
  useDesignMode,
  useFundamental,
  useInfo,
  useModes,
  useObject,
  useObjectMeta,
  useOverrides,
  useRes,
  useResponse,
} from '../composables/useResonator.js';

defineEmits(['close']);

const modes = useModes();
const info = useInfo();
const response = useResponse();
const overrides = useOverrides();
const object = useObject();
const meta = useObjectMeta();
const designMode = useDesignMode();
const r = useRes();
const fundamental = useFundamental();

/** The first sixteen, which is as many as anyone reads off a table. */
const rows = computed(() => modes.list.slice(0, 16));
const peak = computed(() =>
  modes.list.reduce((m, p) => Math.max(m, p.bareDb ?? p.dbL ?? -Infinity), -Infinity),
);
const uses = computed(() => meta.value?.uses || null);
const state = (s) => (!s.has ? 'absent' : s.live ? 'live' : 'declared, silent');
</script>

<template>
  <section class="bench plate">
    <h3 class="cap bench__head">
      Bench<span class="why">the same numbers, as numbers</span>
      <button class="key bench__close" @click="$emit('close')">Close</button>
    </h3>
    <div class="bench__grid">
      <div class="bench__card">
        <h4 class="cap">Where these numbers come from</h4>
        <p v-if="designMode">
          <b>Design mode.</b> No plug-in has answered, so the three streams below are being filled by the
          page's own arithmetic in <code>src/dev/physics/</code> — the equations, so the panel has something
          to draw before the engine exists. Every level in there is invented and no figure on this page may
          be quoted. A production build contains none of that code.
        </p>
        <p v-else>
          <b>Live.</b> Every partial, level and ring time on this page came off the engine's streams. The
          panel derives none of them.
        </p>
        <p>
          The panel renders and does not compute. Where a number it wants is not on a stream, the readout
          that needed it goes dark rather than being worked out here — which is why every field is looked up
          by the name its stream declares, and not by an offset.
        </p>
        <p>
          Ratios are measured against <b>{{ fundamental.fromEngine ? 'the engine’s own fundamental' : 'the Tune control' }}</b>,
          {{ hzText(fundamental.hz) }}.
        </p>
      </div>

      <div class="bench__card">
        <h4 class="cap">What the engine is publishing</h4>
        <table class="tabular">
          <tbody>
            <tr><td>modes stream</td><td>{{ state(modes) }}</td></tr>
            <tr><td>response stream</td><td>{{ state(response) }}</td></tr>
            <tr><td>info stream</td><td>{{ state(info) }}</td></tr>
            <tr><td>partials available</td><td>{{ countText(info.available) }}</td></tr>
            <tr><td>modes the bank runs</td><td>{{ countText(info.used) }}</td></tr>
            <tr><td>drawn in the display</td><td>{{ modes.list.length }}</td></tr>
            <tr><td>crossover</td><td>{{ hzText(info.crossoverHz) }}</td></tr>
            <tr><td>ceiling</td><td>{{ info.ceilingHz == null ? 'not published' : info.ceilingHz > 0 ? hzText(info.ceilingHz) : 'none' }}</td></tr>
            <tr><td>mode table</td><td>{{ info.build == null ? '—' : info.build >= 0.999 ? 'settled' : Math.round(info.build * 100) + '%' }}</td></tr>
            <tr><td>overrides</td><td>{{ overrides.count || 'none' }}</td></tr>
          </tbody>
        </table>
        <p>
          Optional fields this build carries:
          <b>{{ [modes.hasBare ? 'db_bare' : null, modes.hasBaseHz ? 'base_hz' : null].filter(Boolean).join(', ') || 'none' }}</b>.
          Without <code>db_bare</code> the display draws no node ghosts; without <code>base_hz</code> it
          cannot show where Inharm moved a partial from.
        </p>
        <p v-if="!uses">
          <b>The manifest published no object table.</b> Nothing is greyed out, because greying a control the
          engine may be reading would be worse than greying none.
        </p>
        <p v-else>
          This object uses <b>{{ uses.length }}</b> controls; the rest are greyed because
          <code>meta.objects</code> says so, not because the page worked it out.
        </p>
      </div>

      <!--
        The standalone's demo source. Absent under a plug-in, because the host
        is the exciter there — and this device has nothing to say until
        something strikes it, so with no source and no host the panel is
        silent and looks broken rather than idle.
      -->
      <div v-if="r.srcKind" class="bench__card">
        <h4 class="cap">Source<span class="why"> · the standalone only</span></h4>
        <p>Nothing rings until something hits it. These drive the input when there is no host.</p>
        <div class="bench__source">
          <Segmented :p="r.srcKind" class="keys keys--tiny" />
          <ResKnob v-if="r.srcLevel" :p="r.srcLevel" label="Level" :size="42" hint="how hard it strikes" />
          <ResKnob v-if="r.srcFreq" :p="r.srcFreq" label="Rate" :size="42" hint="strikes a second, not a pitch" />
        </div>
      </div>

      <div class="bench__card" style="grid-column: span 2; min-width: 0; overflow: auto">
        <h4 class="cap">Partials · the index is what an override addresses</h4>
        <table class="tabular">
          <thead>
            <tr><th>#</th><th>ratio</th><th>Hz</th><th>L dB</th><th>R dB</th><th>bare</th><th>rings</th><th></th></tr>
          </thead>
          <tbody>
            <tr v-for="p in rows" :key="p.i" :class="{ edited: overrides.has(p.i, p.j) }">
              <td>{{ p.j > 0 ? `${p.i},${p.j}` : p.i }}</td>
              <td>{{ ratioText(p.hz / fundamental.hz) }}</td>
              <td>{{ hzText(p.hz) }}</td>
              <td>{{ p.dbL == null ? '—' : (p.dbL - peak).toFixed(1) }}</td>
              <td>{{ p.dbR == null ? '—' : (p.dbR - peak).toFixed(1) }}</td>
              <td>{{ p.bareDb == null ? '—' : (p.bareDb - peak).toFixed(1) }}</td>
              <td>{{ timeText(p.ring) }}</td>
              <td>{{ overrides.has(p.i, p.j) ? 'edited' : '' }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </section>
</template>
