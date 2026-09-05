/**
 * Presets: what they are, where they live, and what loading one does.
 *
 * **Two sources, one shape.** Factory presets come from the engine in the
 * manifest meta and are read-only — they are physics, and they belong next to
 * the code that defines the ranges they sit in, where they cannot drift out
 * of them. User presets live in the UI store, the same mechanism as the mode
 * table, so they ride inside the plug-in state and a saved project brings its
 * own presets back with it.
 *
 * **Applying is the page's job in both cases, and that is a fact rather than
 * a division of labour.** A parameter change has to go through the host to be
 * recorded and undoable, and only the editor can do that; the engine cannot
 * set a host parameter behind the host's back.
 *
 * ```json
 * { "v": 1, "name": "Struck Slate", "group": "Plate",
 *   "description": "one line, present tense, what it is",
 *   "values": { "type": 4, "tune": 110.0 },
 *   "modes": [ { "i": 3, "j": 1, "cents": -14, "db": -2, "decay": 1.4 } ] }
 * ```
 *
 * **`values` is plain units keyed by parameter id, and it is complete.** On
 * load *every* parameter the manifest declares is set — to its value in the
 * preset if it has one, and otherwise to its own default. A preset therefore
 * fully determines the state and cannot leave a stray control behind from
 * whatever was loaded before it. Full rather than sparse for the same reason:
 * a future version that changes a default must not silently move every preset
 * saved under the old one.
 *
 * **`modes` is mandatory and it always replaces.** An empty array means "this
 * preset has no overrides", which *clears* whatever the user had. One rule and
 * no ambiguity: after loading, the mode table is exactly what the preset says.
 * The per-mode table is the thing this architecture can do that no other can,
 * so a preset system that could not carry it would make the flagship feature
 * the one thing you cannot save — and the face says so, because a preset that
 * silently dropped a user's retuned partials would be worse than one that
 * admits it does not carry them.
 *
 * **Two ids never appear.** `bypass` is a transport control rather than a
 * sound, and a preset that silently bypasses the plug-in is a support ticket.
 * The three `src_*` are the standalone's demo source and are not the device.
 *
 * Unknown ids in `values` are ignored rather than an error, and so is an
 * unrecognised `v`: read what you recognise and leave the rest, which is what
 * lets an old page open a new preset without inventing behaviour for it.
 */
import { computed, reactive, ref } from 'vue';
import { getClient, hasParam, useNoobVstWebguiFramework, useParam, useStoredRef } from './useResonator.js';
import { objectAt } from '../objects.js';

/** The version this page writes. Anything else is read for what it has. */
export const PRESET_VERSION = 1;

/** Never saved and never applied: a transport control, and the standalone's own source. */
export const EXCLUDED = new Set(['bypass', 'src_kind', 'src_level', 'src_freq']);

/** How close two plain values must be to count as unchanged, relative to the parameter's range. */
const EPS = 1e-4;

/** The parameter specs a preset covers, from the manifest. */
function coveredSpecs(manifest) {
  const list = manifest?.params;
  if (!Array.isArray(list)) return [];
  return list.filter((p) => !EXCLUDED.has(p.id));
}

/** A preset read defensively: anything missing gets a safe shape rather than throwing. */
function normalise(p, source) {
  if (!p || typeof p !== 'object') return null;
  return {
    v: Number.isFinite(p.v) ? p.v : PRESET_VERSION,
    name: String(p.name || 'Untitled'),
    group: String(p.group || ''),
    description: String(p.description || ''),
    values: p.values && typeof p.values === 'object' ? p.values : {},
    // Mandatory in the format; an absent one is read as "no overrides" rather
    // than as "leave whatever was there", because leaving them is the one
    // behaviour the format rules out.
    modes: Array.isArray(p.modes) ? p.modes.filter((e) => e && Number.isFinite(e.i)) : [],
    source,
  };
}

let presets = null;

