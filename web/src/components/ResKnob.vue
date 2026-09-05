<script setup>
/**
 * The panel's knob: a dark machined cap in a brass bezel with a bright index,
 * inside an arc that carries the value, over a printed tick scale.
 *
 * The behaviour is entirely the framework's `useKnobGesture` — drag, wheel,
 * Shift for fine, double-click to reset, arrow keys, all bracketed as one
 * host automation gesture — and none of the look is. That division is the
 * standing rule for this project: the framework holds generics and every face
 * stays in the plug-in.
 *
 * **Brass carries no meaning here.** The three accents on this panel each say
 * one thing about the physics, so the controls are deliberately outside that
 * scheme: a knob is a knob whichever engine is running.
 *
 * A bipolar parameter draws its arc from twelve o'clock, so Inharm at −40 %
 * reads as a compression at a glance rather than as "a bit less than half".
 *
 * Props: `p` (the handle, required), `label` (defaults to the handle's name),
 * `size` in px — scaled by `--knob-scale` so a narrow window can shrink the
 * whole deck in one place without flattening the size hierarchy — `sweep` in
 * degrees, `bipolar` (defaults to the handle's own), `ticks`, `off` (why the
 * control is inert on this object: `{ short, why }`, the short form shown in
 * place of the hint and the long one on hover), and `hint` (a second line,
 * for a unit or a law the control has to state).
 */
import { computed } from 'vue';
import { useKnobGesture } from '@noob-audio-engineering/noob-vst-webgui-framework/vue';

const props = defineProps({
  p: { type: Object, required: true },
  label: { type: String, default: null },
  size: { type: Number, default: 52 },
  sweep: { type: Number, default: 280 },
  bipolar: { type: Boolean, default: null },
  ticks: { type: Number, default: 11 },
  off: { type: Object, default: null },
  hint: { type: String, default: null },
});

const { handlers, dragging } = useKnobGesture(props.p);
const on = computed(() => (props.off ? {} : handlers));
const centred = computed(() => (props.bipolar == null ? props.p.isBipolar : props.bipolar));

const R = 33;
const angleFor = (t) => -props.sweep / 2 + t * props.sweep;
const angle = computed(() => angleFor(props.p.norm));

/** A point on the dial at travel `t` and radius `r`, in the SVG's 100-unit box. */
function at(t, r) {
  const a = (angleFor(t) * Math.PI) / 180;
  return [50 + r * Math.sin(a), 50 - r * Math.cos(a)];
}
/** An arc between two travels, drawn the short way round the dial. */
function arc(from, to, r) {
  const [x0, y0] = at(from, r);
  const [x1, y1] = at(to, r);
  const large = Math.abs(to - from) * props.sweep > 180 ? 1 : 0;
  return `M ${x0} ${y0} A ${r} ${r} 0 ${large} ${to > from ? 1 : 0} ${x1} ${y1}`;
}

const track = computed(() => arc(0, 1, R + 8));
const value = computed(() => {
  const from = centred.value ? 0.5 : 0;
  return Math.abs(props.p.norm - from) < 0.002 ? '' : arc(from, props.p.norm, R + 8);
});
const marks = computed(() =>
  Array.from({ length: Math.max(2, props.ticks) }, (_, i) => {
    const n = Math.max(2, props.ticks);
    const t = i / (n - 1);
    const long = i === 0 || i === n - 1 || (centred.value && Math.abs(t - 0.5) < 1e-6);
    const [x1, y1] = at(t, R + 12);
    const [x2, y2] = at(t, long ? R + 18 : R + 15.5);
    return { i, x1, y1, x2, y2, long };
  }),
);
</script>

<template>
  <div
    class="knob"
    :class="{ 'is-off': !!off, 'is-live': dragging }"
    :title="off ? off.why : null"
    :style="{ '--knob-size': `calc(${size}px * var(--knob-scale, 1))` }"
  >
    <div class="knob__box">
      <svg
        viewBox="0 0 100 100"
        class="knob__dial"
        :tabindex="off ? -1 : 0"
        role="slider"
        :aria-label="label || p.name"
        :aria-valuetext="p.text"
        :aria-disabled="!!off"
        v-on="on"
      >
        <g class="knob__marks">
          <line v-for="m in marks" :key="m.i" :x1="m.x1" :y1="m.y1" :x2="m.x2" :y2="m.y2" :class="{ long: m.long }" />
        </g>
        <path :d="track" class="knob__track" />
        <path v-if="value" :d="value" class="knob__value" />
        <circle cx="50" cy="50" :r="R + 2" class="knob__bezel" />
        <circle cx="50" cy="50" :r="R" class="knob__cap" />
        <circle cx="50" cy="50" :r="R - 6" class="knob__crown" />
        <g :transform="`rotate(${angle} 50 50)`">
          <rect x="48.7" :y="50 - R + 3" width="2.6" height="13" rx="1.3" class="knob__index" />
        </g>
      </svg>
    </div>
    <div class="knob__label">{{ label || p.name }}</div>
    <div class="knob__value-text tabular">{{ off ? '—' : p.text }}</div>
    <div v-if="off || hint" class="knob__hint">{{ off ? off.short : hint }}</div>
  </div>
</template>