export function usePresets() {
  if (presets) return presets;
  const { manifest } = useNoobVstWebguiFramework();
  const stored = useStoredRef('presets', null);
  const modeStore = useStoredRef('modes', null);

  /** Read-only, from the engine. */
  const factory = computed(() => {
    const list = manifest.value?.meta?.presets;
    if (!Array.isArray(list)) return [];
    return list.map((p) => normalise(p, 'factory')).filter(Boolean);
  });

  /** The user's own, in the UI store beside the mode table. */
  const user = computed(() => {
    const list = Array.isArray(stored.value) ? stored.value : [];
    return list.map((p) => normalise(p, 'user')).filter(Boolean);
  });

  const all = computed(() => [...factory.value, ...user.value]);

  /**
   * Which preset is loaded, by source and name.
   *
   * Page state, not a parameter: it is a label for what was last applied, and
   * making it a parameter would put a name in an automation lane.
   */
  const loadedKey = ref(null);
  const loaded = computed(() => all.value.find((p) => keyOf(p) === loadedKey.value) || null);
  const keyOf = (p) => (p ? `${p.source}:${p.name}` : null);

  /**
   * Every plain value the device is currently at, for the ids a preset covers.
   *
   * **Read through the framework's handles, not through the raw client.** The
   * client's `Param` is not reactive, so a computed that read it never
   * re-evaluated and the "edited since loaded" dot never appeared however far
   * you moved a knob — a status that is wrong in the safe-looking direction,
   * which is the worst kind. The handles are refs and a computed tracks them.
   */
  function currentValues() {
    const out = {};
    for (const spec of coveredSpecs(manifest.value)) {
      if (!hasParam(spec.id)) continue;
      out[spec.id] = useParam(spec.id).plain;
    }
    return out;
  }

  const currentModes = () => {
    const v = modeStore.value;
    return v && Array.isArray(v.edits) ? v.edits : [];
  };

  /**
   * Whether anything has moved since the preset was loaded.
   *
   * **Diffed here rather than tracked by the engine.** A second copy of the
   * truth on the other side of the wire is a second thing that can disagree
   * with this one, and disagreement that looks plausible is the failure this
   * project keeps catching. The mode table is part of the comparison, because
   * it is part of the preset.
   */
  const modified = computed(() => {
    const p = loaded.value;
    if (!p) return false;
    const now = currentValues();
    for (const spec of coveredSpecs(manifest.value)) {
      const want = spec.id in p.values ? p.values[spec.id] : spec.default;
      const have = now[spec.id];
      if (have == null) continue;
      const span = Math.max(1e-9, Math.abs(spec.max - spec.min));
      if (Math.abs(have - want) > span * EPS) return true;
    }
    return !sameModes(currentModes(), p.modes);
  });

  return (presets = reactive({
    factory,
    user,
    all,
    loaded,
    loadedKey,
    modified,
    keyOf,

    /** Presets grouped by the object they are for, in the catalogue's order. */
    groups: computed(() => {
      const by = new Map();
      for (const p of all.value) {
        const ix = Number.isFinite(p.values?.type) ? Math.round(p.values.type) : -1;
        const label = p.group || (ix >= 0 ? objectAt(ix).label : 'Other');
        if (!by.has(label)) by.set(label, { label, index: ix, presets: [] });
        by.get(label).presets.push(p);
      }
      return [...by.values()].sort((a, b) => a.index - b.index);
    }),

    /**
     * Load one.
     *
     * Every covered parameter is set — from the preset where it has a value
     * and from its own default where it does not — so nothing survives from
     * whatever was loaded before. Each is one bracketed gesture, so the host
     * records it and undo reaches it.
     */
    apply(p) {
      if (!p) return;
      const c = getClient();
      for (const spec of coveredSpecs(manifest.value)) {
        const want = spec.id in p.values ? Number(p.values[spec.id]) : spec.default;
        if (!Number.isFinite(want)) continue;
        try {
          const param = c.param(spec.id);
          param.beginEdit();
          param.setPlain(want);
          param.endEdit();
        } catch {
          /* an id this build does not publish is ignored, not an error */
        }
      }
      // Always replaces, including with nothing.
      modeStore.value = p.modes.length ? { edits: p.modes.map((e) => ({ ...e })) } : null;
      loadedKey.value = keyOf(p);
    },

    /**
     * Save the current state as a user preset.
     *
     * `withModes` false writes an empty table rather than omitting one, which
     * is the format's way of saying "this preset deliberately has no partial
     * edits" — and on load it will clear them.
     */
    save({ name, description = '', withModes = true }) {
      const clean = String(name || '').trim();
      if (!clean) return null;
      const ix = Math.round(currentValues().type ?? 0);
      const entry = {
        v: PRESET_VERSION,
        name: clean,
        group: objectAt(ix).label,
        description: String(description || ''),
        values: currentValues(),
        modes: withModes ? currentModes().map((e) => ({ ...e })) : [],
      };
      const list = (Array.isArray(stored.value) ? stored.value : []).filter((q) => q.name !== clean);
      list.push(entry);
      list.sort((a, b) => a.name.localeCompare(b.name));
      stored.value = list;
      loadedKey.value = `user:${clean}`;
      return entry;
    },

    /** Whether a user preset of this name already exists, so the dialog can say "overwrite". */
    userHas(name) {
      const clean = String(name || '').trim();
      return user.value.some((p) => p.name === clean);
    },

    remove(p) {
      if (!p || p.source !== 'user') return;
      stored.value = (Array.isArray(stored.value) ? stored.value : []).filter((q) => q.name !== p.name);
      if (loadedKey.value === keyOf(p)) loadedKey.value = null;
    },

    rename(p, to) {
      const clean = String(to || '').trim();
      if (!p || p.source !== 'user' || !clean) return;
      const list = (Array.isArray(stored.value) ? stored.value : []).map((q) =>
        q.name === p.name ? { ...q, name: clean } : q,
      );
      list.sort((a, b) => a.name.localeCompare(b.name));
      stored.value = list;
      if (loadedKey.value === keyOf(p)) loadedKey.value = `user:${clean}`;
    },
  }));
}

/**
 * Presets that exist to be compared: identical but for one control.
 *
 * **Found structurally rather than by name.** Two factory presets are the
 * same object at the same budget with Selection on Loudest and on Lowest —
 * the sixty-eight decibel argument, made something a user meets by accident
 * rather than something they have to be told. Detecting it by looking for the
 * pair whose values differ in exactly one id means the browser keeps finding
 * them when the names change, and finds any other deliberate pair too.
 */
export function findPairs(list) {
  const pairs = [];
  for (let a = 0; a < list.length; a++) {
    for (let b = a + 1; b < list.length; b++) {
      const differ = differingIds(list[a], list[b]);
      if (differ.length === 1) pairs.push({ a: list[a], b: list[b], on: differ[0] });
    }
  }
  return pairs;
}

function differingIds(x, y) {
  const ids = new Set([...Object.keys(x.values || {}), ...Object.keys(y.values || {})]);
  const out = [];
  for (const id of ids) {
    const u = x.values?.[id];
    const v = y.values?.[id];
    if (u == null || v == null) {
      if (u !== v) out.push(id);
    } else if (Math.abs(Number(u) - Number(v)) > 1e-9) {
      out.push(id);
    }
    if (out.length > 1) return out;
  }
  return out;
}

/** Two mode tables are the same when they name the same modes with the same edits. */
function sameModes(a, b) {
  if (a.length !== b.length) return false;
  const key = (e) => `${e.i}:${e.j || 0}`;
  const map = new Map(b.map((e) => [key(e), e]));
  for (const e of a) {
    const o = map.get(key(e));
    if (!o) return false;
    for (const f of ['cents', 'db', 'decay']) {
      if (Math.abs((e[f] ?? (f === 'decay' ? 1 : 0)) - (o[f] ?? (f === 'decay' ? 1 : 0))) > 1e-6) return false;
    }
  }
  return true;
}
